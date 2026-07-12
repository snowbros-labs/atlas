//! Read-only context handed to every rule.

use std::collections::{BTreeMap, BTreeSet};

use camino::{Utf8Path, Utf8PathBuf};
use snowbros_core::Span;
use snowbros_framework::nextjs::NextProjectModel;
use snowbros_framework::{framework_packages, DetectedFramework, PackageJson};
use snowbros_graph::SemanticGraph;
use snowbros_parser::{FileFacts, Language};
use snowbros_semantic::SemanticModel;

use crate::requirements::RuleRequirements;

/// An import the resolver could not map to a project file or package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedImport {
    /// File containing the import (root-relative).
    pub file: Utf8PathBuf,
    /// The specifier as written.
    pub specifier: String,
    /// Location of the specifier in the file.
    pub span: Span,
    /// Whether the specifier matched a configured tsconfig `paths` alias
    /// but still resolved nowhere — a *broken alias* (typo or missing
    /// target) rather than an ordinary unresolved import. Set by the
    /// resolver-aware pipeline; `false` by default.
    pub matched_alias: bool,
}

/// A resolved project-internal import with the names it binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportBinding {
    /// Importing file (root-relative).
    pub from: Utf8PathBuf,
    /// Imported file (root-relative, resolved).
    pub to: Utf8PathBuf,
    /// Names bound (`default`, `*`, or named exports).
    pub names: Vec<String>,
}

/// A variable declared in a root `.env*` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvDeclaration {
    /// Variable name.
    pub name: String,
    /// Which env file declares it.
    pub file: Utf8PathBuf,
    /// 1-based line number of the declaration.
    pub line: u32,
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
    /// Per-file extracted facts (exports, env reads, dynamic API calls).
    pub file_facts: BTreeMap<Utf8PathBuf, FileFacts>,
    /// Variables declared in root `.env*` files.
    pub env_declarations: &'a [EnvDeclaration],
    /// Resolved project-internal imports with bound names.
    pub import_bindings: &'a [ImportBinding],
    /// The project symbol model over Atlas IR. `None` for legacy callers
    /// (and older tests) that predate the semantic layer; semantic rules
    /// treat it as an empty project.
    pub semantic: Option<&'a SemanticModel>,
    /// The Next.js project model, when the project is a routed Next.js
    /// app. `None` otherwise; Next.js structural rules no-op without it.
    pub next_model: Option<&'a NextProjectModel>,
    /// Detected language of each source file (root-relative path → language).
    /// The scheduler uses it to decide whether a rule's findings on a file
    /// are admissible for that file's language (see [`AnalysisContext::admits`]).
    /// Empty for legacy callers, which makes every finding admissible — the
    /// pre-multi-language behavior.
    pub file_languages: BTreeMap<Utf8PathBuf, Language>,
}

/// Inputs for building an [`AnalysisContext`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextInputs<'a> {
    /// Parsed `package.json`, when the project has one.
    pub package_json: Option<&'a PackageJson>,
    /// Framework detection results.
    pub frameworks: &'a [DetectedFramework],
    /// Imports the resolver could not map anywhere.
    pub unresolved_imports: &'a [UnresolvedImport],
    /// Variables declared in root `.env*` files.
    pub env_declarations: &'a [EnvDeclaration],
    /// Resolved project-internal imports with bound names.
    pub import_bindings: &'a [ImportBinding],
    /// The project symbol model over Atlas IR, when available.
    pub semantic: Option<&'a SemanticModel>,
    /// The Next.js project model, when available.
    pub next_model: Option<&'a NextProjectModel>,
}

impl<'a> AnalysisContext<'a> {
    /// Builds a context, deriving framework-owned packages from the
    /// detection results.
    pub fn new(
        graph: &'a SemanticGraph,
        file_facts: BTreeMap<Utf8PathBuf, FileFacts>,
        inputs: ContextInputs<'a>,
    ) -> Self {
        let framework_owned_packages = inputs
            .frameworks
            .iter()
            .flat_map(|d| framework_packages(d.framework))
            .map(|s| s.to_string())
            .collect();
        Self {
            graph,
            package_json: inputs.package_json,
            framework_owned_packages,
            unresolved_imports: inputs.unresolved_imports,
            file_facts,
            env_declarations: inputs.env_declarations,
            import_bindings: inputs.import_bindings,
            semantic: inputs.semantic,
            next_model: inputs.next_model,
            file_languages: BTreeMap::new(),
        }
    }

    /// Attaches the per-file language map used by the scheduler to gate rules
    /// by language. Builder-style so the `new` signature — and every existing
    /// caller and test — stays unchanged.
    pub fn with_file_languages(mut self, file_languages: BTreeMap<Utf8PathBuf, Language>) -> Self {
        self.file_languages = file_languages;
        self
    }

    /// Whether a finding a rule produced for `file` is admissible under the
    /// rule's `requirements`.
    ///
    /// A file whose language is known must satisfy the rule's language + stage
    /// contract. A file with no known language — a manifest (`package.json`),
    /// an env file, any non-source artifact — is language-neutral and always
    /// admitted; those findings are not gated by a source-language policy.
    pub fn admits(&self, requirements: &RuleRequirements, file: &Utf8Path) -> bool {
        match self.file_languages.get(file) {
            Some(language) => requirements.admits(*language),
            None => true,
        }
    }
}
