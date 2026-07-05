# Pre-release report — Snowbros Atlas v0.1.0

Status date: 2026-07-05. Repository: local git, 24 commits on `master`,
clean tree, all checks green. Companion documents:
[RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) (step-by-step),
[RELEASING.md](RELEASING.md) (mechanics),
[docs/launch/](docs/launch/) (assets and copy).

## Verdict

**The repository is code-ready and content-ready for v0.1.0.** Every
remaining item is a manual account/infrastructure action (GitHub org,
tokens, registry publishing, media production) that cannot be performed
from this environment. No code blockers.

## Completed

**Product**
- 17 library crates + engine + CLI (`sb`, `snowbros`) + LSP server;
  11 rules with metadata, config enforcement, scorecard, auto-fix,
  watch, SARIF/JSON/Markdown/HTML output.
- ~193 Rust tests + 7 npm wrapper tests; clippy `-D warnings`, fmt,
  cargo-deny all green. Deterministic warm==cold proven by e2e test.
- Dogfooded on zod, axios, fastify (docs/EXAMPLES.md); dogfooding
  found and fixed a real resolver gap (TS-ESM extension substitution).

**Release engineering**
- cargo-dist 0.32: 5 targets, sha256 checksums, shell + PowerShell
  installers, Homebrew formula, tag-triggered release workflow
  (validated: `dist plan`, `dist generate --check`, local `dist build`).
- npm wrapper (`@snowbros/atlas`): checksum-verified download, lazy
  fallback, E2E-tested against a real dist archive on Windows.
- git-cliff changelog automation; CI with fmt/clippy/test×3 OS/deny/
  release-plan/npm-wrapper jobs, triggers on `main` and `master`.
- Toolchain pinned (1.96.1), `Cargo.lock` committed, locked builds.

**Branding (this pass)**
- Full rebrand to SNOWBROS (company) / Snowbros Atlas (product) / `sb`
  (CLI) / `snowbros/atlas` (repo) across README, all docs, website seed
  pages, marketing copy, Cargo/npm metadata, CLI output strings, SARIF
  tool name, cliff config (historical commits rewritten via
  preprocessor). Zero stale references outside internal design docs.
- Package naming decided and recorded (docs/launch/NAMING.md); artifact
  names now `snowbros-atlas-<target>.*`, formula `snowbros-atlas.rb`.
- Availability confirmed 2026-07-05: npm `snowbros` + `@snowbros` scope
  free; crates.io `snowbros`, `snowbros-atlas` free.

**Launch collateral**
- SECURITY.md (supported versions, private reporting, response windows,
  coordinated disclosure).
- Homebrew setup + first-release verification checklist
  (docs/launch/HOMEBREW_SETUP.md).
- Demo production plan: script, commands, expected output, camera flow,
  recording/editing checklists, captions, thumbnail
  (docs/launch/DEMO_PLAN.md).
- Social preview design spec (docs/launch/SOCIAL_PREVIEW_SPEC.md).
- Marketing copy for 6 channels + release notes + repo description and
  topics (docs/launch/).

## Remaining manual actions (in recommended order)

1. **Claim names (do first, minutes):** create GitHub org `snowbros`;
   register npm org `snowbros` (claims `@snowbros` scope). Re-check
   `npm view snowbros` and crates.io immediately before publishing.
2. **Create repos** per the org layout: `atlas` (this code),
   `homebrew-tap` (empty + `Formula/`), `.github` (community health,
   optional at launch), `website` (later, snowbros.me).
3. **Push `atlas`**, confirm first CI run green on all 3 OSes.
4. Repo settings: description + topics (LAUNCH_ASSETS.md), branch
   protection; add `HOMEBREW_TAP_TOKEN` secret (HOMEBREW_SETUP.md).
5. **Media:** produce social preview PNG (spec) and record the 90-second
   demo (plan); embed GIF in README (placeholder marked).
6. **Email:** stand up security@snowbros.me (SECURITY.md references it)
   or swap for a GitHub-only reporting note.
7. **Tag v0.1.0** → release workflow → run the Homebrew first-release
   verification checklist on macOS + Linux.
8. **Publish npm:** `@snowbros/atlas`, then the unscoped `snowbros`
   alias package; smoke `npx snowbros@latest --version` on 3 OSes.
9. **Publish crates.io** in dependency order ending with
   `snowbros-atlas`; smoke `cargo install snowbros-atlas --locked`.
10. **Announce** per MARKETING.md (GitHub → HN → Reddit → X → LinkedIn →
    Product Hunt over 2–3 days).

## Launch blockers (hard)

| # | Blocker | Owner action |
|---|---|---|
| 1 | GitHub org + repos don't exist yet | create org/repos, push |
| 2 | npm org/scope not registered | register before launch day |
| 3 | `HOMEBREW_TAP_TOKEN` secret + tap repo | create (HOMEBREW_SETUP.md) |
| 4 | Homebrew flow never executed (no macOS/Linux locally) | first tagged release + verification checklist |
| 5 | security@snowbros.me mailbox doesn't exist | create or edit SECURITY.md |
| 6 | Social preview image + demo GIF are specs, not files | produce per specs |

## Post-launch tasks

- Triage false-positive reports same-day for week 1 (`fp-report` label).
- Pin roadmap/known-limitations issue; add issue templates.
- Resolver: `package.json#main`/`exports` support (removes the
  fastify-pattern unresolved findings) — first post-launch fix.
- Watch the first release's Homebrew `brew audit` output; adjust dist
  config if needed.
- Rotate/renew `HOMEBREW_TAP_TOKEN` before its expiry.
- Website repo (snowbros.me) seeded from `website/` in this repo.
