//! `graph/dead-file` — files nothing imports.
//!
//! High false-positive territory, handled per the accuracy-first policy:
//! - entry points (framework routes, configs, tests, type declarations,
//!   scripts) are excluded by pattern
//! - confidence is only [`Confidence::Possible`] — the graph cannot see
//!   runtime loading, HTML script tags, or tooling references
//! - severity is [`Severity::Low`]

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity};
use snowbros_graph::{EdgeKind, NodeKind};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::requirements::{AnalysisStage, LanguageSupport, RuleRequirements};
use crate::rules::file_location;

/// See module docs.
pub struct DeadFiles;

/// Path substrings that mark a file as an entry point or tool-consumed —
/// never reported as dead.
const EXCLUDED_DIR_MARKERS: &[&str] = &[
    "pages/",   // Next.js pages router (file-system routed)
    "app/",     // Next.js app router / general app entry dirs
    "routes/",  // Remix, SvelteKit, Express conventions
    "scripts/", // invoked directly, not imported
    "tests/",
    "__tests__/",
    "e2e/",
    "cypress/",
    "stories/",
    "migrations/", // Django/Alembic migrations — loaded by the framework
];

/// File-name suffixes that mark entry points / tool-consumed files.
const EXCLUDED_SUFFIXES: &[&str] = &[
    ".test.ts",
    ".test.tsx",
    ".test.js",
    ".test.jsx",
    ".spec.ts",
    ".spec.tsx",
    ".spec.js",
    ".spec.jsx",
    ".stories.tsx",
    ".stories.ts",
    ".d.ts",
    "_test.py", // Python test module convention
];

/// Exact file names (any directory) that are conventionally entry points.
const EXCLUDED_NAMES: &[&str] = &[
    "middleware.ts",
    "instrumentation.ts",
    "main.ts",
    "main.tsx",
    "main.js",
    "index.ts",
    "index.tsx",
    "index.js",
    "server.ts",
    "server.js",
    "worker.ts",
    "worker.js",
    // Python entry points and tool-loaded modules: run directly, imported by
    // string path, or loaded implicitly by the runtime/framework — never dead
    // just because no source file imports them.
    "__init__.py", // package marker, imported implicitly
    "__main__.py", // `python -m pkg` entry
    "main.py",     // conventional script entry
    "conftest.py", // pytest fixtures, auto-loaded
    "setup.py",    // packaging entry
    "manage.py",   // Django CLI entry
    "wsgi.py",     // WSGI server entry
    "asgi.py",     // ASGI server entry
    "settings.py", // Django settings, loaded by string path
    "urls.py",     // Django URLconf, loaded by string path
];

pub(crate) fn is_excluded(path: &str) -> bool {
    // Config files: vite.config.ts, next.config.mjs, tailwind.config.js…
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.contains(".config.") {
        return true;
    }
    if EXCLUDED_NAMES.contains(&name) {
        return true;
    }
    // Python test modules are `test_*.py` (prefix) as well as `*_test.py`
    // (suffix, above) — pytest discovers both; neither is dead code.
    if name.starts_with("test_") && name.ends_with(".py") {
        return true;
    }
    if EXCLUDED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        return true;
    }
    EXCLUDED_DIR_MARKERS
        .iter()
        .any(|m| path.starts_with(m) || path.contains(&format!("/{m}")))
}

impl Rule for DeadFiles {
    fn id(&self) -> &'static str {
        "graph/dead-file"
    }

    /// Language-agnostic: a file with no incoming import edges is dead
    /// regardless of language. Runs at the semantic (import-graph) stage, with
    /// language-specific entry-point conventions handled by [`is_excluded`].
    fn requirements(&self) -> RuleRequirements {
        RuleRequirements {
            languages: LanguageSupport::Any,
            minimum_stage: AnalysisStage::Semantic,
        }
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (id, node) in ctx.graph.files() {
            let NodeKind::File { path } = &node.kind else {
                continue;
            };
            if is_excluded(path.as_str()) {
                continue;
            }
            if ctx.graph.has_incoming(id, EdgeKind::Imports) {
                continue;
            }

            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Potentially dead file",
                    format!(
                        "`{path}` is not imported by any other file in the project. \
                         If it is not an entry point loaded by other means, it is \
                         dead code."
                    ),
                    "architecture",
                    Severity::Low,
                    // The graph cannot see dynamic loading or tooling refs.
                    Confidence::Possible,
                    file_location(path.clone()),
                )
                .with_evidence(Evidence::note(
                    "no incoming import edges in the project graph",
                )),
            );
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_graph::{Node, SemanticGraph};

    fn ctx_diags(g: &SemanticGraph) -> Vec<String> {
        let ctx = AnalysisContext::new(
            g,
            Default::default(),
            crate::context::ContextInputs::default(),
        );
        DeadFiles
            .run(&ctx)
            .into_iter()
            .map(|d| d.location.file.to_string())
            .collect()
    }

    #[test]
    fn unimported_file_reported() {
        let mut g = SemanticGraph::new();
        let a = g.add_node(Node::file("src/used.ts"));
        let b = g.add_node(Node::file("src/entry.ts"));
        g.add_node(Node::file("src/orphan.ts"));
        g.add_edge(b, a, EdgeKind::Imports);

        // entry.ts imports something but nothing imports it → also a
        // candidate; orphan.ts definitely reported.
        let files = ctx_diags(&g);
        assert!(files.contains(&"src/orphan.ts".to_string()));
        assert!(files.contains(&"src/entry.ts".to_string()));
        assert!(!files.contains(&"src/used.ts".to_string()));
    }

    #[test]
    fn entrypoints_and_tests_excluded() {
        let mut g = SemanticGraph::new();
        g.add_node(Node::file("app/page.tsx"));
        g.add_node(Node::file("src/pages/home.tsx"));
        g.add_node(Node::file("src/util.test.ts"));
        g.add_node(Node::file("next.config.mjs"));
        g.add_node(Node::file("src/index.ts"));
        g.add_node(Node::file("types/global.d.ts"));
        g.add_node(Node::file("scripts/migrate.ts"));

        assert!(ctx_diags(&g).is_empty());
    }

    #[test]
    fn python_entrypoints_and_tests_excluded() {
        let mut g = SemanticGraph::new();
        g.add_node(Node::file("pkg/__init__.py"));
        g.add_node(Node::file("main.py"));
        g.add_node(Node::file("conftest.py"));
        g.add_node(Node::file("manage.py"));
        g.add_node(Node::file("proj/settings.py"));
        g.add_node(Node::file("api/test_views.py")); // test_ prefix
        g.add_node(Node::file("api/models_test.py")); // _test.py suffix
        g.add_node(Node::file("app/migrations/0001_initial.py"));

        assert!(ctx_diags(&g).is_empty());
    }

    #[test]
    fn orphan_python_module_is_reported() {
        let mut g = SemanticGraph::new();
        g.add_node(Node::file("pkg/orphan.py"));
        assert!(ctx_diags(&g).contains(&"pkg/orphan.py".to_string()));
    }

    #[test]
    fn is_language_agnostic() {
        assert_eq!(
            DeadFiles.requirements().languages,
            crate::requirements::LanguageSupport::Any
        );
    }

    #[test]
    fn confidence_is_possible_severity_low() {
        let mut g = SemanticGraph::new();
        g.add_node(Node::file("src/orphan.ts"));
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs::default(),
        );
        let diags = DeadFiles.run(&ctx);
        assert_eq!(diags[0].confidence, Confidence::Possible);
        assert_eq!(diags[0].severity, Severity::Low);
    }
}
