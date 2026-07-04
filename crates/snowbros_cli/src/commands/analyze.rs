//! `snowbros analyze` — run the full pipeline and report findings.

use std::collections::BTreeMap;
use std::process::ExitCode;

use camino::{Utf8Path, Utf8PathBuf};
use owo_colors::OwoColorize;

use snowbros_core::Severity;
use snowbros_output::{json, markdown, sarif, Report};
use snowbros_rules::{run_all, AnalysisContext};

use crate::pipeline;

/// Output format for `analyze`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Human-readable colored terminal output.
    Terminal,
    /// Canonical JSON.
    Json,
    /// Markdown report.
    Markdown,
    /// SARIF v2.1.0 for code-scanning integrations.
    Sarif,
}

/// Runs the analysis pipeline on `path` (defaults to cwd).
///
/// With `ci`, exits with code 2 when any finding of severity High or
/// above exists — the CI gate.
pub fn run(path: Option<Utf8PathBuf>, format: Format, ci: bool) -> Result<ExitCode, String> {
    let root = match path {
        Some(p) => p,
        None => Utf8PathBuf::from_path_buf(
            std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?,
        )
        .map_err(|p| format!("non-UTF-8 working directory: {}", p.display()))?,
    };

    let pipeline = pipeline::build(&root)?;

    let ctx = AnalysisContext::new(
        &pipeline.graph,
        pipeline.facts.package_json.as_ref(),
        &pipeline.frameworks,
        &pipeline.unresolved,
    );
    let report = Report::new(run_all(&ctx));

    match format {
        Format::Json => println!("{}", json::render(&report)),
        Format::Markdown => println!("{}", markdown::render(&report)),
        Format::Sarif => println!("{}", sarif::render(&report)),
        Format::Terminal => {
            print_terminal(&root, pipeline.scanned.files.len(), &pipeline, &report);
        }
    }

    if ci && report.has_findings_at_least(Severity::High) {
        return Ok(ExitCode::from(2));
    }
    Ok(ExitCode::SUCCESS)
}

/// Colored terminal rendering.
fn print_terminal(root: &Utf8Path, file_count: usize, pipe: &pipeline::Pipeline, report: &Report) {
    println!("{} {}", "SNOWBROS Inspector".bold(), "· analyze".dimmed());
    println!("  root: {root}");
    println!("  files scanned: {file_count}");
    if pipe.frameworks.is_empty() {
        println!("  frameworks: none detected");
    } else {
        let list: Vec<String> = pipe
            .frameworks
            .iter()
            .map(|f| match &f.version {
                Some(v) => format!("{} {}", f.framework, v),
                None => f.framework.to_string(),
            })
            .collect();
        println!("  frameworks: {}", list.join(", "));
    }
    println!();

    if report.diagnostics.is_empty() {
        println!("{} no issues found", "✓".green().bold());
    } else {
        for d in &report.diagnostics {
            let sev = match d.severity {
                Severity::Critical | Severity::High => {
                    d.severity.to_string().red().bold().to_string()
                }
                Severity::Medium => d.severity.to_string().yellow().bold().to_string(),
                Severity::Low | Severity::Info => d.severity.to_string().dimmed().to_string(),
            };
            println!(
                "{sev} {} {}",
                d.title.bold(),
                format!("[{}]", d.rule_id).dimmed()
            );
            println!("  at {} · confidence: {}", d.location.file, d.confidence);
            for e in &d.evidence {
                println!("    - {}", e.description);
            }
            println!();
        }

        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (sev, count) in &report.summary.by_severity {
            counts.insert(sev.to_string(), *count);
        }
        let summary: Vec<String> = counts
            .iter()
            .rev()
            .map(|(s, c)| format!("{c} {s}"))
            .collect();
        println!(
            "{} {} finding(s): {}",
            "✗".red().bold(),
            report.summary.total,
            summary.join(", ")
        );
    }

    if !pipe.parse_failures.is_empty() {
        println!(
            "{} {} file(s) had parse/read problems (analyzed anyway where possible)",
            "!".yellow().bold(),
            pipe.parse_failures.len()
        );
    }
}
