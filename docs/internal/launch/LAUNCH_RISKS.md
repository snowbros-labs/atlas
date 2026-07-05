# Launch risk register — v0.1.0

Mental simulation of the full release flow. Ranked by likelihood ×
impact. Mitigations are in place unless marked OPEN.

## High likelihood

1. **crates.io publish rate/wait failures.** Twelve crates published in
   dependency order; the index can lag 1–2 minutes, so a dependent
   publish can fail with "version not found". Mitigation: publish
   sequentially, retry after a minute; the runbook orders them.
   crates.io also enforces a burst limit (~1 new crate/min for new
   publishers) — budget 20–30 minutes.
2. **First `npm publish` of a scoped package defaults to private.**
   `--access public` is in the runbook; forgetting it errors on free
   orgs (harmless, rerun with the flag).
3. **Homebrew publish job 403.** Token scope wrong, secret name typo'd,
   or token expired between setup and tag. Mitigation: HOMEBREW_SETUP.md
   troubleshooting; re-run just the publish job after fixing — no
   re-tag needed.

## Medium likelihood

4. **aarch64-unknown-linux-gnu build failure on CI.** Cross-compiled on
   ubuntu runners via dist's toolchain; we've never exercised it (no ARM
   Linux locally). If it fails: drop the target from
   `dist-workspace.toml`, regenerate, re-tag as v0.1.1. OPEN until first
   release run.
5. **`sha256.sum`/formula digests mismatch after a partial re-run.**
   Re-running individual build jobs after a failure can desync digests.
   Mitigation: on any build failure, delete the release + tag and re-tag
   clean (abort criteria in the runbook).
6. **npm wrapper download URL vs release assets.** Wrapper builds
   `snowbros-atlas-<triple>.<ext>` from its own table; tests pin the
   exact names and `dist plan` confirmed them, but if dist ever renames
   (e.g. version bump changes app name), wrapper breaks. Mitigation:
   npm test asserts names; keep the version-lockstep test green.
7. **Windows Defender/SmartScreen flags unsigned binaries.** No code
   signing at 0.1. Users see a warning on the PowerShell installer path.
   Documented reality for new OSS tools; note in README FAQ post-launch
   if reports come in. OPEN (signing is a paid cert, deferred).

## Low likelihood

8. **GitHub Release auto-notes vs curated notes.** dist generates a body
   from CHANGELOG.md; runbook overwrites with RELEASE_NOTES via
   `gh release edit` — cosmetic either way.
9. **`security@snowbros.me` bounces.** SECURITY.md lists it; mailbox
   doesn't exist yet. Mitigation: create before announcing, or GitHub
   advisories remain the primary channel (already first in the doc).
   OPEN.
10. **Org name squatting between now and launch.** `snowbros` npm org /
    GitHub org taken by someone else first. Mitigation: runbook step 0
    does registrations before anything public. Do it today.
11. **cargo-deny advisory lands between last CI run and tag.** A new
    RUSTSEC advisory can turn CI red without a code change. Mitigation:
    it gates CI, not the tag-triggered release workflow; triage and
    patch as v0.1.1 if it happens.
12. **License copyright lines.** The final audit caught LICENSE-MIT and
    LICENSE-APACHE still naming "SNOWBROS Inspector Contributors"
    (extensionless files escaped the rebrand sweep) — fixed to
    "Snowbros Atlas Contributors" in this pass. Lesson recorded: audit
    sweeps must not filter by file extension.

## Not risks (verified this session)

- Version lock-step (Cargo 0.1.0 == npm 0.1.0, test-enforced).
- `cargo package` file collection for `snowbros_rules` (rules moved
  into the crate; was a real blocker, fixed).
- Internal path deps carry `version = "0.1.0"` (was a real blocker,
  fixed — `cargo publish` would have rejected version-less path deps).
- Workflow YAML validity, trigger branches (main + master), tag pattern.
- All doc links resolve; no stale brand strings; artifact names
  consistent across dist plan, npm wrapper, docs, and runbook.
