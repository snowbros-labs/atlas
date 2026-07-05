# Contributing to Snowbros Atlas

Thanks for helping! This project has a few hard rules that keep it what it
is — read these before opening a PR.

By participating you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Where to start

- **Questions or ideas?** Open a [Discussion][discussions] — not an issue.
- **Found a bug?** File a [bug report][bugs]; a minimal reproduction helps most.
- **Want to write a rule?** That is the most valuable contribution to a rules
  engine. Read **[docs/adding-a-rule.md](docs/adding-a-rule.md)** first.
- **Looking for a starting task?** Check issues labelled
  [`good first issue`][gfi].

[discussions]: https://github.com/snowbros-labs/atlas/discussions
[bugs]: https://github.com/snowbros-labs/atlas/issues/new?template=bug_report.yml
[gfi]: https://github.com/snowbros-labs/atlas/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22

## Ground rules

1. **Determinism is non-negotiable.** Findings must be a pure function of
   the analyzed code and config. No timestamps in reports, no HashMap
   iteration order leaking into output, no network calls during analysis.
   Collections that reach output are sorted.
2. **Accuracy over quantity.** A rule that cannot prove its finding must
   not report it. Unresolvable imports are reported as *unresolved*, never
   guessed. Every rule documents its false-positive guards in its metadata.
3. **Evidence is mandatory.** Every diagnostic carries the concrete chain
   that produced it (imports, spans, config lines).
4. **The cache may never change results.** Warm output must be
   byte-identical to cold output; e2e tests enforce this.
5. **Secrets are always redacted** to their first 4 characters, everywhere.

## Development setup

```sh
git clone https://github.com/snowbros-labs/atlas
cd atlas
cargo test --workspace
```

The toolchain is pinned in `rust-toolchain.toml`. On Windows use the MSVC
toolchain.

Before pushing:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd npm && npm test)      # if you touched the npm wrapper
```

## Adding a rule

Rules are held to a high correctness bar (determinism, provable findings,
mandatory evidence, honest confidence, secret redaction). The full walkthrough —
metadata, detector, tests, and fixers — lives in
**[docs/adding-a-rule.md](docs/adding-a-rule.md)**. In short:

1. Implement it in `crates/snowbros_rules`.
2. Add metadata: `crates/snowbros_rules/rules/<category>/<name>.toml` (embedded at compile time).
   The test harness enforces 1:1 rule↔metadata mapping and metadata
   completeness — a missing file fails the build.
3. Document false-positive guards in the metadata.
4. Add tests: positive case, negative case, and at least one FP guard case.
5. If the rule is mechanically fixable, add a fixer in
   `crates/snowbros_cli/src/fixers.rs` — fixers must be guarded (skip on
   file drift) and idempotent.

## Commit style

Conventional Commits (`feat:`, `fix:`, `perf:`, `docs:`, …). The changelog
is generated from commit messages by git-cliff, so write subjects that read
well in a changelog.

## Crate layout

`snowbros_engine::analyze()` is the single entry point for analysis — the
CLI, the LSP server, and benchmarks all go through it. Don't build a second
pipeline; extend the engine.

## License

Contributions are dual-licensed under MIT OR Apache-2.0, like the project.
