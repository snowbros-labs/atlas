import * as fs from "fs";

/** How the Atlas executable was located. */
export type ResolveSource = "config" | "path" | "npx";

/**
 * A resolved way to invoke Atlas. `command` plus `baseArgs` form the prefix;
 * callers append the subcommand (e.g. `lsp`, `analyze`).
 */
export interface Resolution {
  command: string;
  baseArgs: string[];
  source: ResolveSource;
}

/** Injectable filesystem/env hooks so resolution is unit-testable. */
export interface ResolveDeps {
  isFile: (p: string) => boolean;
  pathEnv: string | undefined;
  pathExt: string | undefined;
  platform: NodeJS.Platform;
}

function defaultDeps(): ResolveDeps {
  return {
    isFile: (p) => {
      try {
        return fs.statSync(p).isFile();
      } catch {
        return false;
      }
    },
    pathEnv: process.env.PATH,
    pathExt: process.env.PATHEXT,
    platform: process.platform,
  };
}

/** Candidate binary names, in priority order. */
const BINARY_NAMES = ["sb", "snowbros"];

/**
 * Searches PATH for one of the Atlas binaries, honoring Windows PATHEXT.
 * Returns the absolute path, or undefined if not found.
 */
export function findOnPath(deps: ResolveDeps): string | undefined {
  if (!deps.pathEnv) {
    return undefined;
  }
  // Derive path semantics from the target platform, not the host, so the
  // function is correct in production and deterministic under test.
  const isWin = deps.platform === "win32";
  const delimiter = isWin ? ";" : ":";
  const sep = isWin ? "\\" : "/";
  const dirs = deps.pathEnv.split(delimiter).filter(Boolean);
  const exts = isWin
    ? (deps.pathExt ?? ".EXE;.CMD;.BAT").split(";").filter(Boolean)
    : [""];
  for (const name of BINARY_NAMES) {
    for (const dir of dirs) {
      for (const ext of exts) {
        const trimmed = dir.endsWith(sep) ? dir.slice(0, -sep.length) : dir;
        const candidate = `${trimmed}${sep}${name}${ext}`;
        if (deps.isFile(candidate)) {
          return candidate;
        }
      }
    }
  }
  return undefined;
}

/**
 * Resolves how to run Atlas:
 *   1. an explicit `atlas.path` that points at a real file,
 *   2. an `sb`/`snowbros` binary found on PATH,
 *   3. otherwise `npx --yes snowbros` as a zero-install fallback.
 *
 * Pure and deterministic given its deps — see the unit tests.
 */
export function resolveExecutable(
  configPath: string,
  deps: ResolveDeps = defaultDeps(),
): Resolution {
  const configured = configPath.trim();
  if (configured.length > 0 && deps.isFile(configured)) {
    return { command: configured, baseArgs: [], source: "config" };
  }

  const onPath = findOnPath(deps);
  if (onPath) {
    return { command: onPath, baseArgs: [], source: "path" };
  }

  const npx = deps.platform === "win32" ? "npx.cmd" : "npx";
  return { command: npx, baseArgs: ["--yes", "snowbros"], source: "npx" };
}

/**
 * Whether a command must be spawned through a shell. Node's fix for
 * CVE-2024-27980 rejects a direct `child_process.spawn` of a Windows batch
 * script (`.cmd`/`.bat`, e.g. `npx.cmd`) with `EINVAL`; running it via the
 * shell is the supported workaround. Only Windows batch scripts need this —
 * real executables (`sb.exe`) and every POSIX command spawn directly.
 */
export function needsShell(
  command: string,
  platform: NodeJS.Platform = process.platform,
): boolean {
  return platform === "win32" && /\.(cmd|bat)$/i.test(command);
}
