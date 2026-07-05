# 90-second demo — full production plan

Everything except the recording itself. Target: 90 s ± 3 s, 60 fps
terminal capture, no voiceover, burned-in captions.

## Prep (before recording)

Demo repo: shallow clone of `axios/axios` (real, recognizable, fast, and
scores 97 — pair it with a planted-issue branch for the fix beat).

```sh
git clone --depth 1 https://github.com/axios/axios demo && cd demo
npm pkg set dependencies.left-pad="^1.3.0"      # plant one unused dependency
printf "OLD_API_URL=http://legacy.internal\n" > .env   # plant one dead env var
sb analyze          # prime: verify findings appear as expected
sb analyze          # prime the cache for the speed beat
git init >/dev/null 2>&1 || true                # ensure `git diff` works for the fix beat
```

Terminal: 120×30, dark theme (#0B1220 background), font ≥ 18 pt
(Cascadia Code / JetBrains Mono), prompt shortened to `$`. Clear scroll
history before each take. Windows Terminal or iTerm2; disable
transparency and animations.

## Shot list — commands, expected output, camera flow

| # | Time | Command | Expected on screen | Camera |
|---|---|---|---|---|
| 1 | 0:00–0:08 | type `npx snowbros analyze` slowly | command line only | full-frame terminal, cursor blink |
| 2 | 0:08–0:22 | (enter) | header `Snowbros Atlas · analyze`, `files scanned: ~431`, `frameworks: express`, findings incl. `LOW Unused dependency "left-pad" [deps/unused-dependency]`, `LOW Unused environment variable OLD_API_URL`, health line `◆ health: 9x/100` | hold; slow 10% zoom toward findings block |
| 3 | 0:22–0:35 | `sb analyze --format json \| sha256sum` twice | two identical hashes stacked | cut; highlight both hashes with caption |
| 4 | 0:35–0:50 | `sb analyze` again, then `touch lib/axios.js && sb analyze` | cache line `431 reused, 0 parsed` then `430 reused, 1 parsed`; sub-100 ms feel | cut; underline cache line |
| 5 | 0:50–1:05 | `sb fix --dry-run` then `sb fix` then `git diff --stat` | plan lists 2 fixes; apply prints `✓ 2 fix(es) changed 2 file(s)`; diff shows `package.json` + `.env`, 2 files changed | cut; end on the diff |
| 6 | 1:05–1:20 | editor open on a file with `eval(` + LSP attached; side panel: GitHub Security tab (SARIF screenshot) | red squiggle, hover shows `security/no-eval · snowbros` | split-screen, 7 s each |
| 7 | 1:20–1:30 | logo card (static) | wordmark + `npx snowbros analyze` + github.com/snowbros-labs/atlas | fade in, hold 6 s |

## Captions (burn-in, one line, bottom center)

1. (0:02) "Your codebase has problems it can prove."
2. (0:12) "Every finding comes with evidence. No guesses."
3. (0:25) "Deterministic: same code in, same findings out. Byte-identical."
4. (0:40) "Incremental: warm runs in milliseconds. Run it on every save."
5. (0:54) "Fixes only what it can prove. Idempotent. Never clobbers."
6. (1:08) "LSP in your editor. SARIF in your CI. One engine, same answers."
7. (1:22) "Snowbros Atlas — open source, Rust, MIT/Apache."

## Recording checklist

- [ ] Fresh demo clone primed (cache warm where the script expects it)
- [ ] Planted findings verified present before rolling
- [ ] Terminal size/font/theme per spec; OS notifications off; clock hidden
- [ ] Capture at 60 fps, lossless or high-bitrate (OBS: CRF ≤ 18)
- [ ] Each shot recorded separately; 2 s of padding before/after
- [ ] Typing speed natural (use a typer script if takes are inconsistent)
- [ ] Shot 6 editor: font matches terminal; LSP confirmed attached before roll

## Editing checklist

- [ ] Total length 87–93 s; per-shot times within ±1 s of the table
- [ ] Cuts on command boundaries, never mid-output
- [ ] Captions: white #E6EDF7, 60% black backing bar, 4 % bottom margin, ≤ 42 chars
- [ ] Zoom in shot 2 eased (no linear zoom); max 110%
- [ ] Hash comparison in shot 3 visually highlighted (box or brighten)
- [ ] End card holds ≥ 5 s; URL spelled `github.com/snowbros-labs/atlas`
- [ ] Export: 1920×1080 H.264 MP4 (launch posts) + 1280×720 GIF ≤ 10 MB
      (README, first 30 s only) — README embeds the GIF, links the MP4
- [ ] Color check on a light-mode screen (captions still readable)

## Thumbnail concept

1280×720. Left two-thirds: dark terminal (#0B1220) showing exactly three
lines — a red `HIGH` finding, the dimmed rule id, and the green
`◆ health: 87/100` line, font large enough to read at 320 px wide.
Right third: vertical wordmark block — "SNOWBROS" small caps in muted
blue (#8FA3BF) above "Atlas" large white; beneath it a single yellow
chip: `~40 ms warm`. No faces, no arrows, no red circles.
