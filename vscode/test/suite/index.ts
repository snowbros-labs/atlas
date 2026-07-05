import * as path from "path";
import Mocha from "mocha";
import { glob } from "glob";

export async function run(): Promise<void> {
  // The test files use Mocha's TDD interface (suite/test), so the runner must
  // load the matching UI — otherwise `suite` is undefined at file load time.
  const mocha = new Mocha({ ui: "tdd", color: true, timeout: 20_000 });
  const testsRoot = path.resolve(__dirname, ".");

  const files = await glob("**/*.test.js", { cwd: testsRoot });
  for (const f of files) {
    mocha.addFile(path.resolve(testsRoot, f));
  }

  await new Promise<void>((resolve, reject) => {
    mocha.run((failures) => {
      if (failures > 0) {
        reject(new Error(`${failures} test(s) failed.`));
      } else {
        resolve();
      }
    });
  });
}
