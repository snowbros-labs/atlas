# Snowbros Atlas for VS Code

**Deterministic engineering intelligence for JavaScript/TypeScript, live in your
editor.**

This extension is a thin, first-party client for the
[Snowbros Atlas](https://github.com/snowbros-labs/atlas) language server. It does
**not** contain any analysis logic — the Rust engine (`sb lsp`) remains the
single source of truth. The extension starts that server, streams its findings
into native VS Code diagnostics, and adds a few convenience commands.

## What you get

- **Live diagnostics** — circular imports, dead files, Next.js server/client
  boundary leaks, unused dependencies, hardcoded secrets, and more, as you open
  and save files. Severities map onto native Errors, Warnings, Information, and
  Hints, and clicking a diagnostic jumps to the exact span.
- **Zero-config startup** — the extension finds `sb`/`snowbros` on your PATH,
  honors an explicit `atlas.path`, or falls back to `npx snowbros` so it works
  even before you install anything globally.
- **Status bar** — Atlas Ready / Running / Error at a glance; click it for the
  project health score.
- **Commands** — analyze, restart, explain a rule, open a report, show health,
  clear the cache.

## Requirements

The extension needs the Atlas binary. Any one of these works:

```sh
npm install -g @snowbros/atlas     # provides `sb` and `snowbros`
brew install snowbros-labs/tap/snowbros-atlas
cargo install snowbros-atlas --locked
```

No global install? The extension will use `npx snowbros` automatically (slower
on first run while npx fetches the package). You can also point `atlas.path` at
a binary directly.

## Commands

Open the Command Palette (`Ctrl/Cmd+Shift+P`) and type **Atlas**:

| Command | Description |
|---|---|
| **Atlas: Analyze Workspace** | Force a full re-analysis and refresh all diagnostics. |
| **Atlas: Restart Language Server** | Restart `sb lsp`. |
| **Atlas: Explain Rule** | Show a rule's detection logic and fix guidance (seeded from the diagnostic under your cursor). |
| **Atlas: Open Report** | Generate and open a report in your configured format (HTML by default). |
| **Atlas: Show Health Score** | Compute the project health score and category breakdown. |
| **Atlas: Clear Cache** | Delete `.snowbros/cache.json` and optionally re-analyze. |

## Settings

| Setting | Default | Description |
|---|---|---|
| `atlas.enable` | `true` | Enable the extension and language server. |
| `atlas.path` | `""` | Absolute path to `sb`/`snowbros`. Empty = auto-detect, then `npx`. |
| `atlas.autoAnalyze` | `true` | Start the server automatically and analyze on open/save. When off, analysis runs only via **Atlas: Analyze Workspace**. |
| `atlas.useCache` | `true` | Use the incremental cache for report/health commands. |
| `atlas.logLevel` | `info` | Output-channel and server (`RUST_LOG`) verbosity. |
| `atlas.format` | `html` | Report format for **Atlas: Open Report** (`html`/`json`/`markdown`/`sarif`). |
| `atlas.enableStatusBar` | `true` | Show the Atlas status bar item. |

## How it works

```
VS Code  ──stdio──▶  sb lsp  (Rust language server, the source of truth)
   ▲                    │
   └── native diagnostics, health, reports
```

The extension uses [`vscode-languageclient`](https://www.npmjs.com/package/vscode-languageclient)
to spawn `sb lsp` over stdio. The server analyzes the whole project through the
Atlas engine (cache-accelerated) on startup, open, and save, and publishes
diagnostics per file. Report, health, and explain commands shell out to the same
CLI so there is never a second implementation to drift.

## Troubleshooting

- **"Could not launch …"** — install the binary (see Requirements) or set
  `atlas.path`. Check the **Snowbros Atlas** output channel for details.
- **No diagnostics** — confirm `atlas.enable` and `atlas.autoAnalyze` are on, and
  that the folder contains JS/TS files. Run **Atlas: Analyze Workspace** to force
  a pass.
- **Slow first run** — the `npx` fallback downloads the package once; a global
  install avoids this.

## Contributing & license

Part of the [Snowbros Atlas](https://github.com/snowbros-labs/atlas) project. See
[DEVELOPMENT.md](DEVELOPMENT.md) to build and test the extension. Licensed under
MIT OR Apache-2.0.
