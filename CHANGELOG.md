# Changelog

All notable changes to Snowbros Atlas are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com) and the
project adheres to [Semantic Versioning](https://semver.org).

## [0.4.0] - 2026-07-14

Multi-language foundation. Atlas grows from a JavaScript/TypeScript analyzer
into a multi-language semantic platform: every language lowers into one
shared Atlas IR, and rules are either language-agnostic or explicitly scoped
to a language family — never a `if language == …` branch inside a detector.
Python is the first new language, and the first cross-language rule proves
the IR carries real diagnostics beyond ECMAScript.

### Features

- **Python support.** A `tree-sitter-python` frontend lowers Python into the
  shared IR: functions, classes, module-level bindings, decorated
  definitions, and imports (relative and absolute). A dedicated Python
  import resolver follows Python's module rules — relative imports to sibling
  modules and package `__init__.py`, absolute first-party imports (including
  a package imported by its own name when the scan root *is* that package),
  and standard-library / installed packages recognized as external and never
  flagged.
- **Language frontends.** Parsing is dispatched through a `FrontendRegistry`;
  a new language is added by registering a `LanguageFrontend`, with no edit
  to the pipeline. The ECMAScript family (JS/JSX/TS/TSX) is one frontend;
  Python is another.
- **Rule execution contract.** Each rule declares the languages and analysis
  stage it needs (`RuleRequirements`); the scheduler runs a rule against a
  file only when the file's language is supported and its frontend is mature
  enough. Policy lives in one place — no language checks in rule bodies.
- **`complexity/large-function` (Low/Possible, nursery).** The first
  language-neutral rule: it reads only the shared IR (function body line
  span), so it flags over-long top-level functions in TypeScript,
  JavaScript, and Python with a single implementation.
- **Cross-language rules.** `graph/no-circular-imports`, `graph/dead-file`
  (with Python entry-point exclusions), and `imports/unresolved-import` now
  run on Python as well as the ECMAScript family.

### Fixed

- **Absolute self-package Python imports.** When scanning a package
  directory directly (e.g. `fastapi/`), absolute imports leading with the
  package's own name (`from fastapi.encoders import x`) now resolve to the
  project file instead of being treated as external — eliminating false
  dead-file findings for internally-used modules.

### Notes

- 23 built-in rules. Python ships at `preview` maturity; existing JS/TS
  analysis is byte-identical (verified warm-vs-cold and across the M0–M2
  integration suites). The VS Code extension remains at 0.3.0 (LSP-compatible).

## [0.3.0] - 2026-07-11

Atlas' first semantic TypeScript analysis engine. The analyzer moves from
file-level facts to a resolved, project-wide symbol model over Atlas IR:
cross-file symbol resolution, a call graph, and type-level nodes, harvested
into new zero- and low-false-positive diagnostics.

### Features

- **Call graph.** Lowering resolves each call to its enclosing top-level
  function (`Call.in_symbol`); the semantic layer builds caller → callee
  edges, intra-file and across files (via named imports). Member calls and
  aliased/default imports are left unresolved by design — accuracy over
  quantity.
- **TypeScript type IR.** Interfaces, type aliases, and enums are lowered as
  first-class symbols, with interface members, `extends` heritage (kept
  separate from member type references), and enum members.
- **Reference tracking.** Lowering records identifier/type uses, powering
  reachability analysis without false positives from callback/value uses.
- **Three new rules:**
  - `typescript/circular-type-reference` (High/Certain) — a cycle of
    interfaces connected by `extends` heritage; a guaranteed TS2310 error,
    so zero false positives. Member-annotation recursion is legal and never
    flagged.
  - `typescript/unreachable-symbol` (Low/Likely) — a non-exported top-level
    declaration referenced nowhere in its module; provably dead code.
  - `imports/broken-path-alias` (Medium/Likely) — a specifier that matches a
    configured tsconfig `paths` alias but resolves to no file (a typo or
    moved target), distinct from an ordinary missing module.
- **Richer symbol graph.** `sb graph --symbols` now renders the full
  semantic surface — declaration kinds plus `Contains` / `Exports` / `Calls`
  (intra- and cross-file) / `TypeRef` (interface inheritance) edges — a
  tangible way to inspect the engine.

### Internal

- Cache format bumped to v8: v6/v7 caches carry IR without `Call.in_symbol`,
  `Module.references`, or the type-node data and are cleanly discarded.
- The existing eleven rules read the file-level rule graph, not the symbol
  graph, so every prior diagnostic and all five output formats remain
  byte-identical. Verified on a real Next.js/TypeScript codebase: the three
  new rules produced zero false positives.

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

