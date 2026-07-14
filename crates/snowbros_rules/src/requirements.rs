//! Rule execution requirements — the contract deciding *whether a rule may
//! run on a given language* ([RFC 0002] §4.1, §5, §2.2).
//!
//! Before Python, every rule silently assumed ECMAScript semantics because
//! JS/TS shared one ecosystem. Once a second language enters the graph that
//! assumption becomes a real policy, and this module makes it explicit: a rule
//! advertises the language family it applies to and the minimum analysis stage
//! it needs, and the scheduler runs the rule against a language only when the
//! language's frontend is mature enough to supply that stage. No rule body
//! contains a `match language` — the policy lives here, once.
//!
//! [RFC 0002]: https://github.com/snowbros-labs/atlas/blob/master/docs/rfcs/0002-atlas-multi-language-semantic-platform.md

use snowbros_parser::Language;

/// The analysis stages, ordered by depth. A rule declares the minimum stage
/// it needs; the engine only lets it run where that stage exists.
///
/// Ordering is semantic: `Ast < Semantic < … < Interprocedural`, so a stage
/// comparison answers "is a deep-enough model available here?".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnalysisStage {
    /// Syntax tree only.
    Ast,
    /// Atlas IR: symbols, imports, exports, references, resolved names.
    Semantic,
    /// Declared/annotated types + type-reference edges + heritage.
    TypeAware,
    /// Resolved call-graph edges (intra + cross-file).
    CallGraph,
    /// Per-function control-flow graph.
    ControlFlow,
    /// Def-use / taint / nullability along control flow.
    DataFlow,
    /// Summaries propagated across the call graph.
    Interprocedural,
}

/// How far a language's frontend has matured ([RFC 0002] §2.2). This is the
/// mechanical link between the maturity tier and what the engine will run:
/// each tier guarantees analysis up to a maximum stage, and a rule needing a
/// deeper stage is skipped on that language automatically — no special casing.
///
/// [RFC 0002]: https://github.com/snowbros-labs/atlas/blob/master/docs/rfcs/0002-atlas-multi-language-semantic-platform.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LanguageMaturity {
    /// Parsing only.
    Experimental,
    /// Semantic model available; a subset of rules run.
    Preview,
    /// Call graph available; meets the accuracy bar for its shipped rules.
    Stable,
    /// The full engine: control flow, data flow, interprocedural.
    Enterprise,
}

impl LanguageMaturity {
    /// The deepest analysis stage this tier guarantees. A rule whose
    /// `minimum_stage` exceeds this is not run on the language.
    pub fn max_stage(self) -> AnalysisStage {
        match self {
            LanguageMaturity::Experimental => AnalysisStage::Ast,
            LanguageMaturity::Preview => AnalysisStage::Semantic,
            LanguageMaturity::Stable => AnalysisStage::CallGraph,
            LanguageMaturity::Enterprise => AnalysisStage::Interprocedural,
        }
    }
}

/// The maturity of a language's frontend today.
///
/// Provisional and deliberately central: as the frontend registry grows this
/// will move onto the frontend itself, but keeping the single source of truth
/// here lets the scheduler enforce the maturity → stage contract now.
/// ECMAScript is the reference Enterprise language; Python is Preview (parsing
/// and semantic, per its M3 target); everything else recognized-but-unwired is
/// Experimental.
pub fn frontend_maturity(language: Language) -> LanguageMaturity {
    if language.is_ecmascript() {
        LanguageMaturity::Enterprise
    } else if language == Language::Python {
        LanguageMaturity::Preview
    } else {
        LanguageMaturity::Experimental
    }
}

/// Which language family a rule applies to.
///
/// Deliberately *groups*, not individual languages, so the set scales as
/// languages are added without a rule ever enumerating them. The registry
/// expands a group to concrete languages via [`LanguageSupport::includes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageSupport {
    /// Every language — a genuinely language-agnostic rule (import cycles,
    /// dead files, layering) that reads only shared IR/graph facts.
    Any,
    /// The JavaScript / TypeScript family (JS, JSX, TS, TSX).
    EcmaScript,
    /// Python.
    Python,
    /// Go.
    Go,
    /// Rust.
    Rust,
    /// Java.
    Java,
    /// The C family (C, C++).
    CFamily,
    /// The .NET family (currently C#).
    DotNet,
}

impl LanguageSupport {
    /// Whether this group includes `language`.
    pub fn includes(self, language: Language) -> bool {
        match self {
            LanguageSupport::Any => true,
            LanguageSupport::EcmaScript => language.is_ecmascript(),
            LanguageSupport::Python => language == Language::Python,
            LanguageSupport::Go => language == Language::Go,
            LanguageSupport::Rust => language == Language::Rust,
            LanguageSupport::Java => language == Language::Java,
            LanguageSupport::CFamily => false, // no C/C++ variants recognized yet
            LanguageSupport::DotNet => language == Language::CSharp,
        }
    }
}

/// What a rule needs in order to run: the language family it applies to and
/// the minimum analysis stage it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleRequirements {
    /// Language family the rule applies to.
    pub languages: LanguageSupport,
    /// Minimum analysis stage the rule reads.
    pub minimum_stage: AnalysisStage,
}

impl RuleRequirements {
    /// The default for every rule Atlas shipped before multi-language: the
    /// ECMAScript family at the semantic stage. Chosen so behavior on JS/TS
    /// repos is unchanged — ECMAScript is Enterprise, so the stage never gates
    /// a TS/JS finding, and non-ECMAScript files are excluded from these
    /// historically-ECMAScript rules.
    pub const fn ecmascript() -> Self {
        Self {
            languages: LanguageSupport::EcmaScript,
            minimum_stage: AnalysisStage::Semantic,
        }
    }

    /// Whether a finding produced for a file of `language` is admissible: the
    /// rule must apply to the language, and the language's frontend must be
    /// mature enough to supply the rule's minimum stage.
    pub fn admits(&self, language: Language) -> bool {
        self.languages.includes(language)
            && frontend_maturity(language).max_stage() >= self.minimum_stage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_are_ordered_by_depth() {
        assert!(AnalysisStage::Ast < AnalysisStage::Semantic);
        assert!(AnalysisStage::Semantic < AnalysisStage::CallGraph);
        assert!(AnalysisStage::CallGraph < AnalysisStage::Interprocedural);
    }

    #[test]
    fn maturity_maps_to_max_stage() {
        assert_eq!(
            LanguageMaturity::Experimental.max_stage(),
            AnalysisStage::Ast
        );
        assert_eq!(
            LanguageMaturity::Preview.max_stage(),
            AnalysisStage::Semantic
        );
        assert_eq!(
            LanguageMaturity::Stable.max_stage(),
            AnalysisStage::CallGraph
        );
        assert_eq!(
            LanguageMaturity::Enterprise.max_stage(),
            AnalysisStage::Interprocedural
        );
    }

    #[test]
    fn ecmascript_is_enterprise_python_is_preview() {
        assert_eq!(
            frontend_maturity(Language::TypeScript),
            LanguageMaturity::Enterprise
        );
        assert_eq!(
            frontend_maturity(Language::Tsx),
            LanguageMaturity::Enterprise
        );
        assert_eq!(
            frontend_maturity(Language::Python),
            LanguageMaturity::Preview
        );
        assert_eq!(
            frontend_maturity(Language::Go),
            LanguageMaturity::Experimental
        );
    }

    #[test]
    fn language_support_groups_expand_correctly() {
        assert!(LanguageSupport::Any.includes(Language::Python));
        assert!(LanguageSupport::EcmaScript.includes(Language::Tsx));
        assert!(!LanguageSupport::EcmaScript.includes(Language::Python));
        assert!(LanguageSupport::Python.includes(Language::Python));
        assert!(!LanguageSupport::Python.includes(Language::TypeScript));
        assert!(LanguageSupport::DotNet.includes(Language::CSharp));
    }

    #[test]
    fn ecmascript_rule_admits_ts_not_python() {
        let req = RuleRequirements::ecmascript();
        assert!(req.admits(Language::TypeScript));
        assert!(req.admits(Language::JavaScript));
        // Python is excluded from historically-ECMAScript rules — no FP.
        assert!(!req.admits(Language::Python));
    }

    #[test]
    fn any_rule_admits_python_only_up_to_its_maturity_stage() {
        // A language-agnostic import-cycle style rule at Semantic stage runs
        // on Python (Preview supplies Semantic).
        let semantic = RuleRequirements {
            languages: LanguageSupport::Any,
            minimum_stage: AnalysisStage::Semantic,
        };
        assert!(semantic.admits(Language::Python));

        // A rule needing the call graph does NOT run on Python yet: Preview
        // tops out at Semantic. Skipped automatically — no special casing.
        let callgraph = RuleRequirements {
            languages: LanguageSupport::Any,
            minimum_stage: AnalysisStage::CallGraph,
        };
        assert!(!callgraph.admits(Language::Python));
        // …but it runs on TypeScript (Enterprise).
        assert!(callgraph.admits(Language::TypeScript));
    }
}
