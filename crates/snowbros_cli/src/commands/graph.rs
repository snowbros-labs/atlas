//! `snowbros graph` — export the semantic graph.

use std::process::ExitCode;

use camino::Utf8PathBuf;

/// Graph export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GraphFormat {
    /// Graphviz DOT (pipe into `dot -Tsvg`).
    Dot,
}

/// Builds the semantic graph and prints it.
pub fn run(path: Option<Utf8PathBuf>, format: GraphFormat) -> Result<ExitCode, String> {
    let root = match path {
        Some(p) => p,
        None => Utf8PathBuf::from_path_buf(
            std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?,
        )
        .map_err(|p| format!("non-UTF-8 working directory: {}", p.display()))?,
    };
    let pipeline = snowbros_engine::pipeline::build(&root, true)?;
    match format {
        GraphFormat::Dot => print!("{}", pipeline.graph.to_dot()),
    }
    Ok(ExitCode::SUCCESS)
}
