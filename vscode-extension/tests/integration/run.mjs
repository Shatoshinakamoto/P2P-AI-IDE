// run.mjs -- integration test runner: downloads VS Code (stable, from Microsoft's update server, checked by
// @vscode/test-electron), starts it with this extension in development mode and a throwaway profile, and runs
// suite.cjs inside its extension host. Usage: npm run build && node tests/integration/run.mjs [vscode-version]
// Security: --disable-extensions and a fresh temp user-data-dir, so no other extension or setting is involved;
// P2P_AI_EXT_TEST=1 turns on the extension's test hooks only inside this test-mode VS Code.
import { runTests } from "@vscode/test-electron";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const profile = mkdtempSync(join(tmpdir(), "p2pai-vscode-"));
try {
  await runTests({
    version: process.argv[2] ?? "stable",
    extensionDevelopmentPath: resolve(here, "../.."),
    extensionTestsPath: join(here, "suite.cjs"),
    extensionTestsEnv: { P2P_AI_EXT_TEST: "1" },
    launchArgs: ["--disable-extensions", "--user-data-dir", join(profile, "user"), "--extensions-dir", join(profile, "ext"), "--disable-workspace-trust"],
  });
  console.log("integration: pass");
} catch (e) {
  console.error("integration: fail", e?.message ?? e);
  process.exitCode = 1;
} finally {
  rmSync(profile, { recursive: true, force: true });
}
