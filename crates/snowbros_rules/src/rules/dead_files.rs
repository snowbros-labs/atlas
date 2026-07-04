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
];

fn is_excluded(path: &str) -> bool {
    // Config files: vite.config.ts, next.config.mjs, tailwind.config.js…
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.contains(".config.") {
        return true;
    }
    if EXCLUDED_NAMES.contains(&name) {
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
        let ctx = AnalysisContext::new(g, None, &[]);
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
    fn confidence_is_possible_severity_low() {
        let mut g = SemanticGraph::new();
        g.add_node(Node::file("src/orphan.ts"));
        let ctx = AnalysisContext::new(&g, None, &[]);
        let diags = DeadFiles.run(&ctx);
        assert_eq!(diags[0].confidence, Confidence::Possible);
        assert_eq!(diags[0].severity, Severity::Low);
    }
}
