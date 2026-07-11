<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/snowbros-logo-light.svg">
  <img src="assets/snowbros-logo-forest.svg" alt="Snowbros Atlas" width="88" height="88">
</picture>

# Snowbros Atlas

**Deterministic engineering intelligence for JavaScript/TypeScript codebases.**

[![CI](https://github.com/snowbros-labs/atlas/actions/workflows/ci.yml/badge.svg)](https://github.com/snowbros-labs/atlas/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/snowbros-labs/atlas)](https://github.com/snowbros-labs/atlas/releases)
[![crates.io](https://img.shields.io/crates/v/snowbros-atlas)](https://crates.io/crates/snowbros-atlas)
[![npm](https://img.shields.io/npm/v/@snowbros/atlas)](https://www.npmjs.com/package/@snowbros/atlas)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.96-orange.svg)](rust-toolchain.toml)

</div>

```sh
npx snowbros analyze
```

Snowbros Atlas maps your entire project — every import, export, env
var, and framework boundary — and reports problems it can **prove**:
circular imports, dead files, server-only code leaking into Next.js client
components, unused dependencies, hardcoded secrets. It behaves like a
compiler for engineering issues: **same codebase in, same findings out,
every time.** No AI decides whether an issue exists; every finding carries
the evidence chain that produced it.

Run on axios (431 files): **~230 ms cold, ~76 ms warm, health 97/100.**
Run on zod: it finds the two real circular-import cycles in `v3` and
`v4/core` — certain, with the cycle members listed.
More: [real-world examples](docs/EXAMPLES.md).

## Contents

- [Why Atlas?](#why-atlas)
- [Demo](#demo)
- [Features](#features)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Commands](#commands)
- [Rules](#rules)
- [Auto-fix](#auto-fix)
- [Editor integration (LSP)](#editor-integration-lsp)
- [CI integration (SARIF)](#ci-integration-sarif)
- [Performance](#performance)
- [How it compares](#how-it-compares)
- [Architecture](#architecture)
- [Roadmap](#roadmap)
- [Community](#community)
- [Support](#support)
- [Development](#development)
- [License](#license)

## Why Atlas?

Per-file linters (ESLint, Biome) see one file at a time. Whole-project structure
— the import graph, the framework boundaries, the dependency manifest — is
invisible to them. That is exactly where the expensive bugs live: a circular
import that survives for years, a `server-only` module that leaks into a client
bundle, a dependency nobody uses anymore.

Snowbros Atlas is built for that layer, and for three properties most tools in
it don't guarantee:

- **Deterministic.** Findings are a pure function of your code and config. The
  warm-cache run is byte-identical to a cold run — the cache can skip work, never
  change results. No AI, no heuristic drift, no flaky CI.
- **Provable, not guessed.** Anything the resolver can't prove is reported as
  *unresolved*, never invented. Every finding ships with the evidence chain that
  produced it and a confidence level.
- **Fast and native.** A Rust binary with an incremental cache: a 500-file repo
  analyzes in ~270 ms cold, ~34 ms after a one-file change.

It is **not** a linter replacement — run it *alongside* ESLint/Biome. Its
territory is project structure, not style.

## Demo

![sb analyze finds a server-only leak, a circular import, and a hardcoded secret — health 93/100](assets/terminal.svg)

One command surfaces a `server-only` module leaking into a Next.js client
component (with the full import chain), a circular import, and a hardcoded
secret — each with evidence, each proven.

<!-- TODO (needs a screen recording): animated GIF of `sb analyze` + `sb watch`.
     Storyboard and capture steps: docs/internal/launch/DEMO_PLAN.md.
     Save to assets/demo.gif and embed above this line. -->

### HTML report

`sb analyze --format html` writes a self-contained, shareable report — health
scorecard plus every finding and its evidence. ([sample](assets/sample-report.html))

![Snowbros Atlas HTML report](assets/screenshot-html-report.png)

## Features

- **Deterministic** — findings are a pure function of your code and config.
  Warm-cache output is byte-identical to a cold run (proven by tests).
- **Fast** — incremental cache: a 500-file repo analyzes in ~270 ms cold,
  ~43 ms warm, ~34 ms after a one-file change (release build).
- **Whole-project analysis** — semantic import graph, cycle detection,
  dead-file reachability, Next.js server/client boundary tracking through
  aliases and re-export chains.
- **Evidence, not vibes** — every finding carries the chain that produced it
  and a confidence level (`certain`, `likely`, `possible`).
- **Auto-fix** — `sb fix` applies guarded, deterministic text edits. It never
  guesses; drifted files are skipped, not clobbered.
- **Editor support** — built-in LSP server (`sb lsp`) publishes diagnostics
  in any LSP-capable editor.
- **CI-native** — SARIF 2.1.0 output for GitHub code scanning, `--ci` exit
  gate, JSON/Markdown/HTML reports, health scorecard.

## Installation

### npm (recommended for JS/TS teams)

```sh
npx snowbros analyze        # one-shot, no install
npm install -g @snowbros/atlas   # or install globally: `sb`, `snowbros`
```

### Shell installer (macOS, Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/snowbros-labs/atlas/releases/latest/download/snowbros-atlas-installer.sh | sh
```

### PowerShell installer (Windows)

```powershell
irm https://github.com/snowbros-labs/atlas/releases/latest/download/snowbros-atlas-installer.ps1 | iex
```

### Homebrew (macOS, Linux)

```sh
brew install snowbros-labs/tap/snowbros-atlas
```

### Cargo (from source)

```sh
cargo install snowbros-atlas --locked
```

### GitHub Releases

Prebuilt archives with SHA-256 checksums for Windows (x64), macOS
(x64/arm64), and Linux (x64/arm64) on the
[releases page](https://github.com/snowbros-labs/atlas/releases).

Platform-by-platform details: [docs/INSTALL.md](docs/INSTALL.md).

## Quick start

```sh
cd your-project
sb init                      # write a starter snowbros.toml
sb analyze                   # full analysis, colored terminal report
```

```text
Snowbros Atlas · analyze
  root: /work/acme-web
  files scanned: 512
  cache: 0 reused, 512 parsed
  frameworks: Next.js 15.1.0, React 19.0.0

HIGH Server-only module imported by a client component [next/server-only-in-client]
  at src/components/Dashboard.tsx · confidence: certain
    - import chain: Dashboard.tsx → lib/metrics.ts → lib/db.ts ("server-only")

✗ 1 finding(s): 1 High
◆ health: 92/100 (security 100, architecture 85, …)
```

## Commands

| Command | What it does |
|---|---|
| `sb init` | Write a starter `snowbros.toml` |
| `sb analyze` | Full analysis; `--format terminal\|json\|markdown\|sarif\|html` |
| `sb analyze --ci` | Exit code 2 when High+ findings exist (CI gate) |
| `sb analyze --no-cache` | Force a cold run |
| `sb watch` | Continuous analysis; prints only new/resolved findings |
| `sb fix` | Apply deterministic auto-fixes; `--dry-run`, `--rule ID`, `--file PATH` |
| `sb graph --format dot` | Export the semantic import graph (Graphviz) |
| `sb explain RULE_ID` | Full rule documentation in the terminal |
| `sb lsp` | LSP server over stdio for editor integration |

The binary installs as both `snowbros` and the short alias `sb`.

## Rules

| Rule | Severity | Confidence |
|---|---|---|
| `security/no-eval` | High | Certain |
| `security/hardcoded-secret` | High | Likely |
| `next/server-only-in-client` | High | Certain |
| `next/private-env-in-client` | High | Likely |
| `graph/no-circular-imports` | High | Certain |
| `typescript/circular-type-reference` | High | Certain |
| `imports/unresolved-import` | Medium | Likely |
| `imports/broken-path-alias` | Medium | Likely |
| `next/forced-dynamic` | Info | Certain |
| `deps/unused-dependency` | Low | Likely |
| `env/unused-env-var` | Low | Possible |
| `exports/unused-export` | Low | Possible |
| `typescript/unreachable-symbol` | Low | Likely |
| `graph/dead-file` | Low | Possible |

A representative slice of the 22 built-in rules; run `sb explain <rule-id>`
for any one. TypeScript rules resolve over a semantic symbol model — a
cross-file call graph and type-level IR — not just per-file facts. Inspect
that model with `sb graph --symbols` (declaration kinds, `Calls`, and
`TypeRef` inheritance edges) or `sb model`.

Run `sb explain <rule-id>` for detection logic, false-positive guards, and
fix guidance. Accuracy beats quantity: anything the resolver cannot prove is
reported as *unresolved*, never guessed.

### Configuration

`snowbros.toml`:

```toml
[analysis]
min_severity = "low"        # drop findings below this severity
min_confidence = "possible" # drop findings below this confidence

[rules]
disable = ["exports/*"]     # exact ids or category globs
enable = ["exports/unused-export"]  # enable wins over disable
```

## Auto-fix

`sb fix` plans edits first, then applies them only when the file still
matches what the analysis saw — files changed since analysis are skipped,
never guessed at. Fixes are idempotent.

```sh
$ sb fix --dry-run
○ would apply 2 fix(es):
  package.json remove unused dependency "left-pad" [deps/unused-dependency]
  .env remove unused variable OLD_API_URL [env/unused-env-var]
```

Currently auto-fixable: `deps/unused-dependency` (format-preserving
`package.json` surgery, devDependencies untouched) and `env/unused-env-var`
(guarded `.env` line removal). The fix engine is generic byte-span based;
more rules gain fixers over time.

## Editor integration (LSP)

`sb lsp` speaks the Language Server Protocol over stdio. Diagnostics carry
the rule id as code and map severities onto editor conventions
(High → Error, Medium → Warning, …). Example VS Code / Neovim wiring is in
[docs/INSTALL.md](docs/INSTALL.md#editor-lsp-setup).

## CI integration (SARIF)

```yaml
- run: sb analyze --format sarif > snowbros.sarif
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: snowbros.sarif
```

Findings appear in the GitHub Security tab with rule metadata. Use
`sb analyze --ci` to fail the build on High+ findings.

## Performance

Real open-source repositories (release build; details in
[docs/EXAMPLES.md](docs/EXAMPLES.md)):

| Repository | Files | Cold | Warm |
|---|---|---|---|
| zod | 554 | ~605 ms | ~88 ms |
| axios | 431 | ~230 ms | ~76 ms |
| fastify | 355 | ~365 ms | ~103 ms |

Criterion benchmarks on a generated 200-file TypeScript project
(~200 lines/file): ~162 ms cold, ~5.3 ms warm. Single-file change on a
500-file repo: ~34 ms. Warm output is byte-identical to cold output —
the cache can never change results, only skip work.

## How it compares

| | Snowbros Atlas | ESLint | Knip | dependency-cruiser | Madge |
|---|---|---|---|---|---|
| Whole-project semantic graph | ✅ | ❌ per-file | partial | ✅ | ✅ |
| Circular imports | ✅ certain, cycle listed | plugin | ❌ | ✅ | ✅ |
| Dead files / unused exports | ✅ | ❌ | ✅ | partial | orphans |
| Unused dependencies | ✅ + auto-fix | ❌ | ✅ | ❌ | ❌ |
| Next.js server/client boundary | ✅ chain evidence | partial plugin | ❌ | ❌ | ❌ |
| Secrets / eval | ✅ redacted | plugin | ❌ | ❌ | ❌ |
| Deterministic, evidence-first output | ✅ by design | mostly | mostly | mostly | mostly |
| SARIF / LSP / watch / health score | ✅ all built in | LSP via editor | ❌ | ❌ | ❌ |
| Runtime | native binary | Node | Node | Node | Node |

Not a linter replacement: Snowbros Atlas doesn't do stylistic or per-file
correctness rules — run it *alongside* ESLint/Biome. Its territory is
project structure: the graph, the boundaries, the manifest.

## Architecture

```
scan → detect frameworks → parse (Tree-sitter, parallel, cache-aware)
     → extract facts → resolve imports (tsconfig paths, aliases)
     → semantic graph (petgraph) → rules → report / scorecard
```

![Analysis pipeline: scan, parse, extract facts, resolve, graph, rules, report](assets/pipeline.svg)

<details>
<summary>Crate architecture</summary>

![Crate architecture — layered dependencies from core up to the CLI and LSP](assets/architecture.svg)

</details>


| Crate | Purpose |
|---|---|
| `snowbros_core` | Shared types: Diagnostic, Severity, Confidence, Span, Config |
| `snowbros_scanner` | Ignore-aware project file walk |
| `snowbros_parser` | Language detection + Tree-sitter parsing, fact extraction |
| `snowbros_framework` | Framework detection with evidence and confidence |
| `snowbros_resolver` | Import resolution: relative, tsconfig paths, aliases |
| `snowbros_graph` | Semantic graph: SCC/cycles, reachability, DOT export |
| `snowbros_cache` | Incremental cache (xxh3 + mtime, config-fingerprinted) |
| `snowbros_rules` | Rule engine, metadata registry, config filtering |
| `snowbros_engine` | One entry point: pipeline + rules + config |
| `snowbros_output` | Terminal, JSON, Markdown, SARIF 2.1.0, HTML, scorecard |
| `snowbros_lsp` | LSP server (tower-lsp, stdio) |
| `snowbros` (crates/snowbros_cli) | `snowbros` / `sb` binaries |

Full design: [ARCHITECTURE.md](ARCHITECTURE.md).

## Roadmap

Snowbros Atlas ships in phases. Phase 1 — the JS/TS analysis core, LSP, and
release pipeline — is done. Next up, tracked in the
[issue tracker](https://github.com/snowbros-labs/atlas/issues):

- **Accuracy** — `package.json#main/exports` resolution and monorepo/workspace
  awareness (the largest confirmed false-positive sources).
- **More rules** — growing the rule set, prioritized by real-world reports.
- **Editor client** — a packaged VS Code extension wrapping the existing LSP.
- **Pattern engine** — Semgrep-style rules authored without writing Rust.

Have an idea? Start a [Discussion](https://github.com/snowbros-labs/atlas/discussions).

## Community

- **[Discussions](https://github.com/snowbros-labs/atlas/discussions)** —
  questions, ideas, and showing off what Atlas found in your repo.
- **[Issues](https://github.com/snowbros-labs/atlas/issues)** — bug reports and
  feature requests (templates provided).
- **[Contributing](CONTRIBUTING.md)** — the ground rules, plus
  [how to add a rule](docs/adding-a-rule.md).
- **[Code of Conduct](CODE_OF_CONDUCT.md)** — expected behavior in all project
  spaces.

New contributors are welcome — look for
[`good first issue`](https://github.com/snowbros-labs/atlas/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22).

## Support

- **Question or usage help?** Open a
  [Discussion](https://github.com/snowbros-labs/atlas/discussions).
- **Found a bug?** File a
  [bug report](https://github.com/snowbros-labs/atlas/issues/new?template=bug_report.yml).
- **Security issue?** Report it privately — see [SECURITY.md](SECURITY.md). Do
  not open a public issue.

More: [FAQ](docs/FAQ.md) · [Roadmap](ROADMAP.md) · [Support policy](SUPPORT.md) ·
[Versioning & compatibility](VERSIONING.md) · [Press kit](docs/PRESS_KIT.md)

## Development

```sh
cargo test --workspace                          # 176 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo bench -p snowbros-atlas                         # criterion benchmarks
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for conventions (determinism rules,
evidence requirements, rule metadata) and [RELEASING.md](RELEASING.md) for
the release process.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
