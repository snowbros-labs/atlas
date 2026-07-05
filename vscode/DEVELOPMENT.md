# Development & publishing

Development guide for the Snowbros Atlas VS Code extension.

## Layout

```
vscode/
├── package.json          Manifest: commands, settings, activation, metadata
├── esbuild.js            Bundles src/extension.ts → out/extension.js
├── tsconfig.json         Strict TypeScript config
├── src/
│   ├── extension.ts      activate/deactivate, wiring, config reactions
│   ├── client.ts         LanguageClient lifecycle (start/stop/restart)
│   ├── resolve.ts        Locate the sb/snowbros binary (config → PATH → npx)
│   ├── cli.ts            Run `sb <args>` with timeout + graceful errors
│   ├── commands.ts       The six atlas.* command implementations
│   ├── statusBar.ts      Status bar item (Ready/Running/Error/health)
│   ├── config.ts         Typed settings snapshot + RUST_LOG mapping
│   └── logger.ts         Level-filtered output channel
├── test/                 @vscode/test-electron + Mocha suite
└── media/                Icon and banner assets
```

The Rust language server (`sb lsp`) lives in `../crates/snowbros_lsp` and is the
source of truth. This extension never re-implements analysis.

## Prerequisites

- Node.js ≥ 18
- The Atlas binary for manual testing: `npm i -g @snowbros/atlas`, or build the
  workspace with `cargo build --release` and point `atlas.path` at
  `../target/release/sb`.

## Setup

```sh
cd vscode
npm install
npm run build          # bundle to out/extension.js
```

## Run it

Open the `vscode/` folder in VS Code and press `F5` (Run Extension). This opens
an Extension Development Host with Atlas loaded. Open a JS/TS project inside it.

Useful scripts:

```sh
npm run watch          # rebuild on change
npm run lint           # eslint
npm run check-types    # tsc --noEmit
npm test               # compile + run the extension test suite (headless)
```

## Tests

`npm test` compiles `src` and `test` with `tsc` (to `out-tests/`) and runs the
Mocha suite under a headless VS Code via `@vscode/test-electron`. On Linux CI
this requires a virtual display — see the workflow, which wraps it in
`xvfb-run`.

Coverage: binary resolution (config/PATH/PATHEXT/npx), report JSON parsing,
extension activation, command registration, and configuration defaults.

## Packaging

```sh
npm run package        # → snowbros-atlas.vsix
```

`vsce package --no-dependencies` is used because the runtime code is already
bundled by esbuild; only `out/extension.js` and assets ship in the `.vsix`
(see `.vscodeignore`).

Install the built package locally:

```sh
code --install-extension snowbros-atlas.vsix
```

## Publishing to the Marketplace

1. Create/confirm the `snowbros` publisher at
   <https://marketplace.visualstudio.com/manage>.
2. Generate a Personal Access Token (Azure DevOps, scope **Marketplace →
   Manage**).
3. Add the real `media/icon.png` (128×128) and `media/banner` assets.
4. Bump the version and update `CHANGELOG.md`.
5. Publish:

   ```sh
   npx @vscode/vsce login snowbros
   npx @vscode/vsce publish
   ```

   Or attach the `.vsix` to a GitHub release and publish via
   `vsce publish --packagePath snowbros-atlas.vsix`.

Optionally mirror to the Open VSX registry with `npx ovsx publish`.

## Release checklist

- [ ] `npm run check-types` and `npm run lint` clean
- [ ] `npm test` green
- [ ] `npm run package` produces a `.vsix`
- [ ] `media/icon.png` present (marketplace requires it)
- [ ] `CHANGELOG.md` updated, version bumped in `package.json`
