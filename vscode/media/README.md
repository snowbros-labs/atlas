# Extension assets

- `icon.svg` / `icon-dark.svg` — the real Snowbros Atlas logo (light/dark),
  copied from the repo's `assets/`.

## TODO before Marketplace publish

The VS Code Marketplace requires a **128×128 PNG** icon; SVG is not accepted.
Export one from the logo and wire it up:

```sh
# example, any SVG→PNG tool works
rsvg-convert -w 128 -h 128 icon.svg -o icon.png
```

Then add to `package.json`:

```json
"icon": "media/icon.png"
```

A gallery banner image is optional (`galleryBanner.color` is already set). Add a
`banner.png` here and reference it if desired. These images are intentionally
left as a TODO rather than machine-generated placeholders.
