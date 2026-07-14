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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use camino::Utf8PathBuf;
use rayon::prelude::*;
use tracing::debug;

use snowbros_cache::FileFingerprint;
use snowbros_cache::{config_fingerprint, hash_bytes, CacheData, CacheStats, FileEntry, Lookup};
use snowbros_framework::nextjs::{self, NextInput, NextProjectModel};
use snowbros_framework::{detect_frameworks, DetectedFramework, ProjectFacts};
use snowbros_graph::{EdgeKind, Node, NodeId, SemanticGraph};
use snowbros_parser::{FileFacts, FrontendRegistry, Language};
use snowbros_resolver::{
    resolve, resolve_python_import, FileSet, PyResolution, Resolution, TsPaths,
};
use snowbros_rules::{EnvDeclaration, ImportBinding, UnresolvedImport};
use snowbros_scanner::{scan, ScanResult};
use snowbros_semantic::{ImportedNames, SemanticModel};

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
    /// Per-file extracted facts (exports, env reads, dynamic API calls).
    pub file_facts: BTreeMap<Utf8PathBuf, FileFacts>,
    /// Variables declared in root `.env*` files.
    pub env_declarations: Vec<EnvDeclaration>,
    /// Resolved project-internal imports with the names they bind.
    pub import_bindings: Vec<ImportBinding>,
    /// The project symbol model, built over lowered Atlas IR.
    pub semantic: SemanticModel,
    /// A dedicated graph populated with symbol-level structure (file →
    /// symbol `Contains`/`Exports` edges). Kept separate from [`graph`]
    /// so every existing file/package/import analyzer — and the
    /// `sb graph` DOT export — stays byte-identical.
    ///
    /// [`graph`]: Pipeline::graph
    pub symbol_graph: SemanticGraph,
    /// The Next.js project model, when the project is a routed Next.js app.
    pub next_model: Option<NextProjectModel>,
    /// Detected language of each analyzed source file (root-relative path →
    /// language), used by the rule scheduler to gate rules by language.
    pub file_languages: BTreeMap<Utf8PathBuf, Language>,
}

/// Root env files considered declarations (`.env.example` is docs, not
/// a declaration, and is deliberately excluded).
const ENV_FILES: &[&str] = &[".env", ".env.local", ".env.development", ".env.production"];

/// Parses `NAME=value` lines (with optional `export ` prefix) from the
/// root `.env*` files.
fn read_env_declarations(root: &Utf8PathBuf) -> Vec<EnvDeclaration> {
    let mut out = Vec::new();
    for file in ENV_FILES {
        let Ok(text) = fs::read_to_string(root.join(file)) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            if let Some((name, _)) = line.split_once('=') {
                let name = name.trim();
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    out.push(EnvDeclaration {
                        name: name.to_string(),
                        file: Utf8PathBuf::from(*file),
                        line: idx as u32 + 1,
                    });
                }
            }
        }
    }
    out
}

/// Resolves one Python import into graph edges, symbol bindings, and
/// unresolved entries, using Python module resolution.
///
/// Symbol bindings are emitted only when the import names a *specific* module
/// (`from pkg.models import User`) — then the imported names are symbols in
/// that one module. A bare `from . import a, b` names submodules, not symbols,
/// so it contributes import edges but no symbol bindings. Python package
/// imports (`import os`) resolve to [`PyResolution::External`] and add no node,
/// keeping the graph free of unactionable standard-library packages.
#[allow(clippy::too_many_arguments)]
fn resolve_python(
    path: &Utf8PathBuf,
    import: &snowbros_parser::Import,
    file_set: &FileSet,
    root_package: Option<&str>,
    from_id: NodeId,
    graph: &mut SemanticGraph,
    import_bindings: &mut Vec<ImportBinding>,
    unresolved: &mut Vec<UnresolvedImport>,
) {
    match resolve_python_import(
        path,
        &import.specifier,
        &import.names,
        file_set,
        root_package,
    ) {
        PyResolution::Project(targets) => {
            for target in &targets {
                let to_id = graph.add_node(Node::file(target.clone()));
                graph.add_edge(from_id, to_id, EdgeKind::Imports);
            }
            let names_a_specific_module = !import.specifier.chars().all(|c| c == '.');
            if names_a_specific_module && targets.len() == 1 {
                let names: Vec<String> = import
                    .names
                    .iter()
                    .filter(|n| n.as_str() != "*")
                    .cloned()
                    .collect();
                if !names.is_empty() {
                    import_bindings.push(ImportBinding {
                        from: path.clone(),
                        to: targets[0].clone(),
                        names,
                    });
                }
            }
        }
        // Standard library / installed package — recognized, not flagged.
        PyResolution::External => {}
        PyResolution::Unresolved(specifier) => {
            let probe_detail = format!(
                "probed `{specifier}` for a sibling `.py` module or package \
                 `__init__.py` — no match"
            );
            unresolved.push(UnresolvedImport {
                file: path.clone(),
                specifier,
                span: import.span,
                // Python has no tsconfig aliases.
                matched_alias: false,
                probe_detail,
            });
        }
    }
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

    // When the scan root is itself a Python package (has a top-level
    // `__init__.py`), its own directory name is a package importable by that
    // name — so `from <root_package>.mod import x` resolves against the root.
    let root_package: Option<String> = file_set
        .contains(&Utf8PathBuf::from("__init__.py"))
        .then(|| root.file_name().map(str::to_string))
        .flatten();

    // Language frontends drive the per-file parse phase. The registry is
    // shared read-only across the parallel lower below (frontends are Sync).
    let registry = FrontendRegistry::default();

    let config_fp = config_fingerprint(root.as_std_path(), env!("CARGO_PKG_VERSION"));
    let cache = if use_cache {
        CacheData::load(root.as_std_path(), &config_fp)
    } else {
        CacheData::empty(config_fp.clone())
    };

    // Parse in parallel; collect() preserves input order, keeping graph
    // construction below fully deterministic. Each file yields its detected
    // language, its new cache entry, and whether the cache served it. Every
    // language a registered frontend handles is analyzed — not just the
    // ECMAScript family.
    type PerFile = (Utf8PathBuf, Language, FileEntry, bool);
    let per_file: Vec<PerFile> = scanned
        .files
        .iter()
        .filter(|f| f.language.is_some_and(|l| registry.supports(l)))
        .collect::<Vec<_>>()
        .par_iter()
        .map(|file| {
            let abs = root.join(&file.path);
            let language = file.language.expect("filtered to files with a language");
            let (entry, hit) = match cache.lookup(&file.path, abs.as_std_path()) {
                Lookup::Fresh(entry) => {
                    debug!(target: "snowbros::cache", path = %file.path, "cache hit");
                    (*entry, true)
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
                            // Dispatch the parse phase through the language
                            // frontend. It extracts facts and lowers to Atlas
                            // IR in one pass, so both ride the cache together.
                            let lowered = registry
                                .frontend_for(language)
                                .ok_or_else(|| format!("no frontend for language `{language}`"))
                                .and_then(|fe| {
                                    fe.lower_file(source, language, &file.path)
                                        .map_err(|e| e.to_string())
                                });
                            match lowered {
                                Ok(lowered) => FileEntry {
                                    fingerprint,
                                    content_hash,
                                    facts: Some(lowered.facts),
                                    ir: Some(lowered.ir),
                                    failure: None,
                                },
                                Err(e) => FileEntry {
                                    fingerprint,
                                    content_hash,
                                    facts: None,
                                    ir: None,
                                    failure: Some(e),
                                },
                            }
                        }
                        Err(reason) => FileEntry {
                            fingerprint,
                            content_hash: String::new(),
                            facts: None,
                            ir: None,
                            failure: Some(reason),
                        },
                    };
                    (entry, false)
                }
            };
            (file.path.clone(), language, entry, hit)
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
    let mut file_facts: BTreeMap<Utf8PathBuf, FileFacts> = BTreeMap::new();
    let mut import_bindings: Vec<ImportBinding> = Vec::new();
    let mut ir_modules: Vec<snowbros_ir::Module> = Vec::new();
    let mut file_languages: BTreeMap<Utf8PathBuf, Language> = BTreeMap::new();

    for (path, language, entry, hit) in per_file {
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
        }
        file_languages.insert(path.clone(), language);
        let from_id = graph.add_node(Node::file(path.clone()));
        match &entry.facts {
            Some(facts) => {
                for import in &facts.imports {
                    // Resolution is language-specific: Python modules resolve
                    // by Python's rules, everything else by the JS/TS resolver.
                    if language == Language::Python {
                        resolve_python(
                            &path,
                            import,
                            &file_set,
                            root_package.as_deref(),
                            from_id,
                            &mut graph,
                            &mut import_bindings,
                            &mut unresolved,
                        );
                        continue;
                    }
                    match resolve(&path, &import.specifier, &file_set, &aliases) {
                        Resolution::Project(target) => {
                            let to_id = graph.add_node(Node::file(target.clone()));
                            graph.add_edge(from_id, to_id, EdgeKind::Imports);
                            import_bindings.push(ImportBinding {
                                from: path.clone(),
                                to: target,
                                names: import.names.clone(),
                            });
                        }
                        Resolution::External(pkg) => {
                            let pkg_id = graph.add_node(Node::package(pkg, None));
                            graph.add_edge(from_id, pkg_id, EdgeKind::DependsOn);
                        }
                        Resolution::Unresolved(specifier) => {
                            // A specifier that expands under a tsconfig alias
                            // yet resolves nowhere is a broken alias, not a
                            // plain missing module.
                            let matched_alias = !aliases.expand(&specifier).is_empty();
                            let probe_detail = format!(
                                "probed `{specifier}` with extensions \
                                 ts/tsx/d.ts/js/jsx/mjs/cjs/json and index files \
                                 — no match"
                            );
                            unresolved.push(UnresolvedImport {
                                file: path.clone(),
                                specifier: specifier.clone(),
                                span: import.span,
                                matched_alias,
                                probe_detail,
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
        if let Some(facts) = &entry.facts {
            file_facts.insert(path.clone(), facts.clone());
        }
        if let Some(module) = &entry.ir {
            ir_modules.push(module.clone());
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

    let env_declarations = read_env_declarations(root);

    // Symbol model over lowered IR. Its graph is deliberately separate
    // from `graph` above: populating symbol nodes there would change the
    // `sb graph` DOT export and any node-count-sensitive analyzer.
    let semantic = SemanticModel::from_modules(ir_modules);
    // Cross-file call resolution input: for each importing file, the
    // unaliased named imports (`default`/`*` excluded) and the project file
    // each resolves to. Built from the already-resolved import bindings so
    // no extra resolution work is done.
    let mut imported_names: ImportedNames = BTreeMap::new();
    for binding in &import_bindings {
        let entry = imported_names.entry(binding.from.clone()).or_default();
        for name in &binding.names {
            if name == "default" || name == "*" {
                continue;
            }
            entry.insert(name.clone(), binding.to.clone());
        }
    }
    let mut symbol_graph = SemanticGraph::new();
    semantic.populate_graph_with_imports(&mut symbol_graph, &imported_names);

    // Next.js project model, built from a deterministic snapshot: the
    // scanned file list, files carrying `"use client"`, and each file's
    // exported names (which surface the Metadata API / route handlers).
    let client_files: BTreeSet<Utf8PathBuf> = file_facts
        .iter()
        .filter(|(_, f)| f.directives.iter().any(|d| d == "use client"))
        .map(|(p, _)| p.clone())
        .collect();
    let file_exports: BTreeMap<Utf8PathBuf, BTreeSet<String>> = file_facts
        .iter()
        .map(|(p, f)| {
            (
                p.clone(),
                f.exports.iter().map(|e| e.name.clone()).collect(),
            )
        })
        .collect();
    let files: Vec<Utf8PathBuf> = scanned.files.iter().map(|f| f.path.clone()).collect();
    let next_model = nextjs::build(NextInput {
        files: &files,
        client_files: &client_files,
        file_exports: &file_exports,
    });

    Ok(Pipeline {
        scanned,
        frameworks,
        facts,
        graph,
        unresolved,
        parse_failures,
        cache_stats: stats,
        file_facts,
        env_declarations,
        import_bindings,
        semantic,
        symbol_graph,
        next_model,
        file_languages,
    })
}
