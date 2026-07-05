# Post-launch roadmap

Principle: 0.1.1 and 0.2.0 are shaped by what real users hit in the
first weeks, not by feature ambition. Nothing below adds analyzer
surface without a reported need. SemVer on 0.x: breaking CLI/output
changes bump the minor.

## v0.1.1 — stabilization (target: 1–2 weeks after launch)

Patch release. Bug fixes only, no new behavior.

- False-positive fixes from `fp-report` issues — highest priority;
  every confirmed FP gets a regression test and, where applicable, a
  documented guard in the rule's metadata.
- Installer/platform fixes: whatever the first real Windows/macOS/Linux
  installs surface (PATH handling, proxy downloads in the npm wrapper,
  antivirus false alarms → document).
- aarch64-linux build fix if the first release run exposes one.
- Docs corrections reported by readers.
- Any RUSTSEC advisory patch bumps.

Explicitly not in 0.1.1: new rules, resolver features.

## v0.2.0 — resolve what users actually hit (target: 4–8 weeks)

Driven by the two limitations users will meet first (both already
observed in dogfooding):

- **`package.json` `main`/`exports` resolution** — removes the
  fastify-pattern false "unresolved import" on package-root
  self-imports. Biggest known FP source remaining.
- **Monorepo/workspace awareness** — multiple `package.json` files
  (pnpm/yarn/npm workspaces) treated as separate packages with correct
  per-package dependency rules. Second-most-likely complaint from real
  repos (zod is a workspace today).
- **Rule maturity gating** — `nursery` tier off by default, so future
  rules can ship without destabilizing scores users already track.
- Config additions only if repeatedly requested (e.g. per-path rule
  overrides — only with concrete demand).

Success criterion for 0.2.0: re-run the dogfood suite; fastify
unresolved findings drop to ~0 and zod analyzes as a workspace without
config gymnastics.

## v0.3.0 — extend rule authoring (target: quarter after 0.2.0)

Only after FP rate is demonstrably low and issue volume is manageable:

- **Pattern rule engine** (Semgrep-style `$VAR` patterns in YAML) — the
  gate for community-contributed rules without Rust. Ship with 3–5
  rules ported to patterns to prove the format.
- **New rules from the issue tracker** — ranked strictly by 👍 count and
  FP-safety, not by roadmap aspiration; each needs the standard
  metadata + guard tests.
- **`sb doctor`** — environment/config self-check, if support burden
  shows people misconfigure (evidence-driven, cheap).
- Begin symbol-level resolution groundwork *only if* dead-file /
  unused-export FP reports show file-level granularity is the limiting
  factor users care about.

## Continuous (any release)

- Keep dogfood suite (zod/axios/fastify + any repo from a good bug
  report) in a periodic job; findings drift = regression signal.
- Benchmarks tracked per release; >2× regression blocks the release.
- Security reports per SECURITY.md take priority over everything.
