# Launch posts — ready to publish

Internal drafts. Post-order guidance is in `MARKETING.md`. Keep every number
consistent with the README and `docs/EXAMPLES.md`. Do **not** post until the
🔴 blockers in the launch checklist are cleared (working installs, social image,
demo GIF, deployed site).

Tone: honest, specific, no hype. Lead with the determinism angle and real
numbers. Never claim users/adoption we don't have.

---

## Show HN

**Title:** Show HN: Snowbros Atlas – deterministic static analysis for JS/TS (Rust)

**Body:**

Atlas analyzes whole-project structure for JavaScript/TypeScript — the import
graph, framework boundaries, and the dependency manifest — and reports only what
it can prove: circular imports, dead files, Next.js `server-only` code leaking
into client components (with the full import chain), unused deps, hardcoded
secrets.

Two things I cared about building it:

- **Deterministic.** Same code and config in, same findings out. The warm-cache
  run is byte-identical to a cold run (enforced by tests) — the cache can skip
  work but never change results. No AI, no heuristic drift.
- **Provable.** Anything the resolver can't prove is reported as *unresolved*,
  never guessed. Every finding carries its evidence chain.

It's a native Rust binary: ~270 ms cold / ~34 ms per changed file on a 500-file
repo. Terminal, JSON, SARIF (CI gate), and self-contained HTML output, plus a
built-in LSP and a VS Code extension.

It is **not** an ESLint replacement — run it alongside your linter; its territory
is project structure, not style. JS/TS only today; more languages are on the
roadmap, not shipped.

Try it: `npx snowbros analyze`
Repo: https://github.com/snowbros-labs/atlas

Happy to talk about the determinism guarantees, the Next.js boundary analysis,
and where the false-positive edges currently are (package `main`/`exports`
resolution is the known gap).

*(Reply-ready notes: single maintainer, ~20h build sprint, dogfooded on
zod/axios/fastify — be upfront if asked about maturity/users: there are none yet,
this is a launch.)*

---

## Product Hunt

**Name:** Snowbros Atlas
**Tagline:** Deterministic static analysis for JS/TS — proven, not guessed
**Topics:** Developer Tools, Open Source, GitHub

**Description:**
Snowbros Atlas maps your whole project — every import, export, and framework
boundary — and reports problems it can prove: circular imports, dead files,
Next.js server/client leaks, unused deps, secrets. Deterministic (same code in,
same findings out), native-fast, with an evidence chain for every finding. CLI +
CI (SARIF) + VS Code.

**First comment (maker):**
Hi PH 👋 I built Atlas because linters see one file at a time, but the expensive
bugs live in whole-project structure — a cycle that's survived for years, a
`server-only` import that leaks into a client bundle. Atlas works on that layer,
and it's deterministic by design: no AI, every finding backed by evidence. It's
open source (MIT/Apache), a native Rust binary, and runs with `npx snowbros
analyze`. Would love feedback on the rules and the Next.js boundary analysis.

*(PH is visual-first — do not launch here until logo + demo GIF + gallery
screenshots + live site exist.)*

---

## Reddit

### r/rust
**Title:** Snowbros Atlas: a deterministic static-analysis tool for JS/TS, written in Rust

Focus for this audience: the architecture (clean crate layering, `engine` as the
single analyze() entry point for CLI + LSP), determinism (warm == cold,
byte-identical), petgraph for SCC/cycles, tree-sitter parsing, incremental cache
(xxh3 + mtime), cargo-dist multi-target releases. Honest about scope: JS/TS only,
11 rules, single maintainer. Ask for review of the panic-surface / FP edges.

### r/nextjs
**Title:** A tool that catches `server-only` code leaking into client components (with the import chain)

Lead with the boundary rule — it's the most useful thing here for this audience.
Show the finding: `Dashboard.tsx → lib/metrics.ts → lib/db.ts ("server-only")`,
resolved through aliases and re-export chains. Also private env vars reaching the
client. `npx snowbros analyze`.

### r/javascript
**Title:** Snowbros Atlas — whole-project analysis (cycles, dead files, unused deps) that's deterministic and fast

General framing; emphasize "run it alongside ESLint/Biome, not instead of."

---

## LinkedIn

Shipping something I'm proud of: **Snowbros Atlas** — open-source, deterministic
static analysis for JavaScript/TypeScript.

Linters check one file at a time. Atlas works one layer up — the whole import
graph, framework boundaries, the dependency manifest — and reports only what it
can prove: circular imports, dead files, Next.js server/client boundary leaks,
unused dependencies, hardcoded secrets.

Two principles it's built on:
• Deterministic — same code in, same findings out. No AI deciding what's a
  problem.
• Provable — every finding carries the evidence chain that produced it.

Native Rust, sub-second incremental analysis, CLI + CI (SARIF) + a VS Code
extension. Try it in one line: `npx snowbros analyze`

Open source (MIT/Apache): https://github.com/snowbros-labs/atlas
Feedback welcome. 🧭

---

## X / Twitter thread

1/ Linters see one file at a time. The expensive bugs live in whole-project
structure: a circular import that's survived for years, a `server-only` module
leaking into a client bundle.

Snowbros Atlas works on that layer. Open source, native Rust. 🧭
`npx snowbros analyze`

2/ It's deterministic by design. Same code + config in → same findings out. The
warm-cache run is byte-identical to a cold run (we test for it). No AI deciding
whether an issue exists.

3/ And it's provable. Anything the resolver can't prove is reported as
*unresolved* — never guessed. Every finding carries its evidence chain:

`Dashboard.tsx → lib/metrics.ts → lib/db.ts ("server-only")`

4/ Fast: ~270 ms cold, ~34 ms per changed file on a 500-file repo. Terminal,
JSON, SARIF (CI gate), self-contained HTML, a health scorecard — plus a built-in
LSP and a VS Code extension.

5/ Not an ESLint replacement — run it alongside your linter. Its territory is the
graph, the boundaries, the manifest. JS/TS today; more on the roadmap.

MIT/Apache. Try it, break it, tell me where it's wrong:
https://github.com/snowbros-labs/atlas

---

## Dev.to / Hashnode / Medium article

**Working title:** "We ran deterministic analysis on zod, axios and fastify —
here's what a whole-project graph finds that linters can't"

**Outline:**
1. The gap: per-file linting vs. project structure. Why cycles and boundary
   leaks are invisible to ESLint.
2. What "deterministic" buys you: reproducible CI, no flaky findings, warm ==
   cold. Contrast with heuristic/AI tools.
3. Case study — zod: the real v3 and v4/core cycles, found deterministically in
   ~88 ms warm. Screenshot of the terminal + HTML report.
4. Case study — axios: a fixture credential caught and redacted; health 97/100.
5. Case study — fastify: honesty in action — package self-imports labeled
   *unresolved* instead of guessed, and why that's the right call.
6. The Next.js angle: catching `server-only` leaks through re-export chains.
7. How it works, briefly: scan → parse (tree-sitter) → resolve → graph
   (petgraph SCC) → rules → report. Evidence-first.
8. Try it: `npx snowbros analyze`; CI with SARIF; the VS Code extension.
9. Honest limitations + roadmap: JS/TS only, 11 rules, `package.json#main` gap.

Publish the same article to Dev.to (canonical), then cross-post to Hashnode and
Medium with `rel=canonical` back to Dev.to. Reuse `assets/terminal.svg` and
`assets/screenshot-html-report.png`.
