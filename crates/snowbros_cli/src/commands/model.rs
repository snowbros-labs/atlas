//! `snowbros model` — print the framework project model.
//!
//! Reads the engine's existing pipeline output (no recomputation) and
//! emits the Next.js [`NextProjectModel`] as canonical JSON. Emits `null`
//! for projects that are not routed Next.js apps, so the output is always
//! valid, machine-parseable JSON.
//!
//! [`NextProjectModel`]: snowbros_framework::nextjs::NextProjectModel

use std::process::ExitCode;

use camino::Utf8PathBuf;

/// Builds the project model and prints it as JSON.
pub fn run(path: Option<Utf8PathBuf>) -> Result<ExitCode, String> {
    let root = match path {
        Some(p) => p,
        None => Utf8PathBuf::from_path_buf(
            std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?,
        )
        .map_err(|p| format!("non-UTF-8 working directory: {}", p.display()))?,
    };
    let pipeline = snowbros_engine::pipeline::build(&root, true)?;
    let json = serde_json::to_string_pretty(&pipeline.next_model)
        .map_err(|e| format!("failed to serialize project model: {e}"))?;
    if pipeline.next_model.is_none() {
        eprintln!("note: no Next.js router detected — emitting null");
    }
    println!("{json}");
    Ok(ExitCode::SUCCESS)
}
