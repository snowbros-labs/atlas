//! `snowbros analyze` — the full pipeline:
//! scan → detect frameworks → parse → extract imports → resolve →
//! build graph → run graph rules → report.
//!
//! First shipped rule: `graph/no-circular-imports` (Certain confidence —
//! cycles are proven by Tarjan SCC, not guessed).

use std::collections::BTreeMap;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use owo_colors::OwoColorize;

use snowbros_core::{Confidence, Diagnostic, Evidence, Position, Severity, SourceLocation, Span};
use snowbros_framework::{detect_frameworks, ProjectFacts};
use snowbros_graph::{EdgeKind, Node, SemanticGraph};
use snowbros_output::{json, markdown, Report};
use snowbros_parser::{extract_imports, parse};
use snowbros_resolver::{resolve, FileSet, Resolution};
use snowbros_scanner::scan;

/// Output format for `analyze`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Human-readable colored terminal output.
    Terminal,
    /// Canonical JSON.
    Json,
    /// Markdown report.
    Markdown,
}

/// Runs the analysis pipeline on `path` (defaults to cwd).
pub fn run(path: Option<Utf8PathBuf>, format: Format) -> Result<(), String> {
    let root = match path {
        Some(p) => p,
        None => Utf8PathBuf::from_path_buf(
            std::env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?,
        )
        .map_err(|p| format!("non-UTF-8 working directory: {}", p.display()))?,
    };
    if !root.is_dir() {
        return Err(format!("`{root}` is not a directory"));
    }

    // 1. Scan.
    let scanned = scan(&root);

    // 2. Frameworks.
    let facts = ProjectFacts::from_dir(&root);
    let frameworks = detect_frameworks(&facts);

    // 3–5. Parse, extract imports, resolve, build graph.
    let file_set: FileSet = scanned.files.iter().map(|f| f.path.clone()).collect();
    let mut graph = SemanticGraph::new();
    let mut parse_failures: Vec<String> = Vec::new();

    for file in scanned.ecmascript_files() {
        let from_id = graph.add_node(Node::file(file.path.clone()));
        let Ok(source) = fs::read_to_string(root.join(&file.path)) else {
            parse_failures.push(format!("{}: unreadable", file.path));
            continue;
        };
        let Some(language) = file.language else {
            continue;
        };
        let parsed = match parse(source, language) {
            Ok(p) => p,
            Err(e) => {
                parse_failures.push(format!("{}: {e}", file.path));
                continue;
            }
        };
        for import in extract_imports(&parsed) {
            match resolve(&file.path, &import.specifier, &file_set) {
                Resolution::Project(target) => {
                    let to_id = graph.add_node(Node::file(target));
                    graph.add_edge(from_id, to_id, EdgeKind::Imports);
                }
                Resolution::External(pkg) => {
                    let pkg_id = graph.add_node(Node::package(pkg, None));
                    graph.add_edge(from_id, pkg_id, EdgeKind::DependsOn);
                }
                Resolution::Unresolved(_) => {
                    // Unknown aliases: no edge, no guess. tsconfig paths
                    // support will resolve these.
                }
            }
        }
    }

    // 6. Rules.
    let diagnostics = circular_import_diagnostics(&graph);
    let report = Report::new(diagnostics);

    // 7. Output.
    match format {
        Format::Json => println!("{}", json::render(&report)),
        Format::Markdown => println!("{}", markdown::render(&report)),
        Format::Terminal => {
            print_terminal(&root, &scanned.files.len(), &frameworks, &report);
            if !parse_failures.is_empty() {
                println!(
                    "{} {} file(s) had parse/read problems (analyzed anyway where possible)",
                    "!".yellow().bold(),
                    parse_failures.len()
                );
            }
        }
    }
    Ok(())
}

/// Rule `graph/no-circular-imports`: every import cycle is one finding
/// anchored at the lexicographically first file of the cycle, with every
/// member listed as evidence — one root cause, not N duplicate warnings.
fn circular_import_diagnostics(graph: &SemanticGraph) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for group in graph.circular_groups() {
        let mut labels: Vec<String> = group
            .iter()
            .filter_map(|&id| graph.node(id).map(|n| n.label()))
            .collect();
        labels.sort();
        let Some(anchor) = labels.first().cloned() else {
            continue;
        };

        let mut diag = Diagnostic::new(
            "graph/no-circular-imports",
            "Circular import chain",
            format!(
                "{} files import each other in a cycle. Cycles make modules \
                 impossible to test in isolation, can break tree-shaking, and \
                 cause undefined imports at runtime under some module orders.",
                labels.len()
            ),
            "architecture",
            Severity::High,
            Confidence::Certain,
            SourceLocation::new(
                anchor,
                Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
            ),
        );
        for label in &labels {
            diag = diag.with_evidence(Evidence::note(format!("cycle member: {label}")));
        }
        diagnostics.push(diag);
    }
    diagnostics
}

/// Colored terminal rendering.
fn print_terminal(
    root: &Utf8Path,
    file_count: &usize,
    frameworks: &[snowbros_framework::DetectedFramework],
    report: &Report,
) {
    println!("{} {}", "SNOWBROS Inspector".bold(), "· analyze".dimmed());
    println!("  root: {root}");
    println!("  files scanned: {file_count}");
    if frameworks.is_empty() {
        println!("  frameworks: none detected");
    } else {
        let list: Vec<String> = frameworks
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
        return;
    }

    for d in &report.diagnostics {
        let sev = match d.severity {
            Severity::Critical | Severity::High => d.severity.to_string().red().bold().to_string(),
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
