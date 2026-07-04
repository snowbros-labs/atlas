# SNOWBROS Inspector

Deterministic engineering intelligence for your codebase.

SNOWBROS Inspector understands an entire software project — files, symbols,
imports, routes, APIs, database schemas — and detects engineering problems
with high confidence. It behaves like a compiler for engineering issues:
same codebase in, same findings out, every time. AI never decides whether an
issue exists.

**Status: Sprint 0 — workspace skeleton.** See `ARCHITECTURE.md` for the full
design.

## Quick start

```sh
sb init                      # write a starter snowbros.toml
sb analyze                   # full analysis (terminal output)
sb analyze --format json     # canonical JSON (also: markdown, sarif, html)
sb analyze --ci              # exit 2 when High+ findings exist
sb watch                     # continuous analysis, prints only changes
sb fix --dry-run             # preview deterministic auto-fixes
sb fix                       # apply them (also: --rule ID, --file PATH)
sb graph --format dot        # semantic graph for Graphviz
sb explain security/no-eval  # full rule documentation
```

The binary installs as both `snowbros` and the short alias `sb`.
Configuration lives in `snowbros.toml` (severity/confidence thresholds,
rule enable/disable). `sb fix` applies only guarded, deterministic text
edits — findings without a mechanical fix are reported, never guessed.

## Workspace layout

| Crate | Purpose |
|---|---|
| `snowbros_core` | Shared types: Diagnostic, Severity, Confidence, Span, Config |
| `snowbros_parser` | Multi-language parsing (Tree-sitter, oxc) |
| `snowbros_resolver` | Symbol & import resolution |
| `snowbros_graph` | Semantic graph engine (petgraph) |
| `snowbros_cache` | Incremental computation cache |
| `snowbros_rules` | Rule engine: registry, patterns, auto-fix |
| `snowbros_security` | Taint analysis, vulnerability DB, secrets |
| `snowbros_deps` | Lockfiles, circular/unused dependencies |
| `snowbros_architecture` | Boundaries, layers, coupling, dead code |
| `snowbros_performance` | Bundle size, complexity, health scoring |
| `snowbros_plugin` | WASM + native plugin hosts |
| `snowbros_lsp` | Language Server Protocol server |
| `snowbros_output` | SARIF, JSON, Markdown, HTML, terminal |
| `snowbros_cli` | `snowbros` / `sb` binary |

All crates except `snowbros_core` and `snowbros_cli` are stubs; they fill in
per the sprint roadmap in `ARCHITECTURE.md` §18.

## Development

```sh
cargo test              # unit + integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## License

MIT OR Apache-2.0
