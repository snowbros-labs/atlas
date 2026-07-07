# Changelog

All notable changes to Snowbros Atlas are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the
project adheres to [Semantic Versioning](https://semver.org).

## [0.2.2] - 2026-07-07

### Bug Fixes

- **`snowbros fix` now guards generic span replacements against file
  drift.** Planned `ReplaceBytes` edits capture the span's current bytes
  at plan time and application refuses the edit when the file changed
  since analysis — previously only bounds were checked, so a same-length
  drift could be silently mis-patched.
- **Accurate skip reporting on write failure.** When a file write failed,
  fixes that had already been skipped for other reasons were reported a
  second time as "write failed"; only the fixes actually rolled back are
  now reported.
- **VS Code extension (0.2.2):** CLI commands are spawned through the
  shell so Windows `.cmd` shims work (fixes `spawn EINVAL`).

### Miscellaneous

- Anchored the git-cliff `tag_pattern` so VS Code extension tags
  (`vscode-v*`) no longer leak into CLI release notes.

## [0.2.1] - 2026-07-06

### Features

- **React component and hook model (M1).** Lowering now records whether a
  function returns JSX, and a new semantic `react` module classifies
  symbols into components (JSX-returning, PascalCase or default export)
  and custom hooks (`useX`). Purely structural — read from Atlas IR, not
  the tree-sitter tree.
- **Four React rules**, all under the additive `react` category:
  - `react/async-client-component` — an `async` component in a
    `"use client"` file (invalid; errors at runtime).
  - `react/hook-in-non-component` — a hook call outside a component or
    custom hook (the first Rule of Hooks), resolved via minimal call
    enclosure.
  - `react/hook-returns-jsx` — a `useX` hook that returns JSX (a
    mislabeled component).
  - `react/component-naming` — a JSX-returning function that is not
    PascalCase (nursery).

### Internal

- Cache format bumped to v6 so a v5 cache cannot serve a stale
  `returns_jsx` value on a warm run.

## [0.2.0] - 2026-07-06

### Features

- **Atlas IR + semantic pipeline (M0).** A new language-agnostic
  intermediate representation (`snowbros_ir`), parser lowering
  (tree-sitter → IR), a project symbol model (`snowbros_semantic`), and a
  Next.js project model (`snowbros_framework::nextjs`) are now built on
  every analysis and wired into the engine. Additive: the existing rules,
  the default JSON/SARIF output, and the `sb graph` DOT export are
  byte-identical.
- **Proof rules over the new layers:** `typescript/unused-export`,
  `typescript/duplicate-declaration`, `next/mixed-router`, and
  `next/client-metadata-ignored`.
- **New CLI:** `sb model` (prints the Next.js project model as JSON) and
  `sb graph --symbols` (exports the symbol-level graph).
- **Optional output:** `sb analyze --project-model` attaches the framework
  project model as an opt-in top-level `project_model` JSON key; the
  default report is unchanged.

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

