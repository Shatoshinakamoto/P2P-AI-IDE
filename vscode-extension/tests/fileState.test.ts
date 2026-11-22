// fileState.test.ts -- the extension's own state file (X3): one-time move from globalState, fresh reads, atomic
// writes, and broken, oversized or hostile files read as empty (CWE-20, CWE-400, CWE-1321). No vscode needed: the
// context is a small fake with a temp global storage folder.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { FileState } from "../src/fileState.ts";

function fakeCtx(old: Record<string, unknown>): { ctx: never; dir: string } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "p2pai-state-"));
  const ctx = {
    globalStorageUri: { fsPath: path.join(dir, "storage") },
    globalState: { keys: () => Object.keys(old), get: (k: string) => old[k] },
  };
  return { ctx: ctx as never, dir };
}

test("moves the old globalState values once, then reads and writes the file", async () => {
  const { ctx, dir } = fakeCtx({ "p2pAi.gateways": [{ id: "x" }], "p2pAi.model.abc": "m", "other.ext": 1 });
  const s = new FileState(ctx, ["p2pAi.gateways", "p2pAi.model."]);
  assert.deepEqual(s.get("p2pAi.gateways"), [{ id: "x" }]);
  assert.equal(s.get("p2pAi.model.abc"), "m");
  assert.equal(s.get("other.ext"), undefined, "only the listed keys move");
  await s.update("p2pAi.tools", false);
  await s.update("p2pAi.gateways", undefined);
  const again = new FileState(ctx, ["p2pAi.gateways"]);
  assert.equal(again.get("p2pAi.gateways"), undefined, "no second move once the file exists");
  assert.equal(again.get("p2pAi.tools"), false);
  const file = path.join(dir, "storage", "state.json");
  if (process.platform !== "win32") assert.equal(fs.statSync(file).mode & 0o777, 0o600, "owner-only file");
  assert.deepEqual(fs.readdirSync(path.join(dir, "storage")), ["state.json"], "no temp files left");
  fs.rmSync(dir, { recursive: true, force: true });
});

test("broken, oversized or hostile files read as empty", async () => {
  const { ctx, dir } = fakeCtx({});
  const s = new FileState(ctx);
  const file = path.join(dir, "storage", "state.json");
  fs.writeFileSync(file, "{not json");
  assert.equal(s.get("a"), undefined);
  fs.writeFileSync(file, "[1,2]");
  assert.equal(s.get("0"), undefined, "an array is not a state object");
  fs.writeFileSync(file, JSON.stringify({ big: "x".repeat(1 << 20) }));
  assert.equal(s.get("big"), undefined, "over 1 MiB is ignored");
  fs.writeFileSync(file, '{"__proto__": {"polluted": true}, "a": 1}');
  assert.equal(s.get("a"), 1);
  await s.update("b", 2);
  assert.equal(({} as Record<string, unknown>).polluted, undefined, "no prototype pollution");
  fs.rmSync(dir, { recursive: true, force: true });
});
