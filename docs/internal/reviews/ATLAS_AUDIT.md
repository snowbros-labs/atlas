# Snowbros Atlas — Engineering Due-Diligence Audit

Audited: local repo (git log, Cargo workspace, per-crate LOC/tests) + GitHub (issues/PRs/releases/CI via `gh`) + live npm registry lookup. crates.io lookup blocked by their public API access policy at audit time — flagged as unverified, not assumed.

**Correction before anything else:** this is not an AI-powered analysis tool. It is explicitly designed as the opposite — a deterministic static-analysis engine whose stated principle is "same codebase in, same findings out, every time — no AI decides." Every "AI" reference in the original brief should be read as "deterministic engine."

**Scale reality check:** entire codebase — 17 crates, CLI, LSP server, release pipeline, npm wrapper, website, docs, rebrand — built and shipped in one continuous ~20-hour session (2026-07-04 21:55 → 2026-07-05 17:40 UTC+5:30), not weeks of iteration. No production usage history: 0 real PRs, 2 GitHub issues (both maintainer-opened roadmap placeholders, not user reports), 0 external contributors, 0 recorded false-positive reports. "Battle-tested" claims mean tested against 3 dogfood repos (zod, axios, fastify), not real users.

---

## 1. Verdict

A real, working, narrow tool — not the platform the original brief assumes. The JS/TS analysis core (parse → resolve → graph → 11 rules → scorecard → SARIF/HTML/JSON/Markdown output) is genuinely implemented, tested (176 Rust + 7 npm tests), dogfooded with a real bug found and fixed. Everything adjacent — plugin system, security/deps/performance/architecture "analyzers," multi-language support beyond JS/TS, VS Code extension, web dashboard — is either an empty stub crate or doesn't exist. Release engineering (cargo-dist, npm wrapper, Homebrew, CI) is unusually mature for the project's age. Main risk: 5 of 17 workspace crates are 7-line placeholders re-exporting `snowbros_core` and nothing else.

## 2. Original Vision vs. Reconstructed Reality

No roadmap doc predates the code — `ARCHITECTURE.md` and `implementation_plan.md` (both ~50KB, written same day) describe an 18-section design already scoped to include the stub crates as future phases. Roadmap didn't change mid-flight; it was authored as multi-phase (Phase 1 = JS/TS core, Phase 2 = pattern engine/more languages, Phase 3/4 = dashboard/multi-repo) and only Phase 1 executed. Two open GitHub issues (#1 v0.3.0, #2 v0.1.1) and `docs/launch/POST_LAUNCH_ROADMAP.md` restate the same phase plan.

## 3. Feature Audit

| Feature | Planned | Implemented | Tested | Prod-ready | Notes |
|---|---|---|---|---|---|
| CLI (`sb`/`snowbros`) | ✓ | Yes | ✓ | Yes | init, analyze, watch, graph, explain, fix, lsp — all wired, 1,088 LOC. |
| Scanner (file walk) | ✓ | Yes | ✓ | Yes | `ignore`-crate walk, excludes `.snowbros`/`node_modules`. |
| Parser / language detection | ✓ | Partial | ✓ | Partial | Detects 23 languages by name/ext/shebang; deep analysis only for JS/TS/JSX/TSX. Code itself says: "recognition is broader than analysis." |
| Resolver (import resolution) | ✓ | Yes | ✓ | Partial | Relative + ext/index probing, tsconfig paths. Known gap: no `package.json#main/exports` — 51 false unresolved imports on fastify (dogfood-confirmed). |
| Import graph / cycle detection | ✓ | Yes | ✓ | Yes | Petgraph + Tarjan SCC, toposort, DOT export. Finds real v3/v4 cycles in zod. |
| Cache / incremental analysis | ✓ | Yes | ✓ | Yes | xxh3+mtime, warm==cold byte-identical (e2e proven). |
| Watch mode | ✓ | Yes | ✓ | Partial | notify 300ms debounce. Self-documented gap: diff line-keyed, not refactor-stable. |
| Config (`snowbros.toml`) | ✓ | Yes | ✓ | Yes | min_severity/min_confidence, per-rule enable/disable, hard-errors on invalid config. |
| Rule engine + 11 rules | 20–50 planned | Partial | ✓ | Yes (for the 11) | 45–78% short of stated 20–50 target. |
| Reporter — JSON/Markdown | ✓ | Yes | ✓ | Yes | |
| SARIF 2.1.0 output | ✓ | Yes | ✓ | Yes | CI-gate integration (`--ci`, exit 2 on High+). |
| HTML report | ✓ | Yes | ✓ | Yes | Self-contained, no external assets. |
| Scorecard / health score | ✓ | Yes | ✓ | Yes | Severity × confidence weights, explainable deductions. |
| Auto-fix (`sb fix`) | ✓ | Partial | ✓ | Partial | Only 2 of 11 rules have real fixers (unused-dependency, unused-env-var). |
| LSP server | ✓ | Yes | Manual e2e only | Partial | tower-lsp/stdio. No CI test, no editor extension ships it. |
| VS Code extension | Implied in brief | **No** | — | No | LSP server exists; no extension client, no `.vsix`, no marketplace listing. |
| Plugin system (WASM) | ✓ (workspace member) | **No** | — | No | `snowbros_plugin` = 7-line stub. |
| Security analyzer (taint/CVE) | ✓ (workspace member) | **No** | — | No | `snowbros_security` stub; the 2 real security rules live in unrelated `snowbros_rules`. |
| Dependency analyzer (deep) | ✓ (workspace member) | **No** | — | No | `snowbros_deps` stub; only shallow unused-dependency rule exists elsewhere. |
| Architecture analyzer | ✓ (workspace member) | **No** | — | No | `snowbros_architecture` stub; circular-imports/dead-file rules live elsewhere. |
| Performance analyzer | ✓ (workspace member) | **No** | — | No | `snowbros_performance` stub; only forced-dynamic (Next.js) rule exists elsewhere. |
| Pattern rule engine | ✓ (v0.3.0) | **No** | — | No | Not started; all 11 rules hand-written Rust. |
| Multi-repo / trends / dashboard | ✓ (Phase 3/4) | **No** | — | No | Zero code. |
| Desktop/web dashboard | Mentioned in brief | **No** | — | No | Doesn't exist, not even a stub. |
| Release pipeline (cargo-dist) | ✓ | Yes | Yes (local) | Yes | 5 targets, sha256, tag-triggered. |
| npm packaging | ✓ | Yes | ✓ | Yes | Verified live: `@snowbros/atlas@0.1.0` + unscoped `snowbros@0.1.0` both on registry. |
| Homebrew tap | ✓ | Partial | Untested | Partial | CI green, but real `brew install` flow never run on real hardware per project's own risk doc. |
| crates.io publish | ✓ | Unverified | — | Unverified | API blocked lookup; documented publish order exists but not independently confirmed live. |
| CI (fmt/clippy/test×3 OS/deny) | ✓ | Yes | ✓ | Yes | Last 5 `gh run list` runs all green. |
| Website | ✓ | Partial | — | Partial | 8 static pages exist locally; live deployment (snowbros.me) unverified. |
| Documentation | ✓ | Yes | — | Yes | Unusually thorough: README, INSTALL, CONTRIBUTING, RELEASING, SECURITY, EXAMPLES. |
| Benchmarks | ✓ | Yes | ✓ (criterion) | Yes | 500-file repo: 270ms cold / 43ms warm / 34ms single-file-change. |

## 4. Finished / Partial / Not Started

**✅ Finished**
- JS/TS analysis core end-to-end, verified against 3 real OSS repos with concrete before/after numbers.
- All 4 output formats (terminal/JSON/Markdown/SARIF/HTML) present and CLI-reachable.
- Release + packaging pipeline — more mature than the analysis feature set, live on npm.

**🟨 Partially Finished**
- Auto-fix: real engine, only 2/11 rules wired.
- Import resolution: solid for relative/tsconfig; known gap on package-root self-imports.
- LSP: server works; no packaged client.
- Homebrew/crates.io distribution: pipeline exists, real install path unverified/untested.

**❌ Not Started**
- Plugin system, deep security/deps/architecture/performance analysis (all 5 are empty stub crates).
- Pattern-rule engine, community rule contribution, VS Code extension, desktop/web dashboard, multi-repo/trends, any language beyond JS/TS family.

## 5. Documentation vs. Code Discrepancies

| Claim | Where stated | Actual state |
|---|---|---|
| "17 library crates" implementing the platform | Memory notes, `PRE_RELEASE_REPORT.md` | 5 of 17 are empty re-export stubs — overstates real surface ~1.4×. |
| `snowbros_security` crate implies security subsystem | `Cargo.toml` | Real security rules live in `snowbros_rules` instead — misleading. |
| "23-language detect" | Memory/internal notes | Accurate for detection only; parser module itself documents deep analysis is JS/TS-only. |
| npm keywords include "architecture", "lsp" | `@snowbros/atlas` npm metadata | LSP real; "architecture" implies the stub crate's promised subsystem, which doesn't exist. |

## 6. Git History

32 commits, all between 2026-07-04 21:55 and 2026-07-05 17:40 — one continuous build. No reverted features, no dead branches (only master + one release branch), clean tree throughout. AI-assisted build sprint per the project's own commit conventions ("conventional commits w/ Claude co-author") — codebase hasn't been exposed to time, multiple contributors, or real user load yet.

## 7. Architecture Assessment

- **Strengths:** clean crate boundaries (core → parser/scanner → resolver → graph → rules → engine → cli/lsp), deterministic-by-design (sorted output, no timestamps, evidence-mandatory findings), secret redaction built into parser layer, config-fingerprinted cache invalidation. `unsafe_code = "deny"` workspace-wide.
- **Technical debt:** 5 stub crates confuse the mental model of where "security"/"architecture" findings actually come from — delete until real, or move existing rules in so names match reality.
- **Maintainability:** rule metadata enforced 1:1 with rule IDs via harness. 176 tests / ~9,000 LOC of real logic is reasonable density.
- **Extensibility:** generic byte-span fix engine ready for more fixers; rule registry pattern should make rule #12 straightforward. Blocker to 20–50 rules is authoring time, not architecture.
- **Refactor first:** resolve stub-crate naming collision before it's load-bearing in anyone's mental model; nail `package.json#main/exports` before onboarding real users (highest-volume confirmed FP source).

## 8. Missing vs. Competing Tools

| Capability | Have it? | Comparable tool |
|---|---|---|
| Community rule authoring w/o Rust | No | Semgrep, ESLint plugins |
| Autofix coverage across most rules | 2 of 11 | Biome, Ruff, Clippy |
| Packaged IDE extension | No | ESLint, Ruff, SonarQube, Biome |
| Monorepo/workspace awareness | No (self-documented gap) | ESLint, Biome, Oxlint |
| Vulnerability DB (OSV/CVE) | No | Semgrep, CodeQL, DeepSource, Snyk |
| Multi-language deep analysis | JS/TS only | SonarQube, CodeQL, Codacy |
| Historical trend tracking/dashboard | No | SonarQube, Codacy, DeepSource |
| Taint-tracking security analysis | No | CodeQL, Semgrep Pro, Snyk Code |
| Deterministic, no-LLM findings | **Yes** | Differentiator vs. most AI-adjacent tools |
| Sub-second incremental analysis | **Yes** (34ms/file @500 files) | Competitive with Biome/Oxlint |
| Explainable scoring w/ evidence chains | **Yes** | Stronger than SonarQube's comparatively opaque scoring |

## 9. Roadmap

**Immediate**
1. Fix/remove the 5 stub-crate naming collision.
2. `package.json#main/exports` resolution — highest-confirmed FP source.
3. Verify Homebrew and crates.io publish end-to-end on real hardware.

**MVP Complete**
4. Monorepo/workspace-aware resolution.
5. Rule maturity gating (nursery tier off by default).
6. Autofix coverage on ≥half the rule set.

**Production Ready**
7. Packaged VS Code extension wrapping existing LSP server.
8. Real user feedback loop (fp-report triage), actually exercised.
9. Automated LSP test in CI.

**Competitive**
10. Pattern rule engine (Semgrep-style).
11. Grow to 20–50 rules, prioritized by real issue reactions.
12. OSV vulnerability DB integration.

**Long Term (12mo)**
13. Symbol-level resolution — only if FP volume proves file-level is the limiter.
14. Multi-repo trends/dashboard — from-scratch build, no code exists.
15. Second language family (Python — most-detected non-JS extension already wired in).

## 10. Hidden Problems

- **Stub-crate mismatch** (Medium/Easy) — low effort to fix, meaningful clarity win.
- **Zero real-world exposure** (High/N/A) — every "proven" claim is self-verified, not independently verified.
- **Unverifiable crates.io state** (Medium/Easy) — needs direct verification by someone with access.
- **Homebrew flow untested on real hardware** (Medium/Easy) — flagged by project's own risk register, unresolved.
- **LSP has no CI test** (Medium/Medium) — regression could ship silently.
- **Auto-fix asymmetry** (Low/Medium) — 9 of 11 rules diagnose-only.
- **No dedicated security review of the codebase itself** (Medium/Medium) — worth a separate pass given the tool markets itself on trustworthy findings.

## 11. Final Scorecard (/10)

Architecture 7 · Code Quality 8 · Performance 8 · Documentation 8 · Dev Experience 6 · Testing 7 · Packaging 7 · CLI UX 7 · Scalability 5 · OSS Readiness 4 · Enterprise Readiness 3 · **Overall 6**

OSS/Enterprise readiness scored low not for code quality but absence of real-user validation, community rule path, IDE integration, or multi-language coverage.

## 12. Next 20 Tasks (ordered)

1. **Resolve stub-crate naming collision** — Priority: High, Difficulty: Easy, Effort: 1–2h, Deps: none. DoD: delete 5 stub crates or move matching rules in so names match reality.
2. **`package.json` main/exports resolution** — Priority: Critical, Difficulty: Medium, Effort: 2–3d, Deps: none. DoD: fastify dogfood run drops to ~0 unresolved on package-root self-imports, regression test added.
3. **Verify crates.io publish state directly** — Priority: High, Difficulty: Easy, Effort: 15m, Deps: crates.io access. DoD: `cargo install snowbros-atlas --locked` succeeds from clean machine.
4. **Execute untested Homebrew install flow** — Priority: High, Difficulty: Easy, Effort: 1h, Deps: macOS/Linux box. DoD: `brew install` + `sb --version` works on real hardware.
5. **Monorepo/workspace awareness in resolver** — Priority: High, Difficulty: Hard, Effort: 1–2w, Deps: task 2. DoD: zod analyzes correctly without config workarounds.
6. **Rule maturity gating (nursery tier)** — Priority: Medium, Difficulty: Medium, Effort: 2–3d, Deps: none. DoD: config supports nursery flag, off by default, documented.
7. **Automated LSP integration test in CI** — Priority: Medium, Difficulty: Medium, Effort: 1–2d, Deps: none. DoD: CI job drives stdio handshake, asserts on published diagnostics.
8. **Package a VS Code extension client** — Priority: High, Difficulty: Medium, Effort: 3–5d, Deps: task 7. DoD: `.vsix` installs, shows live diagnostics on save.
9. **Autofix for 3 more rules** — Priority: Medium, Difficulty: Medium, Effort: 3–5d, Deps: none. DoD: guarded, idempotent fixers with regression tests.
10. **Independent security review of secret-redaction + fix-applier** — Priority: High, Difficulty: Medium, Effort: 2d, Deps: none. DoD: `/security-review` pass completed, findings triaged.
11. **Confirm website deployment** — Priority: Low, Difficulty: Easy, Effort: 1h, Deps: DNS access. DoD: public URL resolves, matches `website/` content.
12. **Set up fp-report triage cadence for real** — Priority: Medium, Difficulty: Easy, Effort: ongoing. DoD: first real triage cycle completed after any user tries the tool.
13. **security@snowbros.me mailbox or SECURITY.md correction** — Priority: Medium, Difficulty: Easy, Effort: 30m. DoD: mailbox live, or doc points to GitHub private vulnerability reporting.
14. **Pattern rule engine (Semgrep-style)** — Priority: Medium, Difficulty: Hard, Effort: 2–3w, Deps: task 6. DoD: 3–5 rules ported to pattern format, documented for contributors.
15. **Grow rule count toward 20–50** — Priority: Medium, Difficulty: Medium, Effort: ongoing, Deps: task 14. DoD: ranked by real issue reactions once available.
16. **OSV vulnerability DB integration** — Priority: Low, Difficulty: Hard, Effort: 1–2w. DoD: known-CVE dependency flagged with source/version evidence.
17. **Second language family (Python candidate)** — Priority: Low, Difficulty: Hard, Effort: 3–4w. DoD: import-graph + dead-file parity with JS/TS core.
18. **`sb doctor` self-check** — Priority: Low, Difficulty: Easy, Effort: 1–2d. DoD: single command surfaces common misconfig.
19. **Symbol-level resolution groundwork** — Priority: Low, Difficulty: Hard, Effort: 3–4w+. DoD: gated behind FP evidence, not started speculatively.
20. **Multi-repo trends dashboard** — Priority: Low, Difficulty: Hard, Effort: 6–8w+. DoD: sequenced last deliberately, out of scope until above tasks close.
