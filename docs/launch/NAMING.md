# Naming decision record

Finalized 2026-07-05.

## Hierarchy

- **Company:** SNOWBROS (GitHub org `snowbros`)
- **Product:** Snowbros Atlas — always the full name in prose, never
  bare "Snowbros"
- **CLI executables:** `sb` (primary) and `snowbros` (long form) —
  unchanged
- **Repository:** `snowbros/atlas`

## Package availability (checked 2026-07-05)

| Registry | Name | Status |
|---|---|---|
| npm | `snowbros` | available |
| npm | `@snowbros/atlas` | available (scope free) |
| crates.io | `snowbros` | available |
| crates.io | `snowbros-atlas` | available |
| crates.io | `snowbros_core` (spot check of lib crates) | available |

## Decisions

- **crates.io CLI package: `snowbros-atlas`.** The company-generic name
  `snowbros` is deliberately *not* used for one product — the SNOWBROS
  ecosystem will ship more. Cargo package name also drives cargo-dist
  artifact names (`snowbros-atlas-<target>.tar.gz`), installer names
  (`snowbros-atlas-installer.sh/.ps1`), and the Homebrew formula
  (`snowbros-atlas.rb` → `brew install snowbros/tap/snowbros-atlas`).
  Library crates keep their `snowbros_*` names.
- **npm canonical package: `@snowbros/atlas`** (register the `snowbros`
  npm org first). Reflects the ecosystem hierarchy and can never be
  squatted once the org exists.
- **npm alias package: `snowbros`** — publish a thin package that
  depends on `@snowbros/atlas` and re-exposes its bins, so
  `npx snowbros analyze` works and the brand name is protected on the
  registry. Marketing copy uses `npx snowbros analyze`.
- **Binary names unchanged:** `sb` and `snowbros` — install commands
  differ per registry but the executables are identical everywhere.
- Consider registering `snowbros` on crates.io as a reserved stub
  (pointing at `snowbros-atlas`) for brand protection; optional, low
  priority.
