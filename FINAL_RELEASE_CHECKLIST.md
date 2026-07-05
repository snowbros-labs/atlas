# Snowbros Atlas v0.1.0 — final release runbook

Follow line by line on launch day. Everything here is copy-paste ready.
Background: [PRE_RELEASE_REPORT.md](PRE_RELEASE_REPORT.md) ·
[RELEASING.md](RELEASING.md) ·
[docs/launch/HOMEBREW_SETUP.md](docs/launch/HOMEBREW_SETUP.md).

GitHub org slugs are case-insensitive; `github.com/SNOWBROS/atlas` and
`github.com/snowbros/atlas` are the same repo. All metadata uses the
lowercase canonical form.

## 0. Accounts (once, before anything)

- [ ] Create GitHub organization `snowbros`.
- [ ] `npm login` · create npm org **snowbros** (claims the `@snowbros`
      scope): https://www.npmjs.com/org/create — do this first, names
      are first-come-first-served.
- [ ] `cargo login` with a crates.io token.
- [ ] Re-check names are still free: `npm view snowbros` (should 404),
      `npm view @snowbros/atlas` (404), and
      https://crates.io/crates/snowbros-atlas (404).

## 1. Create and push the repository

```sh
cd "C:\PROJECTS\snowbros atlas"
gh repo create snowbros/atlas --public \
  --description "Deterministic static analysis for JS/TS: import graph, dead code, Next.js boundary violations, secrets — same code in, same findings out." \
  --disable-wiki
git remote add origin https://github.com/snowbros/atlas.git
git push -u origin master
```

- [ ] First CI run green on all three OSes (fmt, clippy, test×3, deny,
      release-plan, npm-wrapper×3).
- [ ] Add topics: `gh repo edit snowbros/atlas --add-topic static-analysis --add-topic typescript --add-topic javascript --add-topic nextjs --add-topic linter --add-topic developer-tools --add-topic rust --add-topic sarif --add-topic lsp --add-topic code-quality --add-topic architecture --add-topic dead-code --add-topic dependency-graph --add-topic security --add-topic ci`
- [ ] Upload social preview PNG (Settings → Social preview; spec in
      docs/launch/SOCIAL_PREVIEW_SPEC.md).

## 2. Homebrew tap plumbing

```sh
gh repo create snowbros/homebrew-tap --public --description "Homebrew formulas for SNOWBROS tools"
# create Formula/ dir with a placeholder
gh api -X PUT repos/snowbros/homebrew-tap/contents/Formula/.gitkeep \
  -f message="init tap" -f content="$(printf '' | base64)"
# fine-grained PAT: only repo snowbros/homebrew-tap, Contents: read/write
gh secret set HOMEBREW_TAP_TOKEN --repo snowbros/atlas
```

## 3. Tag and release

```sh
dist plan                      # final sanity: 5 archives + installers + formula
git tag v0.1.0
git push origin v0.1.0
gh run watch --repo snowbros/atlas    # release workflow
```

- [ ] Release workflow green (5 build jobs, host job, publish-homebrew).
- [ ] `gh release view v0.1.0 --repo snowbros/atlas` shows: 5 archives,
      5 `.sha256` files, `snowbros-atlas-installer.sh`,
      `snowbros-atlas-installer.ps1`, `sha256.sum`, `source.tar.gz`.
- [ ] Paste docs/launch/RELEASE_NOTES_v0.1.0.md over the release body:
      `gh release edit v0.1.0 --repo snowbros/atlas --notes-file docs/launch/RELEASE_NOTES_v0.1.0.md`

## 4. Publish npm

```sh
cd npm
npm publish --access public          # @snowbros/atlas
```

Then the unscoped alias (one-time creation, see docs/launch/NAMING.md):

```sh
# in a scratch dir: package.json {"name":"snowbros","version":"0.1.0",
#   "bin":{"sb":"bin/sb.js","snowbros":"bin/snowbros.js"},
#   "dependencies":{"@snowbros/atlas":"0.1.0"}}
# with bin/*.js one-liners: require('@snowbros/atlas/bin/sb.js')
npm publish
```

## 5. Publish crates.io (dependency order)

```sh
cargo publish -p snowbros_core
cargo publish -p snowbros_scanner
cargo publish -p snowbros_parser        # waits: crates.io index propagation
cargo publish -p snowbros_framework
cargo publish -p snowbros_resolver
cargo publish -p snowbros_graph
cargo publish -p snowbros_cache
cargo publish -p snowbros_rules
cargo publish -p snowbros_output
cargo publish -p snowbros_engine
cargo publish -p snowbros_lsp
cargo publish -p snowbros-atlas
```

(Stub crates — security, deps, architecture, performance, plugin — are
not dependencies of the CLI; publish them only if you want the names
reserved.)

## 6. Verify every channel

```powershell
# Windows
irm https://github.com/snowbros/atlas/releases/latest/download/snowbros-atlas-installer.ps1 | iex
sb --version        # snowbros 0.1.0
```

```sh
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/snowbros/atlas/releases/latest/download/snowbros-atlas-installer.sh | sh
sb --version

# Homebrew (macOS + one Linux box; full list in docs/launch/HOMEBREW_SETUP.md)
brew install snowbros/tap/snowbros-atlas && sb --version

# npm — all three OSes
npx @snowbros/atlas@0.1.0 --version
npx snowbros@0.1.0 --version

# Cargo
cargo install snowbros-atlas --locked && sb --version

# Checksums + a real run
curl -LO https://github.com/snowbros/atlas/releases/download/v0.1.0/snowbros-atlas-x86_64-unknown-linux-gnu.tar.gz{,.sha256}
sha256sum -c snowbros-atlas-x86_64-unknown-linux-gnu.tar.gz.sha256
git clone --depth 1 https://github.com/axios/axios && cd axios && sb analyze
```

## 7. Announce

- [ ] Record demo per docs/launch/DEMO_PLAN.md; embed GIF in README.
- [ ] Post per docs/launch/MARKETING.md: GitHub → Show HN → Reddit →
      X → LinkedIn → Product Hunt, spread over 2–3 days.
- [ ] Pin roadmap issue; enable issue templates; watch for FP reports
      (label `fp-report`, same-day triage week 1).

## Abort criteria

Stop and fix (delete the tag if needed: `git push origin :v0.1.0` +
`gh release delete v0.1.0`) if: any release build job fails; formula
lands with wrong URLs/sha; installer smoke test fails on any OS; or
`npx` pulls a binary whose `--version` mismatches the tag.
