// Downloads the SNOWBROS binary for the current platform from GitHub
// Releases, verifies its SHA-256 checksum, and caches it in vendor/.
//
// Runs on `npm install` (postinstall). If it fails — offline CI,
// blocked network — installation still succeeds; the bin shim retries
// the download lazily on first run.

"use strict";

const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const platform = require("./platform");
const pkg = require("../package.json");

/** Fetches a URL (following redirects) into a Buffer. */
async function fetchBuffer(url) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) {
    throw new Error(`download failed: ${res.status} ${res.statusText} for ${url}`);
  }
  return Buffer.from(await res.arrayBuffer());
}

/** Extracts a .tar.gz (any platform) or .zip (Windows) archive. */
function extract(archivePath, destDir) {
  // Run from destDir with a relative archive path: GNU tar builds treat
  // a `C:` drive prefix as a remote-host name and fail.
  const relative = path.relative(destDir, archivePath);

  if (archivePath.endsWith(".zip")) {
    // A PATH `tar` may be GNU tar (Git Bash), which cannot read zip.
    // Prefer the Windows-bundled bsdtar; fall back to Expand-Archive.
    const sysTar = path.join(
      process.env.SystemRoot || "C:\\Windows",
      "System32",
      "tar.exe"
    );
    if (fs.existsSync(sysTar)) {
      const result = spawnSync(sysTar, ["-xf", relative], {
        cwd: destDir,
        stdio: "pipe",
        encoding: "utf8",
      });
      if (result.status === 0) {
        return;
      }
    }
    const ps = spawnSync(
      "powershell.exe",
      ["-NoProfile", "-Command", "Expand-Archive -Path $env:SB_ARCHIVE -DestinationPath $env:SB_DEST -Force"],
      {
        env: { ...process.env, SB_ARCHIVE: archivePath, SB_DEST: destDir },
        stdio: "pipe",
        encoding: "utf8",
      }
    );
    if (ps.status !== 0) {
      throw new Error(`zip extraction failed: ${ps.stderr || ps.error}`);
    }
    return;
  }

  const result = spawnSync("tar", ["-xzf", relative], {
    cwd: destDir,
    stdio: "pipe",
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`tar extraction failed: ${result.stderr || result.error}`);
  }
}

/**
 * Downloads, checksum-verifies, and installs the binaries into
 * vendor/. Returns the path of the `sb` binary.
 */
async function install() {
  const t = platform.target();
  if (!t) {
    throw new Error(
      `no prebuilt SNOWBROS binary for ${process.platform}-${process.arch}; ` +
        "install from source instead: cargo install snowbros"
    );
  }

  const url = platform.downloadUrl(pkg.version, t);
  const archive = await fetchBuffer(url);

  // Verify against the published .sha256 file. Format: "<hex>  <name>" or "<hex>".
  const expected = (await fetchBuffer(`${url}.sha256`))
    .toString("utf8")
    .trim()
    .split(/\s+/)[0]
    .toLowerCase();
  const actual = crypto.createHash("sha256").update(archive).digest("hex");
  if (actual !== expected) {
    throw new Error(`checksum mismatch for ${url}: expected ${expected}, got ${actual}`);
  }

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "snowbros-"));
  const archivePath = path.join(tmp, platform.archiveName(t));
  fs.writeFileSync(archivePath, archive);

  const vendor = platform.vendorDir();
  fs.mkdirSync(vendor, { recursive: true });
  extract(archivePath, tmp);

  // Archives may contain a top-level directory; find the binaries.
  for (const name of ["sb", "snowbros"]) {
    const file = `${name}${t.exe}`;
    const found = findFile(tmp, file);
    if (!found) {
      throw new Error(`archive did not contain ${file}`);
    }
    const dest = path.join(vendor, file);
    fs.copyFileSync(found, dest);
    if (t.exe === "") {
      fs.chmodSync(dest, 0o755);
    }
  }

  fs.rmSync(tmp, { recursive: true, force: true });
  return platform.binaryPath(t);
}

/** Breadth-first search for a file name under a directory. */
function findFile(dir, name) {
  const queue = [dir];
  while (queue.length > 0) {
    const current = queue.shift();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isFile() && entry.name === name) {
        return full;
      }
      if (entry.isDirectory()) {
        queue.push(full);
      }
    }
  }
  return null;
}

module.exports = { install, findFile, extract };

// Postinstall entry point: never fail the npm install — the shim
// retries lazily and prints a real error if the download keeps failing.
if (require.main === module) {
  install()
    .then((bin) => console.log(`snowbros: installed ${bin}`))
    .catch((err) => {
      console.warn(`snowbros: deferred binary download (${err.message})`);
    });
}
