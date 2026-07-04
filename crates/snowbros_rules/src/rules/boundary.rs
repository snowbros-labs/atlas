//! Server/client boundary rules for Next.js App Router.
//!
//! Two deterministic violations:
//!
//! - `next/server-only-in-client`: a `"use client"` file can reach —
//!   through the project import graph — a module that imports the
//!   `server-only` marker package. `next build` fails on this; the
//!   engine reports it in milliseconds with the exact import chain.
//!
//! - `next/private-env-in-client`: a `"use client"` file reads a
//!   `process.env` variable without the `NEXT_PUBLIC_` prefix. Next.js
//!   only inlines public variables into the client bundle, so the read
//!   is `undefined` in the browser — a silent runtime bug, not a build
//!   error.
//!
//! Both rules run only on detected Next.js projects.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use camino::Utf8PathBuf;
use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::rules::file_location;

/// Whether facts mark a file as a client module.
fn is_client(facts: &snowbros_parser::FileFacts) -> bool {
    facts.directives.iter().any(|d| d == "use client")
}

/// Whether facts import the `server-only` marker package.
fn imports_server_only(facts: &snowbros_parser::FileFacts) -> bool {
    facts.imports.iter().any(|i| i.specifier == "server-only")
}

/// See module docs.
pub struct ServerOnlyInClient;

impl Rule for ServerOnlyInClient {
    fn id(&self) -> &'static str {
        "next/server-only-in-client"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        if !ctx.framework_owned_packages.contains("next") {
            return Vec::new();
        }

        // Forward adjacency over project-internal imports.
        let mut adjacency: BTreeMap<&Utf8PathBuf, Vec<&Utf8PathBuf>> = BTreeMap::new();
        for b in ctx.import_bindings {
            adjacency.entry(&b.from).or_default().push(&b.to);
        }

        let mut diagnostics = Vec::new();
        for (client, facts) in &ctx.file_facts {
            if !is_client(facts) {
                continue;
            }
            // BFS with parent tracking for the evidence chain.
            let mut parent: BTreeMap<&Utf8PathBuf, &Utf8PathBuf> = BTreeMap::new();
            let mut seen: BTreeSet<&Utf8PathBuf> = BTreeSet::new();
            let mut queue: VecDeque<&Utf8PathBuf> = VecDeque::new();
            seen.insert(client);
            queue.push_back(client);

            let mut hit: Option<&Utf8PathBuf> = None;
            while let Some(current) = queue.pop_front() {
                let marked = ctx.file_facts.get(current).is_some_and(imports_server_only);
                if marked {
                    hit = Some(current);
                    break;
                }
                for next in adjacency.get(current).into_iter().flatten() {
                    if seen.insert(next) {
                        parent.insert(next, current);
                        queue.push_back(next);
                    }
                }
            }

            let Some(hit) = hit else {
                continue;
            };
            // Reconstruct the chain client → … → hit.
            let mut chain = vec![hit];
            let mut cursor = hit;
            while let Some(&prev) = parent.get(cursor) {
                chain.push(prev);
                cursor = prev;
            }
            chain.reverse();
            let chain_str = chain
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(" → ");

            let mut diag = Diagnostic::new(
                self.id(),
                "Server-only module reachable from client component",
                format!(
                    "`{client}` is a client component (\"use client\") but its \
                     import graph reaches `{hit}`, which imports `server-only`. \
                     `next build` will fail on this chain."
                ),
                "architecture",
                Severity::High,
                Confidence::Certain,
                file_location(client.clone()),
            );
            diag = diag.with_evidence(Evidence::note(format!("import chain: {chain_str}")));
            diagnostics.push(diag);
        }
        diagnostics
    }
}

/// See module docs.
pub struct PrivateEnvInClient;

impl Rule for PrivateEnvInClient {
    fn id(&self) -> &'static str {
        "next/private-env-in-client"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        if !ctx.framework_owned_packages.contains("next") {
            return Vec::new();
        }
        let mut diagnostics = Vec::new();
        for (path, facts) in &ctx.file_facts {
            if !is_client(facts) {
                continue;
            }
            for read in &facts.env_reads {
                if read.name.starts_with("NEXT_PUBLIC_") || read.name == "NODE_ENV" {
                    continue;
                }
                diagnostics.push(
                    Diagnostic::new(
                        self.id(),
                        "Private env var read in client component",
                        format!(
                            "`process.env.{}` is read in the client component \
                             `{path}`, but Next.js only inlines `NEXT_PUBLIC_*` \
                             variables into the browser bundle — this value will \
                             be `undefined` at runtime. If the value is secret, \
                             exposing it with a NEXT_PUBLIC_ prefix would be a \
                             leak; read it on the server and pass it down instead.",
                            read.name
                        ),
                        "architecture",
                        Severity::High,
                        // Custom bundler config can inline extra vars.
                        Confidence::Likely,
                        SourceLocation::new(path.clone(), read.span),
                    )
                    .with_evidence(Evidence::note(format!(
                        "\"use client\" file reads `process.env.{}` (not \
                         NEXT_PUBLIC_-prefixed)",
                        read.name
                    ))),
                );
            }
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ContextInputs, ImportBinding};
    use snowbros_framework::{DetectedFramework, Framework};
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, parse, Language};

    fn next_detected() -> Vec<DetectedFramework> {
        vec![DetectedFramework {
            framework: Framework::NextJs,
            confidence: snowbros_core::Confidence::Certain,
            version: None,
            evidence: vec![],
        }]
    }

    fn facts_map(entries: &[(&str, &str)]) -> BTreeMap<Utf8PathBuf, snowbros_parser::FileFacts> {
        entries
            .iter()
            .map(|(path, src)| {
                let lang = if path.ends_with(".tsx") {
                    Language::Tsx
                } else {
                    Language::TypeScript
                };
                (
                    Utf8PathBuf::from(*path),
                    extract_facts(&parse(*src, lang).unwrap()),
                )
            })
            .collect()
    }

    #[test]
    fn transitive_server_only_reach_reported_with_chain() {
        let map = facts_map(&[
            (
                "components/widget.tsx",
                "\"use client\";\nimport { load } from \"./data\";\nexport const W = () => null;",
            ),
            (
                "components/data.ts",
                "import { q } from \"./db\";\nexport const load = q;",
            ),
            (
                "components/db.ts",
                "import \"server-only\";\nexport const q = 1;",
            ),
        ]);
        let bindings = vec![
            ImportBinding {
                from: "components/widget.tsx".into(),
                to: "components/data.ts".into(),
                names: vec!["load".into()],
            },
            ImportBinding {
                from: "components/data.ts".into(),
                to: "components/db.ts".into(),
                names: vec!["q".into()],
            },
        ];
        let g = SemanticGraph::new();
        let frameworks = next_detected();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                frameworks: &frameworks,
                import_bindings: &bindings,
                ..ContextInputs::default()
            },
        );
        let diags = ServerOnlyInClient.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].location.file, "components/widget.tsx");
        assert!(diags[0].evidence[0]
            .description
            .contains("components/data.ts → components/db.ts"));
        assert_eq!(diags[0].confidence, Confidence::Certain);
    }

    #[test]
    fn server_component_reaching_server_only_is_fine() {
        let map = facts_map(&[
            (
                "app/data.ts",
                "import \"server-only\";\nexport const q = 1;",
            ),
            (
                "lib/loader.ts",
                "import { q } from \"./data\";\nexport const l = q;",
            ),
        ]);
        let bindings = vec![ImportBinding {
            from: "lib/loader.ts".into(),
            to: "app/data.ts".into(),
            names: vec!["q".into()],
        }];
        let g = SemanticGraph::new();
        let frameworks = next_detected();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                frameworks: &frameworks,
                import_bindings: &bindings,
                ..ContextInputs::default()
            },
        );
        assert!(ServerOnlyInClient.run(&ctx).is_empty());
    }

    #[test]
    fn private_env_in_client_reported_public_ok() {
        let map = facts_map(&[(
            "components/cfg.tsx",
            "\"use client\";\nconst a = process.env.SECRET_API_URL;\nconst b = process.env.NEXT_PUBLIC_URL;\nconst c = process.env.NODE_ENV;\nexport const C = () => null;",
        )]);
        let g = SemanticGraph::new();
        let frameworks = next_detected();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                frameworks: &frameworks,
                ..ContextInputs::default()
            },
        );
        let diags = PrivateEnvInClient.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("SECRET_API_URL"));
    }

    #[test]
    fn silent_without_next_or_directive() {
        let map = facts_map(&[(
            "src/server.ts",
            "const a = process.env.SECRET;\nimport \"server-only\";",
        )]);
        let g = SemanticGraph::new();
        // Next detected but file has no "use client" → both silent.
        let frameworks = next_detected();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                frameworks: &frameworks,
                ..ContextInputs::default()
            },
        );
        assert!(ServerOnlyInClient.run(&ctx).is_empty());
        assert!(PrivateEnvInClient.run(&ctx).is_empty());
    }
}
