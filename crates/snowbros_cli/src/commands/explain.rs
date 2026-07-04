//! `snowbros explain RULE_ID` — print a rule's full documentation.

use std::process::ExitCode;

use owo_colors::OwoColorize;

use snowbros_rules::{rule_metadata, METADATA};

/// Prints metadata for `rule_id`, or lists all rules when it is unknown.
pub fn run(rule_id: &str) -> Result<ExitCode, String> {
    let Some(meta) = rule_metadata(rule_id) else {
        let known: Vec<&str> = METADATA.keys().map(String::as_str).collect();
        return Err(format!(
            "unknown rule `{rule_id}`. Available rules:\n  {}",
            known.join("\n  ")
        ));
    };

    println!(
        "{} {}",
        meta.id.bold(),
        format!("({})", meta.maturity_label()).dimmed()
    );
    println!("{}", meta.title.bold());
    println!();
    println!(
        "  severity: {} · confidence: {} · category: {}",
        meta.severity, meta.confidence, meta.category
    );
    if !meta.tags.is_empty() {
        println!("  tags: {}", meta.tags.join(", "));
    }
    println!();
    println!("{}", "WHAT".bold());
    println!("  {}", meta.description);
    println!("{}", "WHY".bold());
    println!("  {}", meta.rationale);
    println!("{}", "FIX".bold());
    println!("  {}", meta.recommendation);
    println!("{}", "FALSE POSITIVES".bold());
    println!("  {}", meta.false_positives);

    if !meta.bad_examples.is_empty() {
        println!();
        println!("{}", "TRIGGERS".bold());
        for ex in &meta.bad_examples {
            for line in ex.code.lines() {
                println!("  {}", line.red());
            }
            println!("  {}", format!("→ {}", ex.note).dimmed());
        }
    }
    if !meta.good_examples.is_empty() {
        println!();
        println!("{}", "DOES NOT TRIGGER".bold());
        for ex in &meta.good_examples {
            for line in ex.code.lines() {
                println!("  {}", line.green());
            }
            println!("  {}", format!("→ {}", ex.note).dimmed());
        }
    }
    for url in &meta.references {
        println!();
        println!("  {url}");
    }
    Ok(ExitCode::SUCCESS)
}
