import { spawn } from "child_process";
import type { Logger } from "./logger";
import type { AtlasConfig } from "./config";
import { rustLogFilter } from "./config";
import { resolveExecutable, needsShell } from "./resolve";

export interface CliResult {
  code: number;
  stdout: string;
  stderr: string;
}

export class CliError extends Error {
  constructor(
    message: string,
    readonly kind: "spawn" | "timeout" | "exit",
  ) {
    super(message);
    this.name = "CliError";
  }
}

/**
 * Runs an Atlas subcommand (e.g. `["analyze", "--format", "json"]`) in `cwd`
 * and captures its output. Enforces a timeout and translates every failure
 * mode into a {@link CliError} — it never rejects with a raw spawn error, so
 * callers can report gracefully.
 */
export function runCli(
  config: AtlasConfig,
  cwd: string,
  args: string[],
  log: Logger,
  timeoutMs = 60_000,
): Promise<CliResult> {
  const res = resolveExecutable(config.path);
  const fullArgs = [...res.baseArgs, ...args];
  const rustLog = rustLogFilter(config.logLevel);
  // On Windows a `.cmd`/`.bat` shim (e.g. the `snowbros.cmd`/`sb.cmd` wrappers
  // npm installs globally, or the `npx.cmd` fallback) must be spawned through
  // the shell: Node rejects a direct spawn of a batch script with EINVAL since
  // the CVE-2024-27980 fix. Real executables (`sb.exe`) spawn directly. This
  // mirrors the language-server launch in client.ts so both paths agree.
  const shell = needsShell(res.command);
  log.debug(
    `run (${res.source}, shell=${shell}): ${res.command} ${fullArgs.join(" ")}`,
  );

  return new Promise((resolve, reject) => {
    let settled = false;
    const child = spawn(res.command, fullArgs, {
      cwd,
      shell,
      env: rustLog ? { ...process.env, RUST_LOG: rustLog } : process.env,
    });

    const timer = setTimeout(() => {
      if (settled) {
        return;
      }
      settled = true;
      child.kill();
      reject(new CliError(`command timed out after ${timeoutMs} ms`, "timeout"));
    }, timeoutMs);

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d: Buffer) => (stdout += d.toString()));
    child.stderr.on("data", (d: Buffer) => (stderr += d.toString()));

    child.on("error", (err) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      const hint =
        res.source === "npx"
          ? "Could not run Atlas via npx. Install it (npm i -g @snowbros/atlas) or set atlas.path."
          : `Could not launch '${res.command}'. Check atlas.path or your PATH.`;
      reject(new CliError(`${hint} (${err.message})`, "spawn"));
    });

    child.on("close", (code) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve({ code: code ?? 0, stdout, stderr });
    });
  });
}
