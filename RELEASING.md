# Releasing Snowbros Atlas

Releases are automated with [cargo-dist](https://axodotdev.github.io/cargo-dist)
(`dist-workspace.toml`) and changelog generation with
[git-cliff](https://git-cliff.org) (`cliff.toml`). Versioning follows
[Semantic Versioning](https://semver.org): while on `0.x`, breaking CLI or
output-format changes bump the minor version.

## One-time setup (before the first public release)

1. Create the GitHub repository and push `master`/`main`.
   The `repository`/`homepage` URLs in `Cargo.toml` and `npm/package.json`
   must match the real repo.
2. Create the Homebrew tap repository `snowbros/homebrew-tap` (empty repo
   with a `Formula/` directory is enough).
3. Add a repo secret `HOMEBREW_TAP_TOKEN` — a token with write access to
   the tap — so the release workflow can push the generated formula.
4. Register the npm org `snowbros` (claims the `@snowbros` scope) and
   reserve names on crates.io (`cargo publish` chain, or publish on the
   first release).

## Cutting a release

```sh
# 1. Bump the version (workspace + npm wrapper stay in lock-step).
#    Edit [workspace.package].version in Cargo.toml
#    Edit "version" in npm/package.json
cargo check            # refreshes Cargo.lock

# 2. Regenerate the changelog for the new version.
npx git-cliff --tag vX.Y.Z -o CHANGELOG.md

# 3. Sanity-check the release plan locally.
dist plan
dist build             # builds host-platform artifacts into target/distrib

# 4. Commit, tag, push. The tag triggers .github/workflows/release.yml.
git add -A
git commit -m "chore(release): vX.Y.Z"
git tag vX.Y.Z
git push origin master --tags
```

The release workflow then:

- builds archives for all five targets on native runners,
- generates SHA-256 checksums for every artifact,
- creates the GitHub Release with notes taken from `CHANGELOG.md`,
- uploads `snowbros-atlas-installer.sh` / `snowbros-atlas-installer.ps1`,
- pushes the generated `snowbros.rb` formula to the tap.

## After the workflow succeeds

```sh
# Publish the npm wrapper (it downloads binaries from the new release).
cd npm && npm publish --access public   # @snowbros/atlas (scoped)
# then publish the unscoped `snowbros` alias package (see docs/launch/NAMING.md)

# Publish to crates.io so `cargo install snowbros-atlas --locked` works.
cargo publish -p snowbros_core   # then dependents in dependency order,
cargo publish -p snowbros-atlas  # ending with the CLI package
```

## Verifying a release

```sh
# Windows (PowerShell)
irm https://github.com/snowbros/atlas/releases/latest/download/snowbros-atlas-installer.ps1 | iex
sb --version

# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/snowbros/atlas/releases/latest/download/snowbros-atlas-installer.sh | sh
sb --version

# Homebrew
brew install snowbros/tap/snowbros-atlas && sb --version

# npm (all platforms)
npx snowbros@latest --version

# Checksums
sha256sum -c snowbros-x86_64-unknown-linux-gnu.tar.gz.sha256
```

## Reproducibility

- The Rust toolchain is pinned in `rust-toolchain.toml`.
- `Cargo.lock` is committed; CI and installers build `--locked`.
- Release binaries are built by CI from a tagged commit on clean runners —
  a rebuild of the same tag with the same toolchain produces functionally
  identical binaries (bit-for-bit on Linux; macOS/Windows signatures and
  timestamps may differ).
- Archive checksums are published next to every artifact.

## CI guarantees on every push

`ci.yml` runs fmt, clippy (`-D warnings`), the full test suite on three
OSes, cargo-deny (licenses/advisories), `dist plan`, a check that
`release.yml` is in sync with `dist-workspace.toml`, and the npm wrapper
tests on three OSes.
