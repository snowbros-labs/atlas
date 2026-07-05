import * as assert from "assert";
import * as vscode from "vscode";
import { resolveExecutable, findOnPath, ResolveDeps } from "../../src/resolve";
import { parseReport } from "../../src/commands";

const EXT_ID = "snowbros.snowbros-atlas";

function deps(over: Partial<ResolveDeps>): ResolveDeps {
  return {
    isFile: () => false,
    pathEnv: undefined,
    pathExt: undefined,
    platform: "linux",
    ...over,
  };
}

suite("resolveExecutable", () => {
  test("prefers a configured path that exists", () => {
    const r = resolveExecutable(
      "/opt/atlas/sb",
      deps({ isFile: (p) => p === "/opt/atlas/sb" }),
    );
    assert.strictEqual(r.source, "config");
    assert.strictEqual(r.command, "/opt/atlas/sb");
    assert.deepStrictEqual(r.baseArgs, []);
  });

  test("ignores a configured path that does not exist", () => {
    const r = resolveExecutable("/nope/sb", deps({ isFile: () => false }));
    assert.notStrictEqual(r.source, "config");
  });

  test("finds a binary on PATH (unix)", () => {
    const r = resolveExecutable(
      "",
      deps({
        pathEnv: "/usr/bin:/usr/local/bin",
        isFile: (p) => p === "/usr/local/bin/sb",
        platform: "linux",
      }),
    );
    assert.strictEqual(r.source, "path");
    assert.strictEqual(r.command, "/usr/local/bin/sb");
  });

  test("honors PATHEXT on windows", () => {
    const found = findOnPath(
      deps({
        pathEnv: "C:\\bin",
        pathExt: ".EXE;.CMD",
        isFile: (p) => p === "C:\\bin\\sb.EXE",
        platform: "win32",
      }),
    );
    assert.strictEqual(found, "C:\\bin\\sb.EXE");
  });

  test("falls back to npx when nothing is found", () => {
    const r = resolveExecutable("", deps({ platform: "linux" }));
    assert.strictEqual(r.source, "npx");
    assert.strictEqual(r.command, "npx");
    assert.deepStrictEqual(r.baseArgs, ["--yes", "snowbros"]);
  });

  test("uses npx.cmd on windows fallback", () => {
    const r = resolveExecutable("", deps({ platform: "win32" }));
    assert.strictEqual(r.command, "npx.cmd");
  });
});

suite("parseReport", () => {
  test("extracts overall, total, and categories", () => {
    const json = JSON.stringify({
      summary: { total: 3 },
      scorecard: {
        overall: 87,
        categories: { security: { score: 100 }, architecture: { score: 74 } },
      },
    });
    const r = parseReport(json);
    assert.strictEqual(r.overall, 87);
    assert.strictEqual(r.total, 3);
    assert.strictEqual(r.categories.length, 2);
  });

  test("defaults gracefully on a sparse report", () => {
    const r = parseReport("{}");
    assert.strictEqual(r.overall, 0);
    assert.strictEqual(r.total, 0);
    assert.deepStrictEqual(r.categories, []);
  });
});

suite("extension", () => {
  test("is present and activates", async () => {
    const ext = vscode.extensions.getExtension(EXT_ID);
    assert.ok(ext, "extension should be installed");
    await ext!.activate();
    assert.strictEqual(ext!.isActive, true);
  });

  test("registers all atlas commands", async () => {
    const ext = vscode.extensions.getExtension(EXT_ID);
    await ext!.activate();
    const commands = await vscode.commands.getCommands(true);
    for (const id of [
      "atlas.analyzeWorkspace",
      "atlas.restart",
      "atlas.explainRule",
      "atlas.openReport",
      "atlas.showHealth",
      "atlas.clearCache",
    ]) {
      assert.ok(commands.includes(id), `missing command ${id}`);
    }
  });

  test("exposes typed configuration with defaults", () => {
    const c = vscode.workspace.getConfiguration("atlas");
    assert.strictEqual(c.get("enable"), true);
    assert.strictEqual(c.get("autoAnalyze"), true);
    assert.strictEqual(c.get("useCache"), true);
    assert.strictEqual(c.get("format"), "html");
    assert.strictEqual(c.get("logLevel"), "info");
    assert.strictEqual(c.get("enableStatusBar"), true);
  });
});
