# Changelog

All notable changes to the Snowbros Atlas VS Code extension are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project uses [Semantic Versioning](https://semver.org/).

## [0.1.0] — Unreleased

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
