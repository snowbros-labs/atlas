//! Shared analysis pipeline used by `analyze`, `graph`, and `watch`:
//! scan → detect frameworks → parse (parallel, cache-aware) → extract
//! imports → resolve → build semantic graph.
//!
//! Incrementality: parse results are cached per file (see
//! `snowbros_cache`); unchanged files skip parsing entirely. The graph
//! is rebuilt from (cached or fresh) import lists every run — cheap
//! in-memory work that keeps results provably identical to a cold run.
//! Parsing is file-parallel via rayon; collection preserves scan order,
//! so output is deterministic under any thread scheduling.

use std::fs;

use camino::Utf8PathBuf;
use rayon::prelude::*;
use tracing::debug;

use snowbros_cache::FileFingerprint;
use snowbros_cache::{config_fingerprint, hash_bytes, CacheData, CacheStats, FileEntry, Lookup};
use snowbros_framework::{detect_frameworks, DetectedFramework, ProjectFacts};
use snowbros_graph::{EdgeKind, Node, SemanticGraph};
use snowbros_parser::{extract_imports, parse};
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
    /// Cache hit/miss counters for this run.
    pub cache_stats: CacheStats,
}

/// Runs the pipeline on a project root. `use_cache: false` forces a
/// cold run and skips persisting.
pub fn build(root: &Utf8PathBuf, use_cache: bool) -> Result<Pipeline, String> {
    if !root.is_dir() {
        return Err(format!("`{root}` is not a directory"));
    }

    let scanned = scan(root);
    let facts = ProjectFacts::from_dir(root);
    let frameworks = detect_frameworks(&facts);
    let file_set: FileSet = scanned.files.iter().map(|f| f.path.clone()).collect();
    let aliases = TsPaths::load(root);

    let config_fp = config_fingerprint(root.as_std_path(), env!("CARGO_PKG_VERSION"));
    let cache = if use_cache {
        CacheData::load(root.as_std_path(), &config_fp)
    } else {
        CacheData::empty(config_fp.clone())
    };

    // Parse in parallel; collect() preserves input order, keeping graph
    // construction below fully deterministic. Each file yields its new
    // cache entry plus whether the cache served it.
    type PerFile = (Utf8PathBuf, FileEntry, bool);
    let per_file: Vec<PerFile> = scanned
        .ecmascript_files()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|file| {
            let abs = root.join(&file.path);
            let (entry, hit) = match cache.lookup(&file.path, abs.as_std_path()) {
                Lookup::Fresh(entry) => {
                    debug!(target: "snowbros::cache", path = %file.path, "cache hit");
                    (entry, true)
                }
                Lookup::Stale(content) => {
                    debug!(target: "snowbros::cache", path = %file.path, "cache miss — parsing");
                    let content = match content {
                        Some(c) => Ok(c),
                        None => fs::read_to_string(&abs).map_err(|e| format!("unreadable: {e}")),
                    };
                    let fingerprint =
                        FileFingerprint::read(abs.as_std_path()).unwrap_or(FileFingerprint {
                            size: 0,
                            mtime_ms: 0,
                        });
                    let entry = match content {
                        Ok(source) => {
                            let content_hash = hash_bytes(source.as_bytes());
                            let language =
                                file.language.expect("ecmascript_files guarantees language");
                            match parse(source, language) {
                                Ok(parsed) => FileEntry {
                                    fingerprint,
                                    content_hash,
                                    imports: Some(extract_imports(&parsed)),
                                    failure: None,
                                },
                                Err(e) => FileEntry {
                                    fingerprint,
                                    content_hash,
                                    imports: None,
                                    failure: Some(e.to_string()),
                                },
                            }
                        }
                        Err(reason) => FileEntry {
                            fingerprint,
                            content_hash: String::new(),
                            imports: None,
                            failure: Some(reason),
                        },
                    };
                    (entry, false)
                }
            };
            (file.path.clone(), entry, hit)
        })
        .collect();

    // Rebuild graph from (cached or fresh) import lists, and assemble
    // the next cache generation. Entries for deleted/renamed files drop
    // out automatically because only currently-scanned files are kept.
    let mut graph = SemanticGraph::new();
    let mut unresolved = Vec::new();
    let mut parse_failures = Vec::new();
    let mut stats = CacheStats::default();
    let mut next_cache = CacheData::empty(config_fp);

    for (path, entry, hit) in per_file {
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
        }
        let from_id = graph.add_node(Node::file(path.clone()));
        match &entry.imports {
            Some(imports) => {
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
                                specifier: specifier.clone(),
                                span: import.span,
                            });
                        }
                    }
                }
            }
            None => {
                if let Some(reason) = &entry.failure {
                    parse_failures.push(format!("{path}: {reason}"));
                }
            }
        }
        next_cache.files.insert(path, entry);
    }

    // Persist only when something actually changed — a fully-warm run
    // must not pay serialization and disk-write costs.
    if use_cache && next_cache != cache {
        next_cache.save(root.as_std_path());
    }
    debug!(
        target: "snowbros::cache",
        hits = stats.hits,
        misses = stats.misses,
        "pipeline complete"
    );

    Ok(Pipeline {
        scanned,
        frameworks,
        facts,
        graph,
        unresolved,
        parse_failures,
        cache_stats: stats,
    })
}
