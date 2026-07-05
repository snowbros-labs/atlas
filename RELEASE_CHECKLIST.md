# Release checklist — v0.1.0 public launch

Work top to bottom. Nothing below "Tag and release" happens until every
box above it is checked. Companion docs: [RELEASING.md](RELEASING.md)
(mechanics), [docs/launch/](docs/launch/) (copy + assets).

## 1. Identity (one-time, blocking)

- [ ] Decide the final GitHub org/repo name. Current placeholder:
      `snowbros/snowbros-inspector`.
- [ ] If it differs, replace the URL everywhere:
      `git grep -l "snowbros/snowbros-inspector"` →
      `Cargo.toml`, `crates/snowbros_cli/Cargo.toml`, `npm/package.json`,
      `npm/lib/platform.js`, `npm/test/wrapper.test.js`, `npm/README.md`,
      `README.md`, `docs/*`, `RELEASING.md`, `CONTRIBUTING.md`,
      `website/*.html`, `docs/launch/*`.
- [ ] Verify npm name `snowbros` is available (`npm view snowbros`);
      if taken, rename npm package + wrapper URLs + docs.
- [ ] Verify crates.io name `snowbros` is available; if taken, adjust
      `[package] name` and `cargo install` docs.

## 2. GitHub repository setup

- [ ] Create the repo; push `master` (or rename to `main` — ci.yml
      triggers on both).
- [ ] Set repository description and topics
      (copy from [docs/launch/LAUNCH_ASSETS.md](docs/launch/LAUNCH_ASSETS.md)).
- [ ] Upload social preview image (spec in LAUNCH_ASSETS.md).
- [ ] Enable GitHub Pages → deploy from `website/` (or `docs/` branch
      strategy); set the homepage field to the Pages URL.
- [ ] Create `snowbros/homebrew-tap` repo (empty, with `Formula/` dir).
- [ ] Add repo secret `HOMEBREW_TAP_TOKEN` (write access to the tap).
- [ ] Branch protection on the default branch: require CI green.

## 3. Code green

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace` — all pass, no ignored regressions
- [ ] `(cd npm && npm test)` — 7/7
- [ ] `cargo bench -p snowbros` — numbers within expected range
      (cold ~162 ms / warm ~5 ms @ 200 files; investigate >2× drift)
- [ ] `dist plan` succeeds; `dist generate --check` clean
- [ ] cargo-deny green (CI `deny` job)
- [ ] First CI run on GitHub fully green on all three OSes

## 4. Version + changelog

- [ ] `Cargo.toml [workspace.package] version = "0.1.0"` and
      `npm/package.json "version": "0.1.0"` agree (npm test enforces)
- [ ] `npx git-cliff --tag v0.1.0 -o CHANGELOG.md` — regenerate, review
      wording, commit
- [ ] Review [docs/launch/RELEASE_NOTES_v0.1.0.md](docs/launch/RELEASE_NOTES_v0.1.0.md)
      against the final feature set

## 5. Dry runs

- [ ] `dist build` locally — archive contains `sb` + `snowbros` +
      licenses + README; checksum file parses
- [ ] npm wrapper against the local archive: extract → vendor →
      `node bin/sb.js --version` prints the right version
- [ ] `sb analyze` on one dogfood repo (see docs/EXAMPLES.md) — sane
      output, warm run fast

## 6. Tag and release

- [ ] `git commit -m "chore(release): v0.1.0"` (if anything changed)
- [ ] `git tag v0.1.0 && git push origin --tags`
- [ ] Release workflow green: 5 archives + 2 installers + formula +
      checksums attached to the GitHub Release
- [ ] Release notes on the GitHub Release match RELEASE_NOTES_v0.1.0.md
      (paste over the auto-generated body if needed)

## 7. Publish channels

- [ ] Homebrew: formula landed in the tap;
      `brew install snowbros/tap/snowbros && sb --version` on macOS and
      Linux
- [ ] npm: `cd npm && npm publish` (2FA ready);
      `npx snowbros@0.1.0 --version` on all three OSes
- [ ] crates.io: publish workspace crates in dependency order, ending
      with `cargo publish -p snowbros`;
      then `cargo install snowbros --locked && sb --version`
- [ ] Installer smoke tests:
      - Windows: `irm .../snowbros-installer.ps1 | iex; sb --version`
      - macOS/Linux: `curl -LsSf .../snowbros-installer.sh | sh; sb --version`
- [ ] Checksums verify: `sha256sum -c <archive>.sha256`

## 8. Launch

- [ ] Record the 90-second demo (script in docs/launch/MARKETING.md);
      replace the README GIF placeholder with the real capture
- [ ] Post: GitHub announcement → HN (Show HN) → Reddit → X thread →
      LinkedIn → Product Hunt (copy in MARKETING.md; spread over 2–3 days)
- [ ] Watch issues for false-positive reports; label `fp-report` and
      triage same-day for the first week

## 9. Post-launch hygiene

- [ ] Enable GitHub issue templates (bug / FP report / rule idea)
- [ ] Pin a "roadmap + known limitations" issue (content: website/roadmap
      + docs/EXAMPLES.md limitations)
- [ ] Add SECURITY.md with a private-report channel before announcing to
      security-focused audiences
