# Press kit

Everything needed to write about or feature Snowbros Atlas. Facts here are
verified against the repository; please keep quotes accurate.

## Naming

- **Company:** SNOWBROS
- **Product:** Snowbros Atlas (never bare "Snowbros" in prose)
- **CLI:** `sb` (alias `snowbros`)
- Always "Snowbros Atlas" on first mention; "Atlas" is fine thereafter.

## One-liner

Deterministic engineering intelligence for JavaScript/TypeScript — the
whole-project graph, boundaries, and manifest, proven not guessed.

## Short description (≤ 280 chars)

Snowbros Atlas is a fast, native static-analysis tool for JS/TS projects. It maps
the whole import graph and reports problems it can prove — circular imports, dead
files, Next.js server/client boundary leaks, unused deps, hardcoded secrets —
deterministically, with an evidence chain for every finding.

## Long description

Most linters see one file at a time. Snowbros Atlas works one layer up, on
whole-project structure: the import graph, framework boundaries, and the
dependency manifest — exactly where the expensive, long-lived bugs hide. It is a
native Rust binary with an incremental cache, and it is deterministic by design:
the same codebase and config always produce the same findings, and warm-cache
output is byte-identical to a cold run. No AI decides whether an issue exists;
every finding ships with the evidence chain that produced it and a confidence
level. Atlas runs in the terminal, in CI (SARIF + exit-code gate), and in the
editor via a built-in LSP and a VS Code extension.

## Key facts

- **Language:** written in Rust; analyzes the JS/TS family (`.js/.jsx/.ts/.tsx`
  and `.mjs/.cjs/.mts/.cts`).
- **Speed:** ~270 ms cold / ~43 ms warm / ~34 ms per changed file on a 500-file
  project.
- **Determinism:** warm output proven byte-identical to cold (enforced by tests).
- **Rules:** 11 evidence-first rules, each with documented false-positive guards.
- **Outputs:** terminal, JSON, Markdown, SARIF 2.1.0, self-contained HTML,
  health scorecard.
- **Editor:** built-in LSP (`sb lsp`) + first-party VS Code extension.
- **Distribution:** npm (`@snowbros/atlas`, `snowbros`), crates.io
  (`snowbros-atlas`), Homebrew, shell/PowerShell installers, signed multi-target
  GitHub releases.
- **License:** MIT OR Apache-2.0.

## What makes it different

- Deterministic, explainable findings with evidence chains — not heuristics.
- Whole-project semantic graph (cycles, reachability), not per-file rules.
- Next.js server/client boundary analysis through aliases and re-export chains.
- Sub-second incremental analysis from a native binary.

## Proof points (real runs)

- **zod** — finds the real circular-import cycles in `v3` and `v4/core`,
  deterministically.
- **axios** — health 97/100; flags a fixture credential, redacted to 4 chars.
- **fastify** — finds a real cycle; honestly labels package self-imports as
  unresolved rather than guessing.

Details: [docs/EXAMPLES.md](EXAMPLES.md).

## Links

- Repository: <https://github.com/snowbros-labs/atlas>
- Issues / Discussions: <https://github.com/snowbros-labs/atlas/issues> ·
  <https://github.com/snowbros-labs/atlas/discussions>
- Roadmap: [ROADMAP.md](../ROADMAP.md)

## Brand assets

In [`assets/`](../assets):

| Asset | File |
|---|---|
| Logo (SVG, light/dark) | `snowbros-logo-forest.svg`, `snowbros-logo-light.svg` |
| Logo (PNG) | `snowbros-logo-light.png`, `logo-256.png` |
| Social / OG image (1280×640) | `og-image.svg`, `og-image.png` |
| Banner | `banner.svg`, `banner.png` |
| Terminal render | `terminal.svg` |
| HTML report screenshot | `screenshot-html-report.png` |
| Architecture / pipeline / rule-engine | `architecture.svg`, `pipeline.svg`, `rule-engine.svg` |

**Palette:** forest `#24423A`, mint `#8FD8C0`, light `#F8F9F8`, deep `#0E1B17`.

## Boilerplate

> Snowbros Atlas is an open-source, deterministic static-analysis tool for
> JavaScript and TypeScript projects, built in Rust by SNOWBROS. It is dual
> licensed under MIT and Apache-2.0.
