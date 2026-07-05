# Installing SNOWBROS Inspector

Every method installs two binaries: `snowbros` and the short alias `sb`.
Prebuilt targets: Windows x64, macOS x64/arm64, Linux x64/arm64 (glibc).

## npm / npx

Requires Node.js ≥ 18. The package downloads the prebuilt binary for your
platform on install (SHA-256 verified) and caches it; after the first run
the wrapper adds only a process spawn.

```sh
npx snowbros analyze          # one-shot
npm install -g snowbros       # global: sb, snowbros on PATH
npm install -D snowbros       # per-project devDependency
```

If the postinstall download is blocked (offline CI), installation still
succeeds and the binary is fetched on first use. To point the wrapper at a
manually installed binary set `SNOWBROS_BINARY_PATH`.

## Shell installer — macOS, Linux

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/snowbros/snowbros-inspector/releases/latest/download/snowbros-installer.sh | sh
```

Installs to `~/.cargo/bin` (or `~/.local/bin`), adds PATH guidance.

## PowerShell installer — Windows

```powershell
irm https://github.com/snowbros/snowbros-inspector/releases/latest/download/snowbros-installer.ps1 | iex
```

## Homebrew — macOS, Linux

```sh
brew install snowbros/tap/snowbros
```

The formula is generated and pushed to the tap automatically by the release
pipeline.

## Cargo — any platform with a Rust toolchain

```sh
cargo install snowbros --locked
```

Builds from source with the locked dependency set (reproducible against the
published `Cargo.lock`).

## Manual — GitHub Releases

1. Download the archive for your platform from
   [releases](https://github.com/snowbros/snowbros-inspector/releases):
   `snowbros-<target>.tar.gz` (Unix) or `snowbros-x86_64-pc-windows-msvc.zip`.
2. Verify: `sha256sum -c <archive>.sha256` (or `certutil -hashfile <archive> SHA256`).
3. Extract and place `sb` / `snowbros` on your PATH.

## Verify the installation

```sh
sb --version
sb analyze --format json   # in any JS/TS project
```

## Editor (LSP) setup

The server speaks LSP over stdio: command `sb`, argument `lsp`.

**VS Code** (with a generic LSP client extension), **Neovim**:

```lua
-- Neovim ≥ 0.10
vim.lsp.config['snowbros'] = {
  cmd = { 'sb', 'lsp' },
  root_markers = { 'snowbros.toml', 'package.json' },
  filetypes = { 'javascript', 'typescript', 'javascriptreact', 'typescriptreact' },
}
vim.lsp.enable('snowbros')
```

**Helix** (`languages.toml`):

```toml
[language-server.snowbros]
command = "sb"
args = ["lsp"]
```

Diagnostics re-publish on file open and save; the whole project is
re-analyzed each time (the incremental cache keeps this at tens of
milliseconds on warm runs).
