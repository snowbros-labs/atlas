import * as vscode from "vscode";

/**
 * A thin wrapper over a VS Code output channel that respects the
 * `atlas.logLevel` setting. Never throws; logging must not break the
 * extension.
 */
export type LogLevel = "off" | "error" | "info" | "debug" | "trace";

const ORDER: Record<LogLevel, number> = {
  off: 0,
  error: 1,
  info: 2,
  debug: 3,
  trace: 4,
};

export class Logger {
  private readonly channel: vscode.OutputChannel;
  private level: LogLevel;

  constructor(name: string, level: LogLevel) {
    this.channel = vscode.window.createOutputChannel(name);
    this.level = level;
  }

  setLevel(level: LogLevel): void {
    this.level = level;
  }

  private write(level: LogLevel, message: string): void {
    if (ORDER[level] > ORDER[this.level]) {
      return;
    }
    const stamp = new Date().toISOString();
    this.channel.appendLine(`[${stamp}] [${level}] ${message}`);
  }

  error(message: string): void {
    this.write("error", message);
  }

  info(message: string): void {
    this.write("info", message);
  }

  debug(message: string): void {
    this.write("debug", message);
  }

  trace(message: string): void {
    this.write("trace", message);
  }

  show(): void {
    this.channel.show(true);
  }

  dispose(): void {
    this.channel.dispose();
  }
}
