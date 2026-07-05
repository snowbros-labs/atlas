import * as vscode from "vscode";

export type AtlasState = "ready" | "running" | "error" | "disabled";

/**
 * Owns the Atlas status bar item. Reflects server lifecycle:
 * Ready (idle, healthy), Running (analyzing / starting), or Error.
 */
export class StatusBar {
  private readonly item: vscode.StatusBarItem;
  private enabled: boolean;

  constructor(enabled: boolean) {
    this.item = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100,
    );
    this.item.command = "atlas.showHealth";
    this.enabled = enabled;
    this.set("ready");
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    if (!enabled) {
      this.item.hide();
    }
  }

  set(state: AtlasState, detail?: string): void {
    if (!this.enabled || state === "disabled") {
      this.item.hide();
      return;
    }
    switch (state) {
      case "ready":
        this.item.text = "$(telescope) Atlas";
        this.item.tooltip = "Snowbros Atlas — ready. Click for health score.";
        this.item.backgroundColor = undefined;
        break;
      case "running":
        this.item.text = "$(sync~spin) Atlas";
        this.item.tooltip = detail ?? "Snowbros Atlas — analyzing…";
        this.item.backgroundColor = undefined;
        break;
      case "error":
        this.item.text = "$(error) Atlas";
        this.item.tooltip = `Snowbros Atlas — error${detail ? `: ${detail}` : ""}. Click for details.`;
        this.item.backgroundColor = new vscode.ThemeColor(
          "statusBarItem.errorBackground",
        );
        break;
    }
    this.item.show();
  }

  /** Briefly shows the health score, then returns to ready. */
  showHealth(score: number): void {
    if (!this.enabled) {
      return;
    }
    const icon = score >= 90 ? "$(pass)" : score >= 70 ? "$(warning)" : "$(error)";
    this.item.text = `${icon} Atlas ${score}/100`;
    this.item.tooltip = `Snowbros Atlas — project health ${score}/100.`;
    this.item.backgroundColor = undefined;
    this.item.show();
  }

  dispose(): void {
    this.item.dispose();
  }
}
