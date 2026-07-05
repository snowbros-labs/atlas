# Real-world examples

SNOWBROS Inspector run against three well-known open-source projects
(shallow clones, release build, Windows x64 laptop). Commits analyzed:
zod `912f0f5`, axios `e435384`, fastify `94bcbcc` (July 2026).

## Performance

| Repository | Files scanned | Cold analysis | Warm analysis |
|---|---|---|---|
| colinhacks/zod | 554 | ~605 ms | ~88 ms |
| axios/axios | 431 | ~230 ms | ~76 ms |
| fastify/fastify | 355 | ~365 ms | ~103 ms |

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

The unresolved imports expose a current, documented limitation: SNOWBROS
resolves directory imports via `index.*` files but does not yet consult
`package.json#main`, so fastify's test-suite pattern of requiring the
package root (`require('../..')` → `fastify.js`) reports as unresolved.
Honest label, no guess — and a config `disable = ["imports/*"]` scopes it
out until the resolver learns `main`.

## Auto-fix on these repos

`sb fix --dry-run` reported **nothing auto-fixable** in all three — no
unused dependencies, no dead `.env` entries. Correct behavior: mature
projects keep manifests clean, and SNOWBROS refuses to "fix" anything it
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
git clone --depth 1 https://github.com/axios/axios && cd axios
npx snowbros analyze          # or: sb analyze
sb analyze --format json | jq .summary
sb fix --dry-run
sb graph --format dot | head
```
