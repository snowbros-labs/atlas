//! Shared analysis pipeline used by `analyze` and `graph`:
//! scan → detect frameworks → parse (parallel) → extract imports →
//! resolve → build semantic graph.
//!
//! Parsing is file-parallel via rayon; results are collected in scan
//! order, so the resulting graph is identical regardless of thread
//! scheduling.

use std::fs;

use camino::Utf8PathBuf;
use rayon::prelude::*;

use snowbros_framework::{detect_frameworks, DetectedFramework, ProjectFacts};
use snowbros_graph::{EdgeKind, Node, SemanticGraph};
use snowbros_parser::{extract_imports, parse, Import};
use snowbros_resolver::{resolve, FileSet, Resolution, TsPaths};
use snowbros_rules::UnresolvedImport;
use snowbros_scanner::{scan, ScanResult};

/// Everything the pipeline produces.
pub struct Pipeline {
    /// Scan result (all files, sorted).
    pub scanned: ScanResult,
    /// Detected frameworks.
    pub frameworks: Vec<DetectedFramework>,
    /// Project facts (includes parsed package.json).
    pub facts: ProjectFacts,
    /// The semantic graph.
    pub graph: SemanticGraph,
    /// Imports the resolver could not map anywhere.
    pub unresolved: Vec<UnresolvedImport>,
    /// Files that could not be read or parsed (path: reason).
    pub parse_failures: Vec<String>,
}

/// Runs the pipeline on a project root.
pub fn build(root: &Utf8PathBuf) -> Result<Pipeline, String> {
    if !root.is_dir() {
        return Err(format!("`{root}` is not a directory"));
    }

    let scanned = scan(root);
    let facts = ProjectFacts::from_dir(root);
    let frameworks = detect_frameworks(&facts);
    let file_set: FileSet = scanned.files.iter().map(|f| f.path.clone()).collect();
    let aliases = TsPaths::load(root);

    // Parse in parallel; collect() preserves input order, keeping the
    // graph construction below fully deterministic.
    type Parsed = (Utf8PathBuf, Result<Vec<Import>, String>);
    let parsed_files: Vec<Parsed> = scanned
        .ecmascript_files()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|file| {
            let imports = fs::read_to_string(root.join(&file.path))
                .map_err(|e| format!("unreadable: {e}"))
                .and_then(|source| {
                    let language = file.language.expect("ecmascript_files guarantees language");
                    parse(source, language)
                        .map(|parsed| extract_imports(&parsed))
                        .map_err(|e| e.to_string())
                });
            (file.path.clone(), imports)
        })
        .collect();

    let mut graph = SemanticGraph::new();
    let mut unresolved = Vec::new();
    let mut parse_failures = Vec::new();

    for (path, imports) in parsed_files {
        let from_id = graph.add_node(Node::file(path.clone()));
        let imports = match imports {
            Ok(imports) => imports,
            Err(reason) => {
                parse_failures.push(format!("{path}: {reason}"));
                continue;
            }
        };
        for import in imports {
            match resolve(&path, &import.specifier, &file_set, &aliases) {
                Resolution::Project(target) => {
                    let to_id = graph.add_node(Node::file(target));
                    graph.add_edge(from_id, to_id, EdgeKind::Imports);
                }
                Resolution::External(pkg) => {
                    let pkg_id = graph.add_node(Node::package(pkg, None));
                    graph.add_edge(from_id, pkg_id, EdgeKind::DependsOn);
                }
                Resolution::Unresolved(specifier) => {
                    unresolved.push(UnresolvedImport {
                        file: path.clone(),
                        specifier,
                        span: import.span,
                    });
                }
            }
        }
    }

    Ok(Pipeline {
        scanned,
        frameworks,
        facts,
        graph,
        unresolved,
        parse_failures,
    })
}
