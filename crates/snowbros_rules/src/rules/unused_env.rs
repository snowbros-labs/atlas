//! `env/unused-env-var` — variables declared in `.env*` but never read.
//!
//! FP guards, per the accuracy-first policy:
//! - framework-magic variables are skipped (`NODE_ENV`, `NEXT_*`,
//!   `VERCEL_*`, …) — frameworks and deploy targets read them without a
//!   `process.env` access in user code
//! - `DATABASE_URL`/`DIRECT_URL` are skipped (Prisma reads them from
//!   its schema file)
//! - confidence is [`Confidence::Possible`]: env vars can be consumed
//!   by shell scripts, Docker, or dynamic `process.env[key]` access the
//!   engine refuses to guess about

use std::collections::BTreeSet;

use snowbros_core::{
    Confidence, Diagnostic, Evidence, Position, Severity, SourceLocation, Span, SuggestedFix,
};

use crate::context::AnalysisContext;
use crate::registry::Rule;

/// Names read by frameworks/tools without appearing in user code.
const SKIP_EXACT: &[&str] = &["NODE_ENV", "PORT", "TZ", "CI", "DATABASE_URL", "DIRECT_URL"];

/// Prefixes read by frameworks/platforms implicitly.
const SKIP_PREFIXES: &[&str] = &["NEXT_", "VERCEL_", "TURBO_"];

/// See module docs.
pub struct UnusedEnvVars;

impl Rule for UnusedEnvVars {
    fn id(&self) -> &'static str {
        "env/unused-env-var"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let reads: BTreeSet<&str> = ctx
            .file_facts
            .values()
            .flat_map(|f| f.env_reads.iter().map(|r| r.name.as_str()))
            .collect();

        let mut diagnostics = Vec::new();
        for decl in ctx.env_declarations {
            if SKIP_EXACT.contains(&decl.name.as_str())
                || SKIP_PREFIXES.iter().any(|p| decl.name.starts_with(p))
            {
                continue;
            }
            if reads.contains(decl.name.as_str()) {
                continue;
            }
            let line = decl.line;
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Unused environment variable",
                    format!(
                        "`{}` is declared in `{}` but `process.env.{}` is never \
                         read by any scanned file. Dead configuration confuses \
                         deploys and may leak stale secrets.",
                        decl.name, decl.file, decl.name
                    ),
                    "environment",
                    Severity::Low,
                    Confidence::Possible,
                    SourceLocation::new(
                        decl.file.clone(),
                        Span::new(Position::new(line, 1), Position::new(line, 1), 0, 0),
                    ),
                )
                .with_evidence(Evidence::note(format!(
                    "declared at {}:{} — no `process.env.{}` read found",
                    decl.file, line, decl.name
                )))
                .with_fix(SuggestedFix {
                    description: format!("Delete line {line} of {} (`{}`)", decl.file, decl.name),
                    replacement: None,
                    target: Some(decl.name.clone()),
                }),
            );
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ContextInputs, EnvDeclaration};
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, parse, Language};
    use std::collections::BTreeMap;

    fn decl(name: &str) -> EnvDeclaration {
        EnvDeclaration {
            name: name.into(),
            file: ".env".into(),
            line: 1,
        }
    }

    #[test]
    fn unread_var_reported_read_and_magic_vars_skipped() {
        let facts =
            extract_facts(&parse("const k = process.env.API_KEY;", Language::TypeScript).unwrap());
        let mut map = BTreeMap::new();
        map.insert(camino::Utf8PathBuf::from("src/a.ts"), facts);

        let declarations = vec![
            decl("API_KEY"),         // read → not reported
            decl("GHOST_TOKEN"),     // unread → reported
            decl("NODE_ENV"),        // magic exact → skipped
            decl("NEXT_PUBLIC_URL"), // magic prefix → skipped
            decl("DATABASE_URL"),    // prisma → skipped
        ];
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                env_declarations: &declarations,
                ..ContextInputs::default()
            },
        );
        let diags = UnusedEnvVars.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`GHOST_TOKEN`"));
        assert_eq!(diags[0].confidence, Confidence::Possible);
    }
}
