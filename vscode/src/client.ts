import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  State,
  TransportKind,
} from "vscode-languageclient/node";
import type { Logger } from "./logger";
import type { AtlasConfig } from "./config";
import { rustLogFilter } from "./config";
import { resolveExecutable, ResolveSource } from "./resolve";

export type ClientStatus = "starting" | "ready" | "stopped" | "error";

/**
 * Wraps the LSP client that drives `sb lsp`. The Rust server is the single
 * source of truth for diagnostics; this class only manages its lifecycle and
 * surfaces state changes. All failures are caught so VS Code never crashes.
 */
export class AtlasClient {
  private client: LanguageClient | undefined;
  private starting = false;

  constructor(
    private readonly log: Logger,
    private readonly onStatus: (status: ClientStatus, detail?: string) => void,
  ) {}

  isRunning(): boolean {
    return this.client?.state === State.Running;
  }

  /** The source used to launch the server (config/path/npx), for reporting. */
  resolvedSource(config: AtlasConfig): ResolveSource {
    return resolveExecutable(config.path).source;
  }

  private buildServerOptions(config: AtlasConfig): ServerOptions {
    const res = resolveExecutable(config.path);
    const rustLog = rustLogFilter(config.logLevel);
    const options = {
      command: res.command,
      args: [...res.baseArgs, "lsp"],
      transport: TransportKind.stdio,
      options: {
        env: rustLog ? { ...process.env, RUST_LOG: rustLog } : process.env,
      },
    };
    this.log.info(`language server: ${res.command} (via ${res.source})`);
    return { run: options, debug: options };
  }

  async start(config: AtlasConfig): Promise<void> {
    if (this.client || this.starting) {
      return;
    }
    this.starting = true;
    this.onStatus("starting");
    try {
      const clientOptions: LanguageClientOptions = {
        documentSelector: [
          { scheme: "file", language: "javascript" },
          { scheme: "file", language: "javascriptreact" },
          { scheme: "file", language: "typescript" },
          { scheme: "file", language: "typescriptreact" },
        ],
        synchronize: {
          fileEvents: vscode.workspace.createFileSystemWatcher(
            "**/{snowbros.toml,tsconfig.json,package.json}",
          ),
        },
        // Cap automatic restarts so a persistently broken binary can't spin.
        connectionOptions: { maxRestartCount: 3 },
        outputChannelName: "Snowbros Atlas Language Server",
      };

      const client = new LanguageClient(
        "atlas",
        "Snowbros Atlas",
        this.buildServerOptions(config),
        clientOptions,
      );

      client.onDidChangeState((e) => {
        if (e.newState === State.Running) {
          this.onStatus("ready");
        } else if (e.newState === State.Stopped) {
          this.onStatus("stopped");
        }
      });

      await client.start();
      this.client = client;
      this.log.info("language server started");
    } catch (err) {
      this.onStatus("error", errText(err));
      this.log.error(`failed to start language server: ${errText(err)}`);
      this.client = undefined;
    } finally {
      this.starting = false;
    }
  }

  async stop(): Promise<void> {
    const client = this.client;
    this.client = undefined;
    if (!client) {
      return;
    }
    try {
      await client.stop();
      this.log.info("language server stopped");
    } catch (err) {
      this.log.error(`error stopping language server: ${errText(err)}`);
    }
  }

  async restart(config: AtlasConfig): Promise<void> {
    await this.stop();
    await this.start(config);
  }

  async dispose(): Promise<void> {
    await this.stop();
  }
}

function errText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
