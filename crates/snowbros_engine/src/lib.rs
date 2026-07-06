//! Shared analysis engine.
//!
//! One entry point — [`analyze`] — runs the full pipeline, loads the
//! project config, applies the rules, and returns both the raw pipeline
//! artifacts and the finished report. The CLI, the LSP server, and the
//! benchmarks all call into this crate, so every consumer is guaranteed
//! to produce identical findings for the same tree.

pub mod pipeline;

use camino::{Utf8Path, Utf8PathBuf};

use snowbros_core::Config;
use snowbros_output::Report;
use snowbros_rules::{apply_config, run_all, AnalysisContext, ContextInputs};

pub use pipeline::Pipeline;

/// Everything one analysis run produces.
pub struct Analysis {
    /// Raw pipeline artifacts (graph, facts, cache stats, …).
    pub pipeline: Pipeline,
    /// Config-filtered findings with summary and scorecard.
    pub report: Report,
}

/// Loads `<root>/snowbros.toml`, or the defaults when absent. A present
/// but invalid config is a hard error — silently ignoring it would make
/// results differ from what the user configured.
pub fn load_config(root: &Utf8Path) -> Result<Config, String> {
    let path = root.join(Config::FILE_NAME);
    if !path.exists() {
        return Ok(Config::default());
    }
    Config::load(path.as_std_path()).map_err(|e| e.to_string())
}

/// Runs the pipeline on `root`, applies all rules under the project
/// config, and returns the artifacts plus the report. `use_cache: false`
/// forces a cold run and skips persisting.
pub fn analyze(root: &Utf8PathBuf, use_cache: bool) -> Result<Analysis, String> {
    let pipeline = pipeline::build(root, use_cache)?;
    let config = load_config(root)?;
    let ctx = AnalysisContext::new(
        &pipeline.graph,
        pipeline.file_facts.clone(),
        ContextInputs {
            package_json: pipeline.facts.package_json.as_ref(),
            frameworks: &pipeline.frameworks,
            unresolved_imports: &pipeline.unresolved,
            env_declarations: &pipeline.env_declarations,
            import_bindings: &pipeline.import_bindings,
            semantic: Some(&pipeline.semantic),
            next_model: pipeline.next_model.as_ref(),
        },
    );
    let report = Report::new(apply_config(run_all(&ctx), &config));
    Ok(Analysis { pipeline, report })
}
