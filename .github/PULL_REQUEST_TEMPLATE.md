<!--
Thanks for contributing to Snowbros Atlas! Please fill out the sections below.
Keep the PR focused — one logical change per pull request is easier to review.
-->

## Summary

<!-- What does this PR do, and why? Link any related issue: Closes #123 -->

## Type of change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New rule (adds a detection — see `docs/adding-a-rule.md`)
- [ ] Feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that changes existing behavior)
- [ ] Documentation only
- [ ] Build / CI / tooling

## Checklist

- [ ] `cargo fmt --all` reports no changes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] New or changed behavior has tests (positive, negative, and — for rules —
      a false-positive guard case)
- [ ] Findings remain **deterministic**: no timestamps, no unsorted output, no
      HashMap iteration order leaking into results
- [ ] Any new finding carries an **evidence chain** and a confidence level
- [ ] Docs updated if behavior, commands, or config changed
- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)

## Notes for reviewers

<!-- Anything that needs special attention: tricky edge cases, follow-ups,
     benchmarks, screenshots of terminal/HTML output, etc. -->
