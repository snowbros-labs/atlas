//! Snowbros Atlas CLI implementation.
//!
//! Installed as both `snowbros` and the short alias `sb` — two thin binary
//! entry points call [`run`]. All analysis logic lives in library crates;
//! this crate only parses arguments, dispatches commands, and formats
//! output.

mod commands;
mod fixers;

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
        /// Attach the framework project model as the optional top-level
        /// `project_model` key (JSON output). Off by default; the default
        /// report is unchanged.
        #[arg(long)]
        project_model: bool,
    },
    /// Watch the project and print finding changes as they happen.
    Watch {
        /// Project root to watch (defaults to the current directory).
        path: Option<camino::Utf8PathBuf>,
    },
    /// Apply deterministic fixes for auto-fixable findings.
    Fix {
        /// Project root (defaults to the current directory).
        path: Option<camino::Utf8PathBuf>,
        /// Only fix findings from these rule ids (repeatable).
        #[arg(long = "rule")]
        rules: Vec<String>,
        /// Only fix findings in these files (root-relative, repeatable).
        #[arg(long = "file")]
        files: Vec<camino::Utf8PathBuf>,
        /// Show what would change without writing any file.
        #[arg(long)]
        dry_run: bool,
    },
    /// Start the Language Server Protocol server (stdio) for editors.
    Lsp {
        /// Use stdio transport (default, and only, transport). Accepted for
        /// compatibility with vscode-languageclient, which always passes
        /// `--stdio`. The value is ignored; the server always uses stdio.
        #[arg(long)]
        stdio: bool,
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
        /// Export the symbol-level graph (declared symbols and their
        /// containing files) instead of the file/package import graph.
        #[arg(long)]
        symbols: bool,
    },
    /// Print the framework project model (Next.js) as JSON.
    Model {
        /// Project root (defaults to the current directory).
        path: Option<camino::Utf8PathBuf>,
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
            project_model,
        } => commands::analyze::run(path, format, ci, no_cache, project_model),
        Command::Watch { path } => commands::watch::run(path),
        Command::Fix {
            path,
            rules,
            files,
            dry_run,
        } => commands::fix::run(path, rules, files, dry_run),
        Command::Lsp { stdio: _ } => snowbros_lsp::run_stdio().map(|()| ExitCode::SUCCESS),
        Command::Explain { rule_id } => commands::explain::run(&rule_id),
        Command::Graph {
            path,
            format,
            symbols,
        } => commands::graph::run(path, format, symbols),
        Command::Model { path } => commands::model::run(path),
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
