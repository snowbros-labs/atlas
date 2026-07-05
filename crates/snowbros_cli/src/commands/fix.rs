//! `snowbros fix` — apply deterministic fixes for auto-fixable findings.
//!
//! Only fixes that are pure, guarded text edits are applied (see
//! `crate::fixers`); everything else is reported as not auto-fixable.
//! `--dry-run` shows the plan without touching any file.

use std::process::ExitCode;

use camino::Utf8PathBuf;
use owo_colors::OwoColorize;

use crate::fixers;

/// Runs analysis, plans fixes, and applies (or previews) them.
///
/// `rules` / `files` filter which findings are considered; empty means
/// all.
pub fn run(
    path: Option<Utf8PathBuf>,
    rules: Vec<String>,
    files: Vec<Utf8PathBuf>,
    dry_run: bool,
) -> Result<ExitCode, String> {
    let root = match path {
        Some(p) => p,
        None => Utf8PathBuf::from_path_buf(
            std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?,
        )
        .map_err(|p| format!("non-UTF-8 working directory: {}", p.display()))?,
    };

    let analysis = snowbros_engine::analyze(&root, true)?;
    let mut diagnostics = analysis.report.diagnostics;

    // Scope filters.
    if !rules.is_empty() {
        diagnostics.retain(|d| rules.iter().any(|r| r == &d.rule_id));
    }
    if !files.is_empty() {
        diagnostics.retain(|d| files.iter().any(|f| f == &d.location.file));
    }

    let plan = fixers::plan(&diagnostics);
    if plan.fixes.is_empty() {
        println!(
            "{} nothing to fix ({} finding(s), {} not auto-fixable)",
            "✓".green().bold(),
            diagnostics.len(),
            plan.unfixable
        );
        return Ok(ExitCode::SUCCESS);
    }

    let mode = if dry_run { "would apply" } else { "applying" };
    println!(
        "{} {mode} {} fix(es):",
        if dry_run {
            "○".bold().to_string()
        } else {
            "◆".bold().to_string()
        },
        plan.fixes.len()
    );
    for fix in &plan.fixes {
        println!(
            "  {} {} {}",
            fix.file.to_string().bold(),
            fix.description,
            format!("[{}]", fix.rule_id).dimmed()
        );
    }

    let outcome = fixers::apply(&root, &plan.fixes, dry_run);

    println!();
    let verb = if dry_run { "would change" } else { "changed" };
    println!(
        "{} {} fix(es) {} {} file(s); {} skipped; {} finding(s) not auto-fixable",
        if outcome.skipped.is_empty() {
            "✓".green().bold().to_string()
        } else {
            "!".yellow().bold().to_string()
        },
        outcome.applied,
        verb,
        outcome.files_changed.len(),
        outcome.skipped.len(),
        plan.unfixable
    );
    for (fix, reason) in &outcome.skipped {
        println!("  skipped {} — {reason}", fix.file);
    }
    if dry_run {
        println!("{}", "  (dry run: no files were written)".dimmed());
    }
    Ok(ExitCode::SUCCESS)
}
