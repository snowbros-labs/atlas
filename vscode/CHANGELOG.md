# Changelog

All notable changes to the Snowbros Atlas VS Code extension are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project uses [Semantic Versioning](https://semver.org/).

## [0.3.0]

### Changed

- Tracks Atlas CLI 0.3.0 — the first semantic TypeScript engine. New
  diagnostics surface automatically through the existing language server
  (per-file `publishDiagnostics`): `typescript/circular-type-reference`,
  `typescript/unreachable-symbol`, and `imports/broken-path-alias`. No
  extension changes were required — the LSP and report schemas are unchanged.

## [0.2.2]

### Fixed

- Windows: `spawn EINVAL` toast from the Analyze / Open Report / Explain Rule
  / Show Health commands when Atlas was installed as an npm global (`.cmd`
  shim) or reached via the `npx` fallback. The CLI spawn now routes Windows
  batch shims through the shell (matching the language-server launch), so real
  `sb.exe` binaries still spawn directly. Diagnostics were never affected.

## [0.2.1]

### Changed

- Version realigned with the Atlas CLI/engine release `v0.2.1` (M1: React
  component/hook model and rule set). No extension behavior changes — the
  client remains fully compatible with the `sb lsp` server, the `analyze`
  JSON scorecard schema, and the `explain` command.

## [0.1.2]

### Fixed

- CI: release workflow no longer misfires cargo-dist on `vscode-*` tags;
  extension now has its own tag-triggered publish pipeline
  (`vscode-release.yml`) to the VS Code Marketplace.

## [0.1.1]

### Fixed

- Windows: `spawn EINVAL` when starting the language server via the `npx`
  fallback. Node refuses to spawn a `.cmd`/`.bat` batch script directly since
  the CVE-2024-27980 fix, so `npx.cmd` is now run through the shell.

## [0.1.0]

### Added

- First-party VS Code client for the Snowbros Atlas language server (`sb lsp`).
- Live diagnostics streamed from the Rust engine into native VS Code Errors,
  Warnings, Information, and Hints, with click-to-navigate.
- Automatic server resolution: explicit `atlas.path`, then `sb`/`snowbros` on
  PATH, then an `npx snowbros` fallback.
- Status bar item with Ready / Running / Error states and health-score display.
- Commands: Analyze Workspace, Restart Language Server, Explain Rule, Open
  Report, Show Health Score, Clear Cache.
- Settings: `atlas.enable`, `atlas.path`, `atlas.autoAnalyze`, `atlas.useCache`,
  `atlas.logLevel`, `atlas.format`, `atlas.enableStatusBar`.
- Graceful handling of missing executables, spawn failures, timeouts, and
  unexpected exits — the extension never crashes the editor.
