# Marketing assets — v0.1.0 launch

All copy below is ready to post, pending the final repo URL swap.

---

## GitHub announcement (Discussions / release announcement)

**Title:** Snowbros Atlas v0.1.0 — deterministic static analysis for JS/TS, written in Rust

Hi everyone —

Today we're releasing Snowbros Atlas, a static-analysis engine that
treats engineering problems the way a compiler treats syntax errors:
deterministically. Same codebase in, same findings out, every time.

What it does today:

- Builds a semantic import graph of your whole project (tsconfig paths,
  aliases, re-exports included) and finds cycles, dead files, unresolved
  imports, unused exports/deps/env vars.
- Understands Next.js server/client boundaries: it will show you the exact
  import chain that drags server-only code into a client component.
- Finds `eval` and hardcoded secrets (redacted in output).
- Fixes what it can prove (`sb fix`): unused dependencies and env vars,
  with guarded, idempotent edits.
- Ships an LSP server, SARIF output for GitHub code scanning, watch mode,
  and an explainable health score.

It's fast — a 500-file repo analyzes in ~270 ms cold and ~43 ms warm —
and it never guesses: anything the resolver can't prove is labeled
unresolved, not reported as a finding.

`npx snowbros analyze` in any JS/TS project to try it. Feedback and rule
ideas welcome — the rule engine and metadata format are designed for
contributions.

---

## LinkedIn launch post

I just released Snowbros Atlas — an open-source static-analysis
engine for JavaScript/TypeScript, written in Rust.

The idea: engineering intelligence should be **deterministic**. Most
"AI code review" tools give you different answers on different days.
Snowbros Atlas behaves like a compiler — same code in, same findings out,
every time — and every finding ships with the evidence chain that
produced it.

What it catches today: circular imports, dead files, unresolved imports,
unused dependencies/exports/env vars, hardcoded secrets, eval, and
Next.js server/client boundary violations (with the full import chain
as proof).

A few things I'm proud of:
⚡ ~43 ms warm analysis on a 500-file repo
🔧 `sb fix` applies only provable, idempotent fixes — it never guesses
🧠 Built-in LSP server, SARIF for GitHub code scanning, health scorecard
📦 One command to try: `npx snowbros analyze`

Open source, MIT/Apache-2.0. Link in comments. I'd love to hear what
rules you'd want next.

#opensource #typescript #rust #staticanalysis #devtools

---

## X/Twitter thread

**1/** Shipped: Snowbros Atlas — deterministic static analysis for
JS/TS, written in Rust.

Same code in → same findings out. Every time. No AI deciding whether
your code has a problem.

`npx snowbros analyze`

**2/** It builds a semantic graph of your whole project: every import,
export, env var, tsconfig alias, re-export chain.

Then it proves things:
- circular imports
- dead files
- server-only code leaking into client components (full chain shown)
- unused deps/exports/env vars
- secrets, eval

**3/** Fast enough to run on every save: ~270 ms cold, ~43 ms warm on a
500-file repo. The incremental cache is proven byte-identical to a cold
run — caching can skip work, never change answers.

**4/** `sb fix` auto-fixes what it can prove — unused dependencies,
dead env vars — with guarded, idempotent edits. If the file changed
since analysis, it skips instead of guessing.

**5/** Comes with: LSP server for editor diagnostics, SARIF output for
GitHub code scanning, watch mode, health scorecard, `sb explain <rule>`
for every rule's logic + false-positive guards.

Rust, open source, MIT/Apache. Repo: <link>

---

## Reddit launch post (r/typescript, r/javascript, adapt per sub)

**Title:** Snowbros Atlas — deterministic whole-project analysis for
JS/TS (Rust, open source). Finds circular imports, dead files, Next.js
boundary violations — with proof.

I've been building a static-analysis engine with one hard rule:
**determinism**. Same codebase in, same findings out, every time. Every
finding carries the evidence chain that produced it, and anything the
resolver can't prove is reported as "unresolved" instead of guessed at.

What it does:

- Semantic import graph of the whole project (tsconfig paths, aliases,
  re-exports) → circular imports, dead files, unresolved imports
- Next.js: detects server-only code imported into client components and
  prints the exact import chain
- Unused dependencies / exports / env vars; hardcoded secrets (redacted);
  eval
- `sb fix` — deterministic auto-fixes (unused deps, dead env vars),
  guarded and idempotent
- LSP server, SARIF for code scanning, watch mode, health score

Performance: ~270 ms cold / ~43 ms warm on a 500-file repo (Rust +
Tree-sitter + incremental cache; warm output proven byte-identical to
cold).

Try: `npx snowbros analyze` in any JS/TS project.

Honest limitations right now: deep parsing is JS/TS/JSX/TSX only,
resolution is file-level (symbol-level call graph is on the roadmap),
and monorepo workspaces are treated as one project. Rule ideas and FP
reports very welcome.

---

## Hacker News (Show HN)

**Title:** Show HN: Snowbros Atlas – deterministic static analysis
for JS/TS, in Rust

**Text:**

I built a static-analysis engine around one constraint: the analysis
must be a pure function of the code. No network, no timestamps, no AI in
the loop — same repo in, same findings out, byte-identical output even
from the incremental cache (there's an e2e test asserting warm == cold).

It builds a whole-project semantic graph (imports, exports, env vars,
tsconfig aliases, re-export chains) and reports things it can prove:
circular imports, dead files, unresolved imports, unused deps/exports/
env vars, hardcoded secrets, eval, and Next.js server/client boundary
violations with the full import chain as evidence.

Design choices that might interest HN:

- Accuracy over quantity: if the resolver can't prove an import target,
  the finding is "unresolved import", never a guess. Every rule
  documents its false-positive guards, enforced by a test harness.
- Auto-fix only what's provable: `sb fix` plans byte-span edits, skips
  files that drifted since analysis, and is idempotent.
- Rust + Tree-sitter + rayon + xxh3 cache: ~270 ms cold / ~43 ms warm on
  a 500-file repo, ~34 ms for a single-file change.
- Ships an LSP server and SARIF output; the CLI, LSP, and benches all
  call one engine crate, so results can't diverge between surfaces.

`npx snowbros analyze` to try it. It's 0.1 — JS/TS only, file-level
resolution, 11 rules. Roadmap: pattern-based rule engine (YAML), call
graph, plugins via WASM. Feedback on rule ideas and false positives is
the most useful thing you can give me.

---

## Product Hunt

**Name:** Snowbros Atlas

**Tagline:** A compiler for engineering problems in JS/TS

**Description:**

Snowbros Atlas is an open-source static-analysis engine written in
Rust. It maps your entire JavaScript/TypeScript project — imports,
exports, env vars, framework boundaries — and reports problems it can
prove: circular imports, dead files, unused dependencies, leaked
server-only code in Next.js client components, hardcoded secrets.

Deterministic by design: same code in, same findings out, with the
evidence chain attached. Fast enough for every save (~43 ms warm on
500 files). Auto-fixes what it can prove. LSP server for your editor,
SARIF for GitHub code scanning, health score for your dashboard.

**First comment:** Maker here — happy to answer anything. The one-line
pitch: static analysis you can trust in CI because it never changes its
mind. Try `npx snowbros analyze` in any JS/TS repo; takes under a
minute.

---

## 90-second demo video script

> Format: screen recording, terminal + editor side by side. No talking
> head. Captions burned in. Timestamps are targets.

**[0:00–0:08] Hook.**
Terminal, big font. Type: `npx snowbros analyze`
Caption: "Your codebase has problems it can prove."

**[0:08–0:22] The reveal.**
Output scrolls: frameworks detected, then findings — one HIGH in red
(server-only-in-client with import chain), one MEDIUM, two LOW. Health
score line lands last: `◆ health: 87/100`.
Caption: "Every finding comes with evidence. No guesses. No AI verdicts."

**[0:22–0:35] Determinism beat.**
Run `sb analyze --format json | sha256sum` twice. Same hash twice.
Caption: "Deterministic. Same code in, same findings out. Byte-identical."

**[0:35–0:50] Speed beat.**
Run `sb analyze` again — cache line shows `503 reused, 0 parsed`, timing
~40 ms. Then touch one file, run again: `502 reused, 1 parsed`.
Caption: "Incremental. Warm runs in milliseconds. Run it on every save."

**[0:50–1:05] Auto-fix beat.**
`sb fix --dry-run` shows two planned fixes (unused dependency, dead env
var). Run `sb fix`. Show the git diff — clean, minimal edits.
Caption: "Fixes only what it can prove. Idempotent. Never clobbers."

**[1:05–1:20] Editor + CI beat.**
Split: editor with red squiggle on the eval line (LSP), then a GitHub
Security tab screenshot with SARIF findings.
Caption: "LSP for your editor. SARIF for GitHub. One engine, same answers."

**[1:20–1:30] Close.**
Logo card. Caption: "Snowbros Atlas. Open source, Rust, MIT/Apache."
`npx snowbros analyze` + repo URL.
