import * as vscode from "vscode";
import { Logger } from "./logger";
import { StatusBar } from "./statusBar";
import { AtlasClient, ClientStatus } from "./client";
import { Commands } from "./commands";
import { readConfig } from "./config";

let logger: Logger | undefined;
let statusBar: StatusBar | undefined;
let client: AtlasClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const config = readConfig();
  logger = new Logger("Snowbros Atlas", config.logLevel);
  logger.info("activating Snowbros Atlas extension");

  statusBar = new StatusBar(config.enableStatusBar);
  const bar = statusBar;

  client = new AtlasClient(logger, (status: ClientStatus, detail?: string) => {
    switch (status) {
      case "starting":
        bar.set("running", "Starting language server…");
        break;
      case "ready":
        bar.set("ready");
        break;
      case "stopped":
        bar.set("ready");
        break;
      case "error":
        bar.set("error", detail);
        break;
    }
  });

  new Commands(client, statusBar, logger).register(context);

  // React to relevant setting changes without a reload.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (e) => {
      if (!e.affectsConfiguration("atlas")) {
        return;
      }
      const next = readConfig();
      logger?.setLevel(next.logLevel);
      statusBar?.setEnabled(next.enableStatusBar);
      if (
        e.affectsConfiguration("atlas.path") ||
        e.affectsConfiguration("atlas.logLevel")
      ) {
        if (client?.isRunning()) {
          logger?.info("configuration changed — restarting language server");
          await client.restart(next);
        }
      }
      if (e.affectsConfiguration("atlas.enable")) {
        await applyEnablement(next.enable);
      }
    }),
  );

  // Under the extension test harness there is no real workspace or Atlas
  // binary, so skip auto-starting the language server (it would try to resolve
  // and spawn one, hanging activation). Commands and config stay registered.
  if (context.extensionMode === vscode.ExtensionMode.Test) {
    logger.info("test mode — language server auto-start disabled");
    statusBar.set("ready");
  } else {
    await applyEnablement(config.enable);
  }
}

/** Starts or stops the server to match the master enable + autoAnalyze flags. */
async function applyEnablement(enabled: boolean): Promise<void> {
  const config = readConfig();
  if (!enabled) {
    statusBar?.set("disabled");
    await client?.stop();
    return;
  }
  if (config.autoAnalyze) {
    await client?.start(config);
  } else {
    logger?.info("autoAnalyze off — run 'Atlas: Analyze Workspace' to start");
    statusBar?.set("ready");
  }
}

export async function deactivate(): Promise<void> {
  await client?.dispose();
  client = undefined;
  statusBar?.dispose();
  statusBar = undefined;
  logger?.dispose();
  logger = undefined;
}
