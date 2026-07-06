# Changelog

All notable changes to Snowbros Atlas are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the
project adheres to [Semantic Versioning](https://semver.org).

## [0.1.1] - 2026-07-06

### Bug Fixes

- Accept the standard `--stdio` flag on `snowbros lsp` for VS Code
  compatibility. vscode-languageclient launches the server as
  `snowbros lsp --stdio`; the flag is now accepted (and ignored, since
  stdio is the only transport) instead of being rejected by the CLI
  parser. Fixes the language server failing to start in the VS Code
  extension.

## [0.1.0] - 2026-07-05


### Features

- Bootstrap Snowbros Atlas workspace
- End-to-end analyze pipeline with first rule
- Tsconfig path aliases and rule engine with three rules
- Unresolved-import rule, SARIF output, CI gate, graph command
- Explainable project scoring and self-contained HTML report
- Sprint 5 — incremental cache, watch mode, benchmarks
- File facts and three new rules (forced-dynamic, env, exports)
- Security rules and snowbros.toml enforcement
- Rule metadata registry and `snowbros explain`
- Next.js server/client boundary rules (11 rules total)
- `snowbros fix` — deterministic auto-fixes
- LSP server and shared analysis engine
- Cargo-dist release pipeline with changelog automation
- Npm wrapper — npx snowbros / sb on all platforms

### Bug Fixes

- Box the fat Lookup::Fresh variant
- Collapse nested if in fix applier (clippy)
- TypeScript-ESM extension substitution in the resolver

### Documentation

- Production README, install guide, contributing, releasing
- Launch preparation — website, examples, assets, checklist

