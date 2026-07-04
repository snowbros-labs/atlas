//! `deps/unused-dependency` — runtime dependencies never imported.
//!
//! Accuracy-first scoping:
//! - only `dependencies` are checked; `devDependencies` are routinely
//!   used through tooling (CLIs, configs) rather than imports
//! - `@types/*` packages are skipped (consumed by the compiler)
//! - confidence is [`Confidence::Likely`], not certain — packages can be
//!   referenced from configs, CSS, or fields like `prisma.seed`

use std::collections::BTreeSet;

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity};
use snowbros_graph::NodeKind;

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::rules::file_location;

/// See module docs.
pub struct UnusedDependencies;

/// Maps an import specifier to the npm package that provides it:
/// `react-dom/client` → `react-dom`, `@scope/pkg/sub` → `@scope/pkg`.
/// Returns `None` for Node builtins (`node:fs`, `fs`).
pub fn package_name(specifier: &str) -> Option<String> {
    if specifier.starts_with("node:") {
        return None;
    }
    let mut segments = specifier.split('/');
    let first = segments.next()?;
    if NODE_BUILTINS.contains(&first) {
        return None;
    }
    if let Some(scoped) = first.strip_prefix('@') {
        let second = segments.next()?;
        // A bare "@scope" without a name is not a valid package.
        if scoped.is_empty() || second.is_empty() {
            return None;
        }
        return Some(format!("{first}/{second}"));
    }
    Some(first.to_string())
}

/// Node builtins importable without the `node:` prefix.
const NODE_BUILTINS: &[&str] = &[
    "assert",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "timers",
    "tls",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "worker_threads",
    "zlib",
];

impl Rule for UnusedDependencies {
    fn id(&self) -> &'static str {
        "deps/unused-dependency"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(pkg) = ctx.package_json else {
            return Vec::new();
        };

        // Every package actually imported somewhere, normalized to
        // package names.
        let used: BTreeSet<String> = ctx
            .graph
            .packages()
            .into_iter()
            .filter_map(|(_, node)| match &node.kind {
                NodeKind::Package { name, .. } => package_name(name),
                _ => None,
            })
            .collect();

        let mut diagnostics = Vec::new();
        for (name, version) in &pkg.dependencies {
            if name.starts_with("@types/") {
                continue;
            }
            // Framework packages are consumed implicitly (JSX
            // auto-runtime injects react/jsx-runtime; next runs via CLI).
            if ctx.framework_owned_packages.contains(name) {
                continue;
            }
            if used.contains(name) {
                continue;
            }
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Unused dependency",
                    format!(
                        "`{name}` is declared in package.json dependencies but \
                         never imported by any scanned file. Unused dependencies \
                         slow installs, widen the attack surface, and confuse \
                         readers."
                    ),
                    "dependencies",
                    Severity::Low,
                    // Configs/CSS/tool fields can reference packages without
                    // an import — likely, not proven.
                    Confidence::Likely,
                    file_location("package.json"),
                )
                .with_evidence(Evidence::note(format!(
                    "declared as \"{name}\": \"{version}\" but no import of `{name}` found"
                ))),
            );
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_framework::PackageJson;
    use snowbros_graph::{EdgeKind, Node, SemanticGraph};

    fn pkg(deps: &[(&str, &str)]) -> PackageJson {
        let mut p = PackageJson::default();
        for (n, v) in deps {
            p.dependencies.insert((*n).into(), (*v).into());
        }
        p
    }

    #[test]
    fn package_name_normalization() {
        assert_eq!(package_name("react"), Some("react".into()));
        assert_eq!(package_name("react-dom/client"), Some("react-dom".into()));
        assert_eq!(
            package_name("@supabase/ssr/dist/x"),
            Some("@supabase/ssr".into())
        );
        assert_eq!(package_name("node:fs"), None);
        assert_eq!(package_name("fs"), None);
    }

    #[test]
    fn reports_unused_runtime_dep_only() {
        let mut g = SemanticGraph::new();
        let f = g.add_node(Node::file("src/app.ts"));
        let used = g.add_node(Node::package("react", None));
        let sub = g.add_node(Node::package("react-dom/client", None));
        g.add_edge(f, used, EdgeKind::DependsOn);
        g.add_edge(f, sub, EdgeKind::DependsOn);

        let pkg = pkg(&[
            ("react", "^19.0.0"),
            ("react-dom", "^19.0.0"),   // used via subpath import
            ("lodash", "^4.17.21"),     // unused
            ("@types/node", "^22.0.0"), // skipped
        ]);
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs {
                package_json: Some(&pkg),
                ..Default::default()
            },
        );
        let diags = UnusedDependencies.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`lodash`"));
        assert_eq!(diags[0].confidence, Confidence::Likely);
    }

    #[test]
    fn no_package_json_no_findings() {
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs::default(),
        );
        assert!(UnusedDependencies.run(&ctx).is_empty());
    }
}
