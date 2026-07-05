//! `snowbros watch` — continuous analysis.
//!
//! Watches the project, reruns the (cache-accelerated) pipeline on
//! changes, and prints only the *delta*: new findings and resolved
//! findings. Stop with Ctrl+C.

use std::collections::BTreeSet;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use owo_colors::OwoColorize;

use snowbros_core::Diagnostic;
use snowbros_output::Report;

/// Path segments whose events are never analysis-relevant.
const IGNORED_SEGMENTS: &[&str] = &[".snowbros", ".git", "node_modules", "target", "dist"];

/// Stable identity of a finding, for diffing across runs.
fn key(d: &Diagnostic) -> String {
    format!(
        "{}|{}|{}:{}|{}",
        d.rule_id,
        d.location.file,
        d.location.span.start.line,
        d.location.span.start.column,
        d.title
    )
}

/// One analysis pass; returns the report and the cache stats line.
fn pass(root: &Utf8PathBuf) -> Result<(Report, String), String> {
    let started = Instant::now();
    let analysis = snowbros_engine::analyze(root, true)?;
    let stats = format!(
        "{} reused, {} parsed, {} ms",
        analysis.pipeline.cache_stats.hits,
        analysis.pipeline.cache_stats.misses,
        started.elapsed().as_millis()
    );
    Ok((analysis.report, stats))
}

fn print_finding(prefix: &str, d: &Diagnostic) {
    println!(
        "  {prefix} [{}] {} — {} ({}:{})",
        d.severity,
        d.title,
        d.rule_id.dimmed(),
        d.location.file,
        d.location.span.start.line
    );
}

/// Runs watch mode until interrupted.
pub fn run(path: Option<Utf8PathBuf>) -> Result<ExitCode, String> {
    let root = match path {
        Some(p) => p,
        None => Utf8PathBuf::from_path_buf(
            std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?,
        )
        .map_err(|p| format!("non-UTF-8 working directory: {}", p.display()))?,
    };

    // Initial full pass.
    let (report, stats) = pass(&root)?;
    println!(
        "{} watching {root} — initial: {} finding(s), health {}/100 ({stats})",
        "◉".bold(),
        report.summary.total,
        report.scorecard.overall
    );
    for d in &report.diagnostics {
        print_finding("•", d);
    }
    println!("{}", "  (Ctrl+C to stop)".dimmed());

    let mut previous: BTreeSet<String> = report.diagnostics.iter().map(key).collect();
    let mut prev_report = report;

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(300), tx)
        .map_err(|e| format!("cannot start watcher: {e}"))?;
    debouncer
        .watcher()
        .watch(root.as_std_path(), RecursiveMode::Recursive)
        .map_err(|e| format!("cannot watch {root}: {e}"))?;

    for result in rx {
        let events = match result {
            Ok(events) => events,
            Err(e) => {
                eprintln!("watch error: {e}");
                continue;
            }
        };
        let relevant = events.iter().any(|event| {
            let p = event.path.to_string_lossy().replace('\\', "/");
            !IGNORED_SEGMENTS
                .iter()
                .any(|seg| p.split('/').any(|part| part == *seg))
        });
        if !relevant {
            continue;
        }

        let (report, stats) = match pass(&root) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("analysis error: {e}");
                continue;
            }
        };
        let current: BTreeSet<String> = report.diagnostics.iter().map(key).collect();

        let new: Vec<&Diagnostic> = report
            .diagnostics
            .iter()
            .filter(|d| !previous.contains(&key(d)))
            .collect();
        let resolved: Vec<&Diagnostic> = prev_report
            .diagnostics
            .iter()
            .filter(|d| !current.contains(&key(d)))
            .collect();

        if new.is_empty() && resolved.is_empty() {
            println!("{} no finding changes ({stats})", "↻".dimmed());
        } else {
            println!(
                "{} {} new, {} resolved — health {}/100 ({stats})",
                "↻".bold(),
                new.len(),
                resolved.len(),
                report.scorecard.overall
            );
            for d in &new {
                print_finding(&"+".red().bold().to_string(), d);
            }
            for d in &resolved {
                print_finding(&"-".green().bold().to_string(), d);
            }
        }

        previous = current;
        prev_report = report;
    }
    Ok(ExitCode::SUCCESS)
}
