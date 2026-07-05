import * as vscode from "vscode";
import * as os from "os";
import * as path from "path";
import * as fs from "fs/promises";
import type { Logger } from "./logger";
import type { AtlasClient } from "./client";
import type { StatusBar } from "./statusBar";
import { readConfig, AtlasConfig } from "./config";
import { runCli, CliError } from "./cli";

/** Wires up and implements every `atlas.*` command. */
export class Commands {
  constructor(
    private readonly client: AtlasClient,
    private readonly status: StatusBar,
    private readonly log: Logger,
  ) {}

  register(context: vscode.ExtensionContext): void {
    const add = (id: string, fn: () => Promise<void>) =>
      context.subscriptions.push(
        vscode.commands.registerCommand(id, () => this.guard(id, fn)),
      );

    add("atlas.analyzeWorkspace", () => this.analyzeWorkspace());
    add("atlas.restart", () => this.restart());
    add("atlas.explainRule", () => this.explainRule());
    add("atlas.openReport", () => this.openReport());
    add("atlas.showHealth", () => this.showHealth());
    add("atlas.clearCache", () => this.clearCache());
  }

  /** Runs a command body, converting any failure into a friendly message. */
  private async guard(id: string, fn: () => Promise<void>): Promise<void> {
    try {
      await fn();
    } catch (err) {
      const msg = err instanceof CliError ? err.message : errText(err);
      this.log.error(`${id} failed: ${msg}`);
      this.status.set("error", msg);
      void vscode.window.showErrorMessage(`Atlas: ${msg}`);
    }
  }

  private folder(): vscode.WorkspaceFolder {
    const active = vscode.window.activeTextEditor?.document.uri;
    const folder = active
      ? vscode.workspace.getWorkspaceFolder(active)
      : undefined;
    const chosen = folder ?? vscode.workspace.workspaceFolders?.[0];
    if (!chosen) {
      throw new Error("open a folder to analyze first.");
    }
    return chosen;
  }

  private async analyzeWorkspace(): Promise<void> {
    const config = readConfig();
    this.status.set("running", "Analyzing workspace…");
    await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Window, title: "Atlas: analyzing…" },
      async () => {
        // The server re-analyzes on start; restart forces a fresh full pass
        // and republishes all diagnostics.
        if (this.client.isRunning()) {
          await this.client.restart(config);
        } else {
          await this.client.start(config);
        }
      },
    );
    await this.showHealth();
  }

  private async restart(): Promise<void> {
    const config = readConfig();
    this.status.set("running", "Restarting language server…");
    await this.client.restart(config);
    this.status.set("ready");
    void vscode.window.showInformationMessage("Atlas: language server restarted.");
  }

  private async explainRule(): Promise<void> {
    const config = readConfig();
    const seed = this.ruleAtCursor();
    const ruleId = await vscode.window.showInputBox({
      title: "Atlas: Explain Rule",
      prompt: "Rule id to explain (e.g. security/no-eval)",
      value: seed ?? "",
      ignoreFocusOut: true,
    });
    if (!ruleId) {
      return;
    }
    const { stdout, stderr, code } = await runCli(
      config,
      this.folder().uri.fsPath,
      ["explain", ruleId.trim()],
      this.log,
    );
    if (code !== 0) {
      throw new Error(stderr.trim() || `no explanation for '${ruleId}'.`);
    }
    const doc = await vscode.workspace.openTextDocument({
      content: stdout,
      language: "markdown",
    });
    await vscode.window.showTextDocument(doc, { preview: true });
  }

  private async openReport(): Promise<void> {
    const config = readConfig();
    const folder = this.folder();
    this.status.set("running", "Generating report…");
    const args = ["analyze", "--format", config.format];
    if (!config.useCache) {
      args.push("--no-cache");
    }
    const { stdout, code, stderr } = await runCli(
      config,
      folder.uri.fsPath,
      args,
      this.log,
    );
    if (code !== 0 && stdout.length === 0) {
      throw new Error(stderr.trim() || "report generation failed.");
    }
    this.status.set("ready");
    const ext = extFor(config.format);
    const file = path.join(os.tmpdir(), `atlas-report-${Date.now()}.${ext}`);
    await fs.writeFile(file, stdout, "utf8");
    const uri = vscode.Uri.file(file);
    if (config.format === "html") {
      await vscode.env.openExternal(uri);
    } else {
      const doc = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(doc, { preview: true });
    }
  }

  private async showHealth(): Promise<void> {
    const config = readConfig();
    const folder = this.folder();
    const args = ["analyze", "--format", "json"];
    if (!config.useCache) {
      args.push("--no-cache");
    }
    const { stdout, code, stderr } = await runCli(
      config,
      folder.uri.fsPath,
      args,
      this.log,
    );
    if (stdout.length === 0) {
      throw new Error(stderr.trim() || `analysis failed (exit ${code}).`);
    }
    const report = parseReport(stdout);
    this.status.showHealth(report.overall);
    const cats = report.categories
      .map((c) => `${c.name} ${c.score}`)
      .join(" · ");
    void vscode.window.showInformationMessage(
      `Atlas health ${report.overall}/100 — ${report.total} finding(s). ${cats}`,
    );
  }

  private async clearCache(): Promise<void> {
    const folder = this.folder();
    const cache = vscode.Uri.joinPath(folder.uri, ".snowbros", "cache.json");
    try {
      await vscode.workspace.fs.delete(cache);
      this.log.info(`removed ${cache.fsPath}`);
    } catch {
      // Missing cache is fine — nothing to clear.
    }
    const pick = await vscode.window.showInformationMessage(
      "Atlas: cache cleared.",
      "Re-analyze",
    );
    if (pick === "Re-analyze") {
      await this.analyzeWorkspace();
    }
  }

  /** Rule id of a Snowbros diagnostic at the cursor, if any. */
  private ruleAtCursor(): string | undefined {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      return undefined;
    }
    const pos = editor.selection.active;
    const diags = vscode.languages.getDiagnostics(editor.document.uri);
    for (const d of diags) {
      if (d.source === "snowbros" && d.range.contains(pos)) {
        return typeof d.code === "string" ? d.code : String(d.code);
      }
    }
    return undefined;
  }
}

interface HealthReport {
  overall: number;
  total: number;
  categories: { name: string; score: number }[];
}

/** Parses the health score out of `sb analyze --format json`. */
export function parseReport(json: string): HealthReport {
  const data = JSON.parse(json) as {
    summary?: { total?: number };
    scorecard?: {
      overall?: number;
      categories?: Record<string, { score?: number }>;
    };
  };
  const overall = data.scorecard?.overall ?? 0;
  const total = data.summary?.total ?? 0;
  const categories = Object.entries(data.scorecard?.categories ?? {}).map(
    ([name, c]) => ({ name, score: c.score ?? 0 }),
  );
  return { overall, total, categories };
}

function extFor(format: AtlasConfig["format"]): string {
  switch (format) {
    case "html":
      return "html";
    case "json":
      return "json";
    case "markdown":
      return "md";
    case "sarif":
      return "sarif.json";
  }
}

function errText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
