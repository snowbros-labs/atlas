# Snowbros Atlas v0.1.0 — first public release

Snowbros Atlas is a deterministic static-analysis engine for
JavaScript/TypeScript projects, written in Rust. It builds a semantic
model of your whole project — files, imports, exports, environment
variables, frameworks — and reports engineering problems with evidence.

**Same code in, same findings out, every time.** No AI in the analysis
loop; warm-cache runs are byte-identical to cold runs, proven by tests.

## Highlights

- **11 rules** across security, architecture, imports, dependencies,
  environment, and Next.js server/client boundaries — including
  `next/server-only-in-client`, which tracks server-only code into client
  components through aliases and re-export chains, with the full import
  chain as evidence.
- **Fast:** ~270 ms cold / ~43 ms warm on a 500-file repo; single-file
  change re-analysis in ~34 ms (release build, incremental cache).
- **Auto-fix** (`sb fix`): guarded, deterministic, idempotent text edits
  for unused dependencies and unused env vars. Never guesses; skips
  drifted files.
- **Editor support:** built-in LSP server (`sb lsp`) — diagnostics in any
  LSP-capable editor.
- **CI-native:** SARIF 2.1.0 for GitHub code scanning, `--ci` exit gate,
  JSON/Markdown/HTML reports, explainable 0–100 health scorecard.
- **Watch mode** (`sb watch`): continuous analysis printing only deltas.
- **`sb explain <rule>`:** every rule documents its detection logic and
  false-positive guards.

## Install

```sh
npx snowbros analyze                     # npm / npx
brew install snowbros/tap/snowbros-atlas       # Homebrew
cargo install snowbros-atlas --locked          # from source
# shell / PowerShell installers + prebuilt archives on this release page
```

Prebuilt for Windows x64, macOS x64/arm64, Linux x64/arm64. All archives
ship SHA-256 checksums.

## Known limitations (0.1)

- JS/TS/JSX/TSX analysis only (23 languages detected, 4 parsed deeply).
- File-level resolution; symbol-level call graph is on the roadmap.
- Monorepos with multiple package.json files are treated as one project.

Full changelog: see CHANGELOG.md.
