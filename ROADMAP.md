# Roadmap

The public roadmap for Snowbros Atlas. This is direction, not a dated
commitment — priorities shift with real-world feedback. Track live work in the
[issue tracker](https://github.com/snowbros-labs/atlas/issues) and propose ideas
in [Discussions](https://github.com/snowbros-labs/atlas/discussions).

Atlas ships in phases. **Phase 1 is done**: the deterministic JS/TS analysis
core, the CLI, the LSP server, the VS Code extension, and multi-platform
releases.

## Now — accuracy & adoption

The near-term focus is making the existing analysis more accurate on real repos
and lowering the bar to try it.

- **`package.json#main`/`exports` resolution** — the largest confirmed source of
  false "unresolved import" findings (e.g. fastify's package self-imports).
- **Monorepo / workspace awareness** — resolve across multiple `package.json`
  and tsconfig roots without config workarounds.
- **VS Code Marketplace publish** — ship the existing extension.
- **Shell completions and `sb doctor`** — CLI quality-of-life.

## Next — depth

- **More rules**, prioritized by real issue reports rather than a target count.
  Each must clear the [correctness bar](docs/adding-a-rule.md).
- **Rule maturity gating** — a `nursery` tier, off by default.
- **Wider auto-fix coverage** — more rules gain guarded, idempotent fixers.

## Later — extensibility

- **Pattern rule engine** — author rules without writing Rust (Semgrep-style).
- **OSV / vulnerability data** — flag known-vulnerable dependencies with
  evidence.
- **A second language family** (Python is the most-detected non-JS candidate),
  only once the JS/TS core proves the model.

## Under consideration

Symbol-level resolution, multi-repo trend dashboards, and a plugin system are
tracked but deliberately unscheduled — they follow demand and the accuracy work
above.

## Principles that will not change

- **Deterministic**: same code and config in, same findings out.
- **Provable**: no finding without evidence; unknowns are reported as unknown,
  never guessed.
- **Fast**: incremental and native.

See [VERSIONING.md](VERSIONING.md) for how roadmap changes map onto releases.
