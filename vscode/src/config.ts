import * as vscode from "vscode";
import type { LogLevel } from "./logger";

export type ReportFormat = "html" | "json" | "markdown" | "sarif";

/** Strongly-typed snapshot of the `atlas.*` settings. */
export interface AtlasConfig {
  enable: boolean;
  path: string;
  autoAnalyze: boolean;
  useCache: boolean;
  logLevel: LogLevel;
  format: ReportFormat;
  enableStatusBar: boolean;
}

/** Reads the current `atlas.*` configuration into a typed object. */
export function readConfig(): AtlasConfig {
  const c = vscode.workspace.getConfiguration("atlas");
  return {
    enable: c.get<boolean>("enable", true),
    path: c.get<string>("path", "").trim(),
    autoAnalyze: c.get<boolean>("autoAnalyze", true),
    useCache: c.get<boolean>("useCache", true),
    logLevel: c.get<LogLevel>("logLevel", "info"),
    format: c.get<ReportFormat>("format", "html"),
    enableStatusBar: c.get<boolean>("enableStatusBar", true),
  };
}

/** Maps the extension log level onto a `RUST_LOG` filter for the server. */
export function rustLogFilter(level: LogLevel): string | undefined {
  switch (level) {
    case "off":
    case "error":
      return "snowbros=error";
    case "info":
      return "snowbros=info";
    case "debug":
      return "snowbros=debug";
    case "trace":
      return "snowbros=trace";
    default:
      return undefined;
  }
}
