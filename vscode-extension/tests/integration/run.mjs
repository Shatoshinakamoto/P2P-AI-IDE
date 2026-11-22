// run.mjs -- integration test runner: downloads VS Code (stable, from Microsoft's update server, checked by
// @vscode/test-electron), starts it with this extension in development mode and a throwaway profile, and runs
// suite.cjs inside its extension host. Usage: npm run build && node tests/integration/run.mjs [vscode-version]
// Security: --disable-extensions and a fresh temp user-data-dir, so no other extension or setting is involved;
// P2P_AI_EXT_TEST=1 turns on the extension's test hooks only inside this test-mode VS Code. The VS Code opens a
// throwaway project folder (deleted afterwards) for the agent-tool test; --disable-workspace-trust makes it trusted.
import { runTests } from "@vscode/test-electron";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const profile = mkdtempSync(join(tmpdir(), "p2pai-vscode-"));
// A small project folder for the agent-tool test (X2): one file to edit and a secret that must stay hidden.
const project = join(profile, "project");
mkdirSync(project);
writeFileSync(join(project, "hello.py"), "def add(a, b):\n    return a - b\n");
writeFileSync(join(project, ".env"), "API_KEY=do-not-send\n");
try {
  await runTests({
    version: process.argv[2] ?? "stable",
    extensionDevelopmentPath: resolve(here, "../.."),
    extensionTestsPath: join(here, "suite.cjs"),
    extensionTestsEnv: { P2P_AI_EXT_TEST: "1", P2P_AI_TEST_PROJECT: project },
    launchArgs: [project, "--disable-extensions", "--user-data-dir", join(profile, "user"), "--extensions-dir", join(profile, "ext"), "--disable-workspace-trust"],
  });
  console.log("integration: pass");
} catch (e) {
  console.error("integration: fail", e?.message ?? e);
  process.exitCode = 1;
} finally {
  rmSync(profile, { recursive: true, force: true });
}
