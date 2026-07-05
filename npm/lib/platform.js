// Maps the current Node platform/arch onto a SNOWBROS release artifact.
// Artifact names must match what cargo-dist produces (see
// dist-workspace.toml at the repository root).

"use strict";

const path = require("path");

/** Release targets keyed by `${process.platform}-${process.arch}`. */
const TARGETS = {
  "win32-x64": { triple: "x86_64-pc-windows-msvc", ext: ".zip", exe: ".exe" },
  "darwin-x64": { triple: "x86_64-apple-darwin", ext: ".tar.gz", exe: "" },
  "darwin-arm64": { triple: "aarch64-apple-darwin", ext: ".tar.gz", exe: "" },
  "linux-x64": { triple: "x86_64-unknown-linux-gnu", ext: ".tar.gz", exe: "" },
  "linux-arm64": { triple: "aarch64-unknown-linux-gnu", ext: ".tar.gz", exe: "" },
};

/**
 * Returns the target descriptor for a platform/arch pair, or null when
 * no prebuilt binary exists for it.
 */
function target(platform = process.platform, arch = process.arch) {
  return TARGETS[`${platform}-${arch}`] || null;
}

/** Archive file name for a target, e.g. `snowbros-atlas-x86_64-apple-darwin.tar.gz`. */
function archiveName(t) {
  return `snowbros-atlas-${t.triple}${t.ext}`;
}

/** Download URL for a version + target. */
function downloadUrl(version, t) {
  const repo = "https://github.com/snowbros-labs/atlas";
  return `${repo}/releases/download/v${version}/${archiveName(t)}`;
}

/** Directory the extracted binaries are cached in (inside the package). */
function vendorDir() {
  return path.join(__dirname, "..", "vendor");
}

/** Absolute path of the cached binary for a target. */
function binaryPath(t, name = "sb") {
  return path.join(vendorDir(), `${name}${t.exe}`);
}

module.exports = { TARGETS, target, archiveName, downloadUrl, vendorDir, binaryPath };
