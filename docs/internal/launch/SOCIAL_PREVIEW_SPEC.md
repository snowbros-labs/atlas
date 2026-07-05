# Social preview image — design specification

For GitHub repository social card and link unfurls (Open Graph /
Twitter). Design spec only; produce in Figma or similar.

## Canvas

- **Size:** 1280×640 px (2:1). Design at 2× (2560×1280) and downscale.
- **Safe margins:** keep all text/logo ≥ 64 px from every edge.
  Platforms crop unpredictably: keep critical content inside the center
  1120×512 region. Nothing legible in the outer 32 px.

## Grid and layout

12-column grid, 64 px outer gutters, 24 px column gaps.

- **Left block (columns 1–7):** brand + copy, vertically centered.
  - Row 1 — company eyebrow: `SNOWBROS` — 28 px, tracking +8%,
    color #8FA3BF, uppercase.
  - Row 2 — product wordmark: `Atlas` — 120 px, weight 800,
    color #FFFFFF, -2% tracking. Baseline 16 px below eyebrow.
  - Row 3 — tagline: `Same code in. Same findings out. Every time.` —
    36 px, weight 500, color #C7D4E8, max width 620 px, line-height 1.25,
    40 px below wordmark.
  - Row 4 — CTA chip: rounded rect (radius 12 px), fill #1E2A41,
    1 px stroke #2C3B58, inner padding 14×22 px, containing
    `npx snowbros analyze` in 30 px monospace #5EB1FF. 48 px below
    tagline.
- **Right block (columns 8–12):** stylized terminal card.
  - Card: 460×360 px, radius 16 px, fill #0A0F1A, 1 px stroke #1E2A41,
    drop shadow 0/24/64 px at 40% black. Three 12 px traffic dots top-left
    (#FF5F57, #FEBC2E, #28C840) — decorative, 40% opacity.
  - Content, 26 px monospace, line-height 1.7, 28 px padding:
    - `HIGH  server-only-in-client` — `HIGH` in #FF6B6B bold
    - `  Dashboard.tsx → db.ts` — #C7D4E8, arrow in #5EB1FF
    - `MEDIUM  unresolved-import` — `MEDIUM` in #FFD166
    - `LOW  unused-dependency ✚fix` — #6C7A92, `✚fix` in #5DD39E
    - blank line
    - `◆ health: 92/100` — diamond #5DD39E, number #FFFFFF bold
- **Bottom-left strip (inside safe margin):** three chips, 24 px
  monospace, same chip style as CTA but 10×16 px padding, 16 px apart:
  `Rust` · `~40 ms warm` · `SARIF · LSP`. Color #8FA3BF.

## Color palette

| Role | Hex |
|---|---|
| Background | #0B1220 |
| Background texture | 2 px dot grid, #16233A at 30%, 32 px spacing, fade to 0% at edges |
| Panel / chips | #111A2C / #1E2A41 |
| Strokes | #1E2A41, #2C3B58 |
| Primary text | #FFFFFF |
| Secondary text | #C7D4E8 |
| Muted / eyebrow | #8FA3BF |
| Accent (CTA, arrows) | #5EB1FF |
| Severity high / medium / low | #FF6B6B / #FFD166 / #6C7A92 |
| Success / health | #5DD39E |

Contrast: all copy ≥ 4.5:1 against its background (verified for the
values above); severity colors used at ≥ 26 px only.

## Typography

- Sans: **Inter** (fallback: system geometric sans). Eyebrow 28/800,
  wordmark 120/800, tagline 36/500.
- Mono: **JetBrains Mono** (fallback: Cascadia Code). Terminal 26/500,
  CTA 30/600, chips 24/500.
- No more than these two families. No italics. No gradients on text.

## Icons

Only two: the ❄-in-hexagon interim monogram (48 px, stroke 2 px,
#5EB1FF at 80%) placed left of the eyebrow, and the `◆` health diamond
inside the terminal card. No other decoration.

## Export

- PNG, sRGB, 1280×640 (from the 2× master). Keep under 1 MB (GitHub
  limit 5 MB; smaller unfurls faster).
- Also export 1200×630 variant (exact OG ratio) for the website later.
- Test: GitHub repo settings preview, X card validator, LinkedIn post
  inspector, and a Slack unfurl. Verify the wordmark survives the
  center-crop each platform applies.
