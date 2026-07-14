# Real-world examples

Snowbros Atlas run against well-known open-source projects (shallow clones,
release build, Windows x64 laptop). Commits analyzed: zod `912f0f5`,
axios `e435384`, fastify `94bcbcc` (July 2026); fastapi `b1346bb` (v0.4.0,
July 2026) — the first Python dogfood.

## Performance

| Repository | Files scanned | Cold analysis | Warm analysis |
|---|---|---|---|
| colinhacks/zod | 554 | ~605 ms | ~88 ms |
| axios/axios | 431 | ~230 ms | ~76 ms |
| fastify/fastify | 355 | ~365 ms | ~103 ms |
| fastapi/fastapi (Python) | 48 | ~400 ms | ~61 ms |

Warm runs reuse the incremental cache (`.snowbros/cache.json`); output is
byte-identical to a cold run.

## What it found

### zod — 126 findings, health 83/100

```text
HIGH  Circular import cycle [graph/no-circular-imports]
  at packages/zod/src/v3/ZodError.ts · confidence: certain
HIGH  Circular import cycle [graph/no-circular-imports]
  at packages/zod/src/v4/core/api.ts · confidence: certain
LOW   86 potentially dead files · 38 unused exports
```

Both cycles are real, long-lived structural facts of the codebase — the
kind of thing that is invisible in per-file linting and obvious in a
whole-project graph. The dead-file findings concentrate in `packages/bench`
and docs tooling, which a config exclude would silence.

Notable: zod uses TypeScript-ESM `./util.js`-style imports for `.ts`
sources throughout. The resolver applies TypeScript's own extension
substitution, so all 524 of those imports resolve — zero false
"unresolved import" findings.

### axios — 12 findings, health 97/100

```text
MEDIUM Hardcoded credential candidate [security/hardcoded-secret]
  at tests/module/cjs/tests/helpers/cjs-typing.ts · confidence: possible
    - a credential-named binding is assigned a long literal; value redacted to `s00p…`
LOW    7 potentially dead files · 3 unused exports
```

The flagged value is a fixture credential in test helpers — reported at
`possible` confidence and redacted to four characters, exactly as
designed. A clean, mature codebase scores 97.

### fastify — 91 findings, health 73/100

```text
HIGH   Circular import cycle [graph/no-circular-imports]
  at fastify.d.ts · confidence: certain
MEDIUM 51 unresolved imports (mostly `require('../..')` package self-imports)
LOW    39 potentially dead files
```

The unresolved imports expose a current, documented limitation: Snowbros Atlas
resolves directory imports via `index.*` files but does not yet consult
`package.json#main`, so fastify's test-suite pattern of requiring the
package root (`require('../..')` → `fastify.js`) reports as unresolved.
Honest label, no guess — and a config `disable = ["imports/*"]` scopes it
out until the resolver learns `main`.

### fastapi (Python) — 23 findings, health 92/100

The first Python dogfood, and the proof that the shared IR and rule engine
carry across languages. Analyzing the `fastapi/` package itself:

```text
LOW  14 large functions [complexity/large-function]
  e.g. jsonable_encoder (146 lines), analyze_param (160), solve_dependencies (124)
HIGH  2 circular import cycles [graph/no-circular-imports]
  _compat/__init__.py ↔ _compat/v2.py; utils.py `import fastapi` ↔ __init__ re-export
LOW   7 potentially dead files [graph/dead-file]
  middleware/cors.py, staticfiles.py, templating.py, testclient.py, …
```

Every finding is language-neutral by construction: `complexity/large-function`
reads only the shared IR (function body size), so the same rule flags oversized
functions in Python exactly as it does in TypeScript. The two cycles are real
module-level import cycles present in FastAPI's source (pylint's
`cyclic-import` flags them too). The seven dead files are public-API
re-export leaves with no *internal* importer — a language-agnostic limitation
of whole-program dead-code analysis on a library, not a Python quirk.

**Zero Python-specific false positives.** An earlier run reported 24 dead
files; dogfooding surfaced a real resolver gap — absolute self-package imports
(`from fastapi.encoders import x`) were unresolved when the scan root *is* the
`fastapi` package — which v0.4.0 fixed. No rule contains an `if language ==`
branch: a rule is either language-agnostic or scoped to a language family in
one place (the scheduler).

## Auto-fix on these repos

`sb fix --dry-run` reported **nothing auto-fixable** in all three — no
unused dependencies, no dead `.env` entries. Correct behavior: mature
projects keep manifests clean, and Snowbros Atlas refuses to "fix" anything it
cannot prove. (On a repo with an unused dependency, the fix is a
format-preserving `package.json` edit; devDependencies are never touched.)

## Dependency graphs

`sb graph --format dot` exports the semantic import graph:

| Repository | DOT graph size |
|---|---|
| zod | 1,533 lines |
| axios | 971 lines |

Render with Graphviz: `sb graph --format dot | dot -Tsvg > graph.svg`.

## Reproduce

```sh
# JavaScript / TypeScript
git clone --depth 1 https://github.com/axios/axios && cd axios
npx snowbros analyze          # or: sb analyze
sb analyze --format json | jq .summary
sb fix --dry-run
sb graph --format dot | head

# Python — point it at the package directory
git clone --depth 1 https://github.com/fastapi/fastapi && cd fastapi
sb analyze fastapi            # the importable package dir
sb analyze fastapi --format json | jq '.summary.by_rule'
```
