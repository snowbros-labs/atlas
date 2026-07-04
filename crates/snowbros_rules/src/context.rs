//! Read-only context handed to every rule.

use std::collections::BTreeSet;

use camino::Utf8PathBuf;
use snowbros_core::Span;
use snowbros_framework::{framework_packages, DetectedFramework, PackageJson};
use snowbros_graph::SemanticGraph;

/// An import the resolver could not map to a project file or package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedImport {
    /// File containing the import (root-relative).
    pub file: Utf8PathBuf,
    /// The specifier as written.
    pub specifier: String,
    /// Location of the specifier in the file.
    pub span: Span,
}

/// Everything a rule may look at. Strictly read-only.
#[derive(Debug, Clone)]
pub struct AnalysisContext<'a> {
    /// The project's semantic graph.
    pub graph: &'a SemanticGraph,
    /// Parsed `package.json`, when the project has one.
    pub package_json: Option<&'a PackageJson>,
    /// Packages belonging to detected frameworks. These are consumed
    /// implicitly (JSX auto-runtime, CLIs) and must not be flagged as
    /// unused.
    pub framework_owned_packages: BTreeSet<String>,
    /// Imports the resolver could not map anywhere.
    pub unresolved_imports: &'a [UnresolvedImport],
}

impl<'a> AnalysisContext<'a> {
    /// Builds a context, deriving framework-owned packages from the
    /// detection results.
    pub fn new(
        graph: &'a SemanticGraph,
        package_json: Option<&'a PackageJson>,
        frameworks: &[DetectedFramework],
        unresolved_imports: &'a [UnresolvedImport],
    ) -> Self {
        let framework_owned_packages = frameworks
            .iter()
            .flat_map(|d| framework_packages(d.framework))
            .map(|s| s.to_string())
            .collect();
        Self {
            graph,
            package_json,
            framework_owned_packages,
            unresolved_imports,
        }
    }
}
