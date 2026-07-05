// Bin shim: resolves the cached SNOWBROS binary (downloading it on
// first run if postinstall was skipped) and execs it with the caller's
// arguments, stdio, and exit code. Overhead after the first run is one
// stat() plus process spawn.

"use strict";

const fs = require("fs");
const { spawnSync } = require("child_process");

const platform = require("./platform");

/** Runs the named binary (`sb` or `snowbros`) with the given argv. */
async function run(name, argv) {
  // Escape hatch for tests and for pointing at a locally built binary.
  let bin = process.env.SNOWBROS_BINARY_PATH;

  if (!bin) {
    const t = platform.target();
    if (!t) {
      console.error(
        `snowbros: no prebuilt binary for ${process.platform}-${process.arch}; ` +
          "install from source: cargo install snowbros"
      );
      return 1;
    }
    bin = platform.binaryPath(t, name);
    if (!fs.existsSync(bin)) {
      const { install } = require("./install");
      try {
        await install();
      } catch (err) {
        console.error(`snowbros: cannot obtain binary: ${err.message}`);
        return 1;
      }
    }
  }

  // Batch files cannot be spawned directly on modern Node; route them
  // through cmd.exe (matters when SNOWBROS_BINARY_PATH is a .cmd shim).
  let cmd = bin;
  let args = argv;
  if (process.platform === "win32" && /\.(cmd|bat)$/i.test(bin)) {
    cmd = process.env.comspec || "cmd.exe";
    args = ["/d", "/s", "/c", bin, ...argv];
  }
  const result = spawnSync(cmd, args, { stdio: "inherit" });
  if (result.error) {
    console.error(`snowbros: failed to launch ${bin}: ${result.error.message}`);
    return 1;
  }
  return result.status === null ? 1 : result.status;
}

module.exports = { run };
