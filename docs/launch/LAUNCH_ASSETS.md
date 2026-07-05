# Launch assets

Everything needed to configure the GitHub repository and public profiles
at launch. Copy-paste ready.

## GitHub repository description

> Deterministic static analysis for JS/TS: import graph, dead code,
> Next.js boundary violations, secrets — same code in, same findings out.
> Fast (ms warm runs), SARIF, LSP, auto-fix.

(GitHub limit is ~350 chars; this is 214.)

## GitHub topics

```
static-analysis, typescript, javascript, nextjs, linter, developer-tools,
rust, sarif, lsp, code-quality, architecture, dead-code, dependency-graph,
security, ci
```

(GitHub allows 20 topics; these 15 cover search intent without spam.)

## Website / homepage field

`https://snowbros.me` — the product page lives in the separate
`snowbros/website` repository (built later). Until it's live, leave the
homepage field empty or point it at the repository README. The
`website/` directory in this repo is seed content for that site.

## Social preview image (1280×640)

Full design spec: [SOCIAL_PREVIEW_SPEC.md](SOCIAL_PREVIEW_SPEC.md).
Summary for the repository social card (Settings → Social preview):

- Background: deep navy (#0B1220), subtle dot grid.
- Left 60%: wordmark "Snowbros Atlas" in bold geometric sans
  (e.g. Inter 800), white; below it the tagline in #8FA3BF:
  "Same code in. Same findings out. Every time."
- Right 40%: stylized terminal window showing three findings lines
  (red HIGH, yellow MEDIUM, dimmed LOW) and the health score line
  `◆ health: 92/100`.
- Bottom-left: three small chips: `Rust`, `~40 ms warm`, `SARIF · LSP`.
- No gradients over text; contrast ratio ≥ 4.5:1.

## Logo usage guidelines

Until a designed logo exists, the wordmark is the logo:

- Hierarchy lockup: company eyebrow "SNOWBROS" (small, tracking +8%,
  muted #8FA3BF) above the product wordmark "Atlas" (Inter 800, white on
  navy #0B1220, or navy on white). Full spec:
  [SOCIAL_PREVIEW_SPEC.md](SOCIAL_PREVIEW_SPEC.md).
- Glyph (favicon/avatar): a snowflake-in-hexagon monogram, single color.
  Use ❄ inside a hexagon outline as an interim avatar.
- Don't: stretch, recolor per-letter, place on busy imagery, or attach
  taglines other than the official one.
- Clear space: at least the height of the "A" in Atlas on all sides.
- Naming rules: company is SNOWBROS (caps); the product is always
  "Snowbros Atlas" in prose — never bare "Snowbros"; `sb` is the CLI;
  packages are `@snowbros/atlas` (npm) and `snowbros-atlas` (crates.io).

## Elevator one-liner (everywhere)

> A compiler for engineering problems: deterministic whole-project
> analysis for JavaScript/TypeScript, written in Rust.
