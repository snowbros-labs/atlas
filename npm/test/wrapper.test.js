// Tests for the npm wrapper: platform mapping, artifact naming, and
// end-to-end shim execution against a stub binary.

"use strict";

const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const { spawnSync } = require("node:child_process");

const platform = require("../lib/platform");
const pkg = require("../package.json");

test("every supported platform maps to a dist target", () => {
  const expected = {
    "win32-x64": "x86_64-pc-windows-msvc",
    "darwin-x64": "x86_64-apple-darwin",
    "darwin-arm64": "aarch64-apple-darwin",
    "linux-x64": "x86_64-unknown-linux-gnu",
    "linux-arm64": "aarch64-unknown-linux-gnu",
  };
  for (const [key, triple] of Object.entries(expected)) {
    const [plat, arch] = key.split("-");
    assert.strictEqual(platform.target(plat, arch).triple, triple);
  }
});

test("unsupported platforms return null", () => {
  assert.strictEqual(platform.target("freebsd", "x64"), null);
  assert.strictEqual(platform.target("win32", "ia32"), null);
});

test("archive names match cargo-dist output", () => {
  assert.strictEqual(
    platform.archiveName(platform.target("win32", "x64")),
    "snowbros-atlas-x86_64-pc-windows-msvc.zip"
  );
  assert.strictEqual(
    platform.archiveName(platform.target("linux", "arm64")),
    "snowbros-atlas-aarch64-unknown-linux-gnu.tar.gz"
  );
});

test("download url embeds the package version as the tag", () => {
  const url = platform.downloadUrl(pkg.version, platform.target("darwin", "arm64"));
  assert.strictEqual(
    url,
    `https://github.com/snowbros/atlas/releases/download/v${pkg.version}/snowbros-atlas-aarch64-apple-darwin.tar.gz`
  );
});

test("shim runs the binary from SNOWBROS_BINARY_PATH and forwards args + exit code", () => {
  // Stub binary: echoes its args and exits 7.
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sbtest-"));
  const isWin = process.platform === "win32";
  const stub = path.join(dir, isWin ? "stub.cmd" : "stub.sh");
  if (isWin) {
    fs.writeFileSync(stub, "@echo stub-args %*\r\n@exit /b 7\r\n");
  } else {
    fs.writeFileSync(stub, '#!/bin/sh\necho "stub-args $@"\nexit 7\n');
    fs.chmodSync(stub, 0o755);
  }

  const result = spawnSync(
    process.execPath,
    [path.join(__dirname, "..", "bin", "sb.js"), "analyze", "--format", "json"],
    {
      env: { ...process.env, SNOWBROS_BINARY_PATH: stub },
      encoding: "utf8",
      shell: false,
    }
  );
  assert.match(result.stdout, /stub-args analyze --format json/);
  assert.strictEqual(result.status, 7);
  fs.rmSync(dir, { recursive: true, force: true });
});

test("extract + findFile locate binaries in a nested archive", () => {
  const { extract, findFile } = require("../lib/install");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "sbextract-"));
  const inner = path.join(dir, "payload", "nested");
  fs.mkdirSync(inner, { recursive: true });
  fs.writeFileSync(path.join(inner, "sb-marker.txt"), "hello");

  const archive = path.join(dir, "a.tar.gz");
  const tar = spawnSync("tar", ["-czf", "a.tar.gz", "payload"], { cwd: dir });
  assert.strictEqual(tar.status, 0);

  const out = path.join(dir, "out");
  fs.mkdirSync(out);
  extract(archive, out);
  const found = findFile(out, "sb-marker.txt");
  assert.ok(found, "marker not found after extraction");
  assert.strictEqual(fs.readFileSync(found, "utf8"), "hello");
  fs.rmSync(dir, { recursive: true, force: true });
});

test("npm package version matches the Cargo workspace version", () => {
  const cargo = fs.readFileSync(path.join(__dirname, "..", "..", "Cargo.toml"), "utf8");
  const m = cargo.match(/\[workspace\.package\][^[]*version\s*=\s*"([^"]+)"/);
  assert.ok(m, "workspace version not found");
  assert.strictEqual(pkg.version, m[1]);
});
