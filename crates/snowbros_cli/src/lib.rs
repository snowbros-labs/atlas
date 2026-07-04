//! SNOWBROS Inspector CLI implementation.
//!
//! Installed as both `snowbros` and the short alias `sb` — two thin binary
//! entry points call [`run`]. All analysis logic lives in library crates;
//! this crate only parses arguments, dispatches commands, and formats
//! output.

mod commands;
pub mod pipeline;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Deterministic engineering intelligence for your codebase.
#[derive(Parser)]
#[command(name = "snowbros", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a starter `snowbros.toml` in the current directory.
    Init {
        /// Overwrite an existing `snowbros.toml`.
        #[arg(long)]
        force: bool,
    },
    /// Analyze a project and report engineering issues.
    Analyze {
        /// Project root to analyze (defaults to the current directory).
        path: Option<camino::Utf8PathBuf>,
        /// Output format.
        #[arg(long, value_enum, default_value = "terminal")]
        format: commands::analyze::Format,
        /// CI gate: exit with code 2 when findings of severity High or
        /// above exist.
        #[arg(long)]
        ci: bool,
        /// Ignore and don't write the incremental cache (force cold run).
        #[arg(long)]
        no_cache: bool,
    },
    /// Watch the project and print finding changes as they happen.
    Watch {
        /// Project root to watch (defaults to the current directory).
        path: Option<camino::Utf8PathBuf>,
    },
    /// Explain a rule: what it detects, why, and how to fix findings.
    Explain {
        /// Rule id, e.g. `security/no-eval`.
        rule_id: String,
    },
    /// Export the project's semantic graph.
    Graph {
        /// Project root (defaults to the current directory).
        path: Option<camino::Utf8PathBuf>,
        /// Export format.
        #[arg(long, value_enum, default_value = "dot")]
        format: commands::graph::GraphFormat,
    },
}

/// Parses CLI arguments and runs the selected command.
pub fn run() -> ExitCode {
    // Observability: RUST_LOG=snowbros::cache=debug shows hit/miss
    // decisions on stderr without touching report output on stdout.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init();

    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init { force } => commands::init::run(force),
        Command::Analyze {
            path,
            format,
            ci,
            no_cache,
        } => commands::analyze::run(path, format, ci, no_cache),
        Command::Watch { path } => commands::watch::run(path),
        Command::Explain { rule_id } => commands::explain::run(&rule_id),
        Command::Graph { path, format } => commands::graph::run(path, format),
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
