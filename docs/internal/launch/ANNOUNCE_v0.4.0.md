# v0.4.0 announcement — "Multi-language foundation"

Internal drafts for the v0.4.0 release. Keep every number consistent with the
README, `CHANGELOG.md`, and `docs/EXAMPLES.md`. Tone: honest, specific, no
hype. Lead with the architecture (one shared IR) and real numbers. Never claim
users/adoption we don't have.

Release facts (all live):
- CLI/engine **0.4.0** on crates.io (19 crates), npm (`@snowbros/atlas`,
  `snowbros`), Homebrew, and GitHub Releases (5 platforms). VS Code extension
  stays **0.3.0** (LSP-compatible; no changes needed).
- **23 rules.** Python at **preview** maturity.
- FastAPI dogfood (`fastapi/` package, commit `b1346bb`): 48 files, ~400 ms
  cold / ~61 ms warm, 23 findings, health 92, **zero Python-specific false
  positives**.

---

## GitHub release note (short)

**Snowbros Atlas v0.4.0 — Multi-language foundation**

Atlas grows from a JavaScript/TypeScript analyzer into a multi-language
platform. Every language now lowers into one shared semantic IR, and a rule is
either language-agnostic or scoped to a language family in exactly one place —
never an `if language ==` branch buried in a detector.

- **Python support** — a `tree-sitter-python` frontend and a dedicated Python
  import resolver (relative + absolute, including a package imported by its own
  name when the scan root *is* that package).
- **`complexity/large-function`** — the first cross-language rule. It reads only
  the shared IR (function body size), so the same rule flags oversized
  functions in TypeScript, JavaScript, and Python.
- **Cross-language rules** — circular imports, dead files (with Python
  entry-point exclusions), and unresolved imports now run on Python too.
- **Fix** — absolute self-package Python imports resolve correctly when
  scanning a package directory (found dogfooding FastAPI).

23 rules. Python ships at preview maturity; existing JS/TS output is
byte-identical. Full notes in `CHANGELOG.md`.

---

## Show HN

**Title:** Show HN: Atlas v0.4.0 – one semantic IR for JS/TS and Python (Rust)

**Body:**

Atlas is a deterministic, whole-project static analyzer. v0.4.0 is the release
where it stops being JS/TS-only: JavaScript, TypeScript, and Python now lower
into one shared semantic IR, and rules run over that IR instead of over any one
language's syntax.

The design constraint I set: a rule is either genuinely language-agnostic or
it's explicitly scoped to a language family in one place (the scheduler). There
is no `if language == "python"` branch anywhere inside a detector. To prove the
IR actually carries cross-language meaning, the first new rule —
`complexity/large-function` — reads only the IR (function body size) and fires
identically on a 60-line TypeScript function and a 60-line Python one.

I validated it by dogfooding FastAPI. Pointing Atlas at the `fastapi/` package
(48 files, ~400 ms) reported 14 large functions (`jsonable_encoder` at 146
lines, `analyze_param` at 160), two real module-level import cycles (pylint's
cyclic-import flags the same ones), and seven dead re-export leaves — with zero
Python-specific false positives. The dogfood also surfaced a real resolver bug
(absolute self-package imports like `from fastapi.encoders import x` weren't
resolving when the scan root *is* the package), which this release fixes.

Still true from before: deterministic (same code + config → same findings,
warm-cache run byte-identical to cold, enforced by tests), evidence for every
finding, and anything the resolver can't prove is labeled *unresolved*, never
guessed. Native Rust; LSP + VS Code; SARIF/JSON/HTML/Markdown output.

Install: `npx snowbros analyze` (JS/TS) or `sb analyze <package-dir>` (Python).
Repo and docs in the first comment.

---

## r/rust

**Title:** Atlas v0.4.0 — a multi-language static analyzer built on one shared IR (Rust)

Atlas is a deterministic whole-project analyzer written in Rust. The v0.4.0
architecture question I wanted to get right: how do you add a language without
forking the analysis?

The answer is a three-layer approach — tree-sitter frontends per language, each
lowering into one shared "Atlas IR", with the semantic model, symbol/import
graph, and rule engine all operating over the IR. Adding Python meant writing a
`LanguageFrontend` and registering it; the pipeline didn't change. Rules declare
which languages and analysis stage they need (`RuleRequirements`), and a
scheduler gates them — so the policy lives in one place, not scattered through
detectors.

The payoff: `complexity/large-function` reads only the IR and runs on TS, JS,
and Python with a single implementation. Determinism is enforced (warm cache
byte-identical to cold); everything is sorted and content-addressed.

Happy to talk about the IR design, the frontend trait, or the zero-false-
positive discipline (a frontend may extend the IR but may not weaken its
guarantees).

---

## r/Python

**Title:** Ran a Rust-based whole-project analyzer on FastAPI — here's what it found

I added Python support to Atlas (a deterministic static analyzer) and dogfooded
it on FastAPI to check for false positives before shipping.

On the `fastapi/` package (48 files, ~400 ms): 14 large functions
(`jsonable_encoder` 146 lines, `analyze_param` 160, `solve_dependencies` 124),
two real import cycles (`_compat/__init__` ↔ `_compat/v2`, and `utils.py` doing
`import fastapi` against the package `__init__` re-export), and seven dead
re-export leaves. Zero Python-specific false positives — every finding is
either provable or a language-agnostic library limitation.

It's not a linter replacement — it works one layer up, on whole-project
structure (imports, cycles, dead files, function size). Deterministic, no AI,
evidence for every finding. `pip`-free: `sb analyze <your-package-dir>`, or
`npx snowbros analyze`. Python is preview maturity — feedback from real
Django/FastAPI/Flask codebases is exactly what I'm looking for.

---

## LinkedIn

Snowbros Atlas v0.4.0 is out — the release where it becomes a multi-language
platform.

The interesting part isn't "it does Python now." It's *how*: every language
lowers into one shared semantic IR, and rules run over that IR. A rule is
either language-agnostic or scoped to a language family in exactly one place —
never a language check buried in a detector. To prove the IR carries real
cross-language meaning, the first new rule (`large-function`) reads only the IR
and fires identically on TypeScript and Python.

I validated it by dogfooding FastAPI: 48 files, ~400 ms, 23 findings, and zero
Python-specific false positives — the dogfood even surfaced (and I fixed) a real
resolver bug along the way. Deterministic by construction, evidence for every
finding, native Rust.

RFC-0002 (the multi-language architecture) is now validated in production. Next
on the roadmap: Go, Rust, and Java frontends on the same IR.

Open source (MIT/Apache-2.0). Try it: npx snowbros analyze

---

## X / Twitter thread

1/ Snowbros Atlas v0.4.0 is out. It's now multi-language: JavaScript,
TypeScript, and Python lower into ONE shared semantic IR. Rules run over the
IR, not over any language's syntax. 🧵

2/ The rule I'm proudest of: `complexity/large-function`. It reads only the
shared IR (function body size), so the *same* rule flags a 60-line TS function
and a 60-line Python function. No `if language ==` anywhere in it.

3/ Adding Python meant writing one `LanguageFrontend` + registering it. The
pipeline didn't change. Rules declare the languages/stage they need; a
scheduler gates them. Policy in one place.

4/ Validated by dogfooding FastAPI: 48 files, ~400 ms, 23 findings, ZERO
Python-specific false positives. It even surfaced a real resolver bug (absolute
self-package imports) — now fixed.

5/ Still deterministic (warm cache byte-identical to cold, enforced by tests),
still evidence-for-every-finding, still native Rust. Live on crates.io, npm,
Homebrew, GitHub Releases.

Try it → npx snowbros analyze
