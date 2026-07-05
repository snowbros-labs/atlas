# Versioning & compatibility policy

Snowbros Atlas follows [Semantic Versioning](https://semver.org/) — with one
project-specific clarification about what "the API" means for a static-analysis
tool.

## What counts as the public surface

For SemVer purposes, the public surface of Atlas is:

1. **The CLI** — command names, flags, and documented exit codes.
2. **The machine-readable outputs** — the JSON report schema and the SARIF
   output. These are consumed by CI and other tools and are treated as an API.
3. **The configuration format** — `snowbros.toml` keys and their meaning.
4. **Rule identities** — a rule id (e.g. `graph/no-circular-imports`) will not be
   silently repurposed to mean something else.

The Rust crates are **not** a stability guarantee while on `0.x`; use the CLI and
its outputs for integration.

## What versions mean

While Atlas is on **0.x** (pre-1.0):

- **Patch** (`0.1.0 → 0.1.1`): bug fixes, false-positive fixes, docs. No new
  flags or schema changes.
- **Minor** (`0.1.0 → 0.2.0`): new rules, new commands/flags, additive schema
  fields, and — because we are pre-1.0 — occasionally a breaking change to the
  CLI or schema, always called out in the changelog and release notes.

After **1.0**, breaking changes to the public surface will bump the **major**
version, with deprecations announced at least one minor release ahead where
practical.

## Findings are not part of SemVer

A new rule, or an accuracy fix that changes which findings appear, is **not** a
breaking change even though it may change your report or CI exit status. This is
by design: Atlas is meant to find more real issues over time.

To keep CI stable across upgrades:

- Pin the version you install (npm/cargo/brew all support pinning).
- Gate CI with `sb analyze --ci`, and use `snowbros.toml` thresholds and
  rule enable/disable to control what fails the build.
- Read the [CHANGELOG](CHANGELOG.md) before bumping.

## Supported versions

Security fixes target the most recent release; see [SECURITY.md](SECURITY.md).

## Platform compatibility

Prebuilt binaries and packages are published for:

| Platform | Target |
|---|---|
| macOS | x86_64, aarch64 (Apple Silicon) |
| Linux | x86_64, aarch64 |
| Windows | x86_64 (MSVC) |

Node (for the `npm`/`npx` wrapper) is supported on **≥ 18**. The VS Code
extension targets VS Code **≥ 1.85**. Analysis covers the JavaScript/TypeScript
family (`.js`, `.jsx`, `.ts`, `.tsx`, and their `.mjs`/`.cjs`/`.mts`/`.cts`
variants); other languages are detected but not deeply analyzed.
