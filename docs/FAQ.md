# FAQ

### Is Snowbros Atlas a linter? Do I replace ESLint/Biome with it?

No. Atlas works one layer up from linters. ESLint and Biome are excellent at
per-file style and correctness; Atlas analyzes **whole-project structure** — the
import graph, framework boundaries, and dependency manifest. Run it *alongside*
your linter, not instead of it.

### Does it use AI / an LLM?

No. Atlas is deterministic by design: the same codebase and config always
produce the same findings. No model decides whether an issue exists. Every
finding carries the evidence chain that produced it.

### What does it actually detect?

Circular imports, dead files, unused exports and dependencies, unresolved
imports, hardcoded secrets, `eval` use, and Next.js server/client boundary
violations (including `server-only` leaks and private env vars reaching the
client). Run `sb explain <rule-id>` for the detection logic and false-positive
guards of any rule. See the [rules table](../README.md#rules).

### Which languages are supported?

Deep analysis covers the JavaScript/TypeScript family: `.js`, `.jsx`, `.ts`,
`.tsx`, and their `.mjs`/`.cjs`/`.mts`/`.cts` variants. Many other languages are
*detected* for context, but not deeply analyzed. Other language families are on
the [roadmap](../ROADMAP.md), not shipped.

### How fast is it?

Native Rust with an incremental cache. On a 500-file project: ~270 ms cold,
~43 ms warm, ~34 ms after a one-file change. Warm output is byte-identical to a
cold run — the cache only skips work, it can never change results. Real repos:
see [docs/EXAMPLES.md](EXAMPLES.md).

### Will it slow down or break my CI?

It is fast, and `sb analyze --ci` gives you a single exit-code gate (fails on
High+ findings). Because Atlas finds *more* over time, pin the version and use
`snowbros.toml` thresholds and rule toggles to control what fails the build. See
[VERSIONING.md](../VERSIONING.md).

### It reported an import as "unresolved" but it's fine. Why?

Atlas reports what it can prove and labels the rest honestly rather than
guessing. The most common current gap is package self-imports that rely on
`package.json#main`/`exports`, which the resolver does not yet consult (on the
[roadmap](../ROADMAP.md)). Scope it out with `disable = ["imports/*"]` until it
lands, and please file it if you hit a different case.

### Is it safe to run on untrusted code? Will it execute my code?

Atlas performs **static** analysis — it parses and inspects files, it does not
execute your project. Secrets it surfaces are redacted to four characters
everywhere. See [SECURITY.md](../SECURITY.md) for the threat model and how to
report issues.

### How do I use it in my editor?

Install the VS Code extension (it wraps the built-in `sb lsp` server), or wire
`sb lsp` into any LSP-capable editor. See
[docs/INSTALL.md](INSTALL.md#editor-lsp-setup).

### How do I turn a rule off, or change severities that fail CI?

In `snowbros.toml`: set `min_severity`/`min_confidence`, or
`disable`/`enable` specific rule ids or category globs. See the
[Configuration](../README.md#configuration) section.

### Can I write my own rules?

Today rules are written in Rust — see
[docs/adding-a-rule.md](adding-a-rule.md). A no-Rust pattern engine is on the
[roadmap](../ROADMAP.md).

### What's the license?

Dual-licensed MIT OR Apache-2.0. Use it freely.
