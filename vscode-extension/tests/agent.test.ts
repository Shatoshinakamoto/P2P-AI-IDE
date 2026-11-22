// agent.test.ts -- the extension's agent tools (side track X2): the shared cases in core/tests/vectors/agent_tools.json
// (the IDE's Rust agent runs the same file in core/src/agent_vectors.rs), plus workspace and turn tests with hostile
// input. Security: path escapes and links (CWE-22, CWE-59), the secrets deny list (CWE-200), strict arguments
// (CWE-20), lost-update checks (CWE-367), budgets (CWE-400) and that nothing is written without an accept (OWASP LLM06).
import { test } from "node:test";
import assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { Workspace, WsError, checkRel } from "../src/agent/workspace.ts";
import { Pending, SYSTEM_PROMPT, denied, lineDelta, normPath, runTool, runTurn, toolDefs, type StepFn } from "../src/agent/tools.ts";
import { NetError, addPieces, type ToolCall } from "../src/gateway/checks.ts";

const v = JSON.parse(fs.readFileSync(new URL("../../core/tests/vectors/agent_tools.json", import.meta.url), "utf8"));

/** A fresh temp folder with `f.txt` and README.md; removed by the caller. */
function tmp(text = ""): { ws: Workspace; dir: string } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "p2pai-agent-"));
  fs.writeFileSync(path.join(dir, "f.txt"), text);
  fs.writeFileSync(path.join(dir, "README.md"), "hi\n");
  return { ws: Workspace.open(dir), dir };
}
const rm = (dir: string): void => fs.rmSync(dir, { recursive: true, force: true });
const call = (name: string, a: unknown): ToolCall => ({ id: "c1", name, arguments: typeof a === "string" ? a : JSON.stringify(a) });

test("shared: tool definitions and system prompt", () => {
  assert.equal(SYSTEM_PROMPT, v.system_prompt);
  assert.deepEqual(toolDefs(), v.tool_defs);
});

test("shared: path rules", () => {
  for (const c of v.rel_paths) {
    let ok = true;
    try {
      checkRel(c.path);
    } catch {
      ok = false;
    }
    assert.equal(ok, c.ok, c.why);
  }
});

test("shared: deny list and path normalising", () => {
  for (const c of v.denied) assert.equal(denied(c.path), c.denied, c.why);
  for (const c of v.norm_paths) assert.equal(normPath(c.raw), c.want, c.why);
});

test("shared: edits, line ranges and line counts", () => {
  for (const c of v.edits) {
    const { ws, dir } = tmp(c.text);
    const p = new Pending();
    const r = runTool(ws, p, call("edit_file", { path: "f.txt", old_text: c.old, new_text: c.new }));
    if ("want" in c) {
      assert.ok(r.ok, `${c.why}: ${r.result}`);
      assert.equal(p.files.get("f.txt")?.after, c.want, c.why);
    } else assert.equal(r.result, `error: ${c.error}`, c.why);
    rm(dir);
  }
  for (const c of v.ranges) {
    const { ws, dir } = tmp(c.text);
    const a: Record<string, unknown> = { path: "f.txt" };
    if ("start" in c) a.start_line = c.start;
    if ("end" in c) a.end_line = c.end;
    const r = runTool(ws, new Pending(), call("read_file", a));
    if ("want" in c) assert.deepEqual([r.ok, r.result], [true, c.want], c.why);
    else assert.equal(r.result, `error: ${c.error}`, c.why);
    rm(dir);
  }
  for (const c of v.line_delta) assert.deepEqual(lineDelta(c.before, c.after), { added: c.added, removed: c.removed }, c.why);
});

test("shared: argument checks", () => {
  const { ws, dir } = tmp();
  for (const c of v.args) {
    const r = runTool(ws, new Pending(), { id: "c", name: c.tool, arguments: c.arguments });
    assert.equal(r.ok, false, c.why);
    assert.equal(r.result, `error: ${c.error}`, c.why);
  }
  rm(dir);
});

test("workspace refuses links, drive roots and escapes", () => {
  const { ws, dir } = tmp("x");
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), "p2pai-outside-"));
  fs.writeFileSync(path.join(outside, "secret.txt"), "top secret");
  // A junction on Windows needs no admin rights; a file link may (EPERM), so that part is skipped there.
  fs.symlinkSync(outside, path.join(dir, "out"), process.platform === "win32" ? "junction" : "dir");
  let fileLink = true;
  try {
    fs.symlinkSync(path.join(outside, "secret.txt"), path.join(dir, "link.txt"));
  } catch {
    fileLink = false;
  }
  const kind = (f: () => unknown): string => {
    try {
      f();
      return "ok";
    } catch (e) {
      return e instanceof WsError ? e.kind : String(e);
    }
  };
  assert.equal(kind(() => ws.readText("out/secret.txt")), "outside", "a folder link is never followed");
  if (fileLink) {
    assert.equal(kind(() => ws.readText("link.txt")), "outside", "a file link is never followed");
    assert.equal(kind(() => ws.writeText("link.txt", "x", undefined)), "outside", "never written through a link");
  }
  assert.equal(kind(() => ws.writeText("out/new.txt", "x", undefined)), "outside", "never written into a linked folder");
  assert.equal(kind(() => ws.listDir("out")), "outside", "a linked folder is not listed");
  assert.equal(kind(() => ws.readText("../" + path.basename(outside) + "/secret.txt")), "bad_path");
  assert.equal(fs.readFileSync(path.join(outside, "secret.txt"), "utf8"), "top secret");
  const listed = ws.listDir("").entries.map((e) => [e.name, e.kind]);
  assert.deepEqual(listed.find((e) => e[0] === "out"), ["out", "symlink"], "links are shown as links");
  assert.equal(kind(() => Workspace.open(path.parse(process.cwd()).root)), "bad_root", "a whole drive is refused");
  assert.equal(kind(() => Workspace.open("relative/path")), "bad_root");
  const r = runTool(ws, new Pending(), call("search", { query: "top secret" }));
  assert.equal(r.result, "no matches", "search does not follow links");
  rm(dir);
  rm(outside);
});

test("workspace caps, binary files and the lost-update check", () => {
  const { ws, dir } = tmp("v1");
  fs.writeFileSync(path.join(dir, "big.bin"), Buffer.alloc((5 << 20) + 1));
  fs.writeFileSync(path.join(dir, "bin.dat"), Buffer.from([0xff, 0xfe, 0x00]));
  assert.throws(() => ws.readText("big.bin"), (e: unknown) => e instanceof WsError && e.kind === "too_large");
  assert.throws(() => ws.readText("bin.dat"), (e: unknown) => e instanceof WsError && e.kind === "not_text");
  const v1 = ws.readText("f.txt").version;
  fs.writeFileSync(path.join(dir, "f.txt"), "changed outside");
  assert.throws(() => ws.writeText("f.txt", "mine", v1), (e: unknown) => e instanceof WsError && e.kind === "conflict");
  assert.equal(fs.readFileSync(path.join(dir, "f.txt"), "utf8"), "changed outside", "the outside change is kept");
  assert.throws(() => ws.writeText("README.md", "x", undefined), (e: unknown) => e instanceof WsError && e.kind === "conflict", "a new-file write never replaces an existing file");
  assert.throws(() => ws.search(""), (e: unknown) => e instanceof WsError && e.kind === "bad_query");
  assert.deepEqual(fs.readdirSync(dir).filter((n) => n.endsWith(".tmp")), [], "no temp files left behind");
  rm(dir);
});

test("edits stay pending until accepted; secrets stay hidden", () => {
  const { ws, dir } = tmp("def add(a, b):\n    return a - b\n");
  fs.writeFileSync(path.join(dir, ".env"), "API_KEY=secret\n");
  fs.mkdirSync(path.join(dir, "src"));
  const p = new Pending();
  assert.ok(runTool(ws, p, call("edit_file", { path: "f.txt", old_text: "a - b", new_text: "a + b" })).ok);
  assert.ok(runTool(ws, p, call("edit_file", { path: "./f.txt", old_text: "def add", new_text: "def plus" })).ok, "edits build on each other");
  assert.ok(runTool(ws, p, call("create_file", { path: "src/new.py", content: "x = 1\n" })).ok);
  assert.equal(runTool(ws, p, call("create_file", { path: "f.txt", content: "" })).result, "error: that file exists; use edit_file");
  assert.equal(runTool(ws, p, call("create_file", { path: "nofolder/x.py", content: "" })).ok, false, "the folder must exist");
  assert.equal(fs.readFileSync(path.join(dir, "f.txt"), "utf8"), "def add(a, b):\n    return a - b\n", "nothing written yet");
  assert.ok(!fs.existsSync(path.join(dir, "src/new.py")));
  assert.match(runTool(ws, p, call("read_file", { path: "f.txt" })).result, /def plus\(a, b\):\n    return a \+ b/, "reads see pending text");
  assert.ok(!runTool(ws, p, call("list_dir", { path: "" })).result.includes(".env"), ".env is not listed");
  assert.equal(runTool(ws, p, call("search", { query: "API_KEY" })).result, "no matches", ".env is not searched");
  assert.deepEqual(p.views(), [
    { rel: "f.txt", created: false, added: 2, removed: 2 },
    { rel: "src/new.py", created: true, added: 1, removed: 0 },
  ]);
  p.accept(ws, "f.txt");
  assert.equal(fs.readFileSync(path.join(dir, "f.txt"), "utf8"), "def plus(a, b):\n    return a + b\n");
  fs.writeFileSync(path.join(dir, "src/new.py"), "someone else\n");
  assert.throws(() => p.accept(ws, "src/new.py"), (e: unknown) => e instanceof WsError && e.kind === "conflict", "a file that appeared meanwhile is not replaced");
  assert.ok(p.reject("src/new.py"));
  assert.equal(p.files.size, 0);
  rm(dir);
});

test("a turn runs tools, keeps budgets and stops", async () => {
  const { ws, dir } = tmp("hello\n");
  const seen: unknown[][] = [];
  let n = 0;
  const step: StepFn = async (msgs, onDelta) => {
    seen.push(structuredClone(msgs));
    n++;
    if (n === 1) return { end: { finishReason: "tool_calls", chars: 0 }, calls: [call("read_file", { path: "f.txt" }), { id: "", name: "nope", arguments: "{}" }] };
    onDelta("done");
    return { end: { finishReason: "stop", chars: 4 }, calls: [] };
  };
  const events: unknown[] = [];
  const end = await runTurn(ws, new Pending(), [{ role: "user", content: "read f" }], step, (e) => events.push(e));
  assert.deepEqual([end.steps, end.toolCalls, end.text, end.finishReason], [2, 2, "done", "stop"]);
  const second = seen[1] as Array<Record<string, unknown>>;
  assert.equal(second[0].role, "system");
  assert.deepEqual(second.slice(-2).map((m) => [m.role, m.tool_call_id, m.content]), [
    ["tool", "c1", "hello\n"],
    ["tool", "call_1_1", "error: unknown tool; use list_dir, read_file, search, edit_file or create_file"],
  ]);
  assert.deepEqual(events.filter((e) => (e as { kind: string }).kind === "tool"), [
    { kind: "tool", name: "read_file", target: "f.txt", ok: true },
    { kind: "tool", name: "nope", target: "", ok: false },
  ]);
  // A model that never stops calling tools ends at 12 steps.
  const loop: StepFn = async () => ({ end: { finishReason: "tool_calls", chars: 0 }, calls: [call("list_dir", { path: "" })] });
  assert.equal((await runTurn(ws, new Pending(), [{ role: "user", content: "x" }], loop, () => {})).steps, 12);
  // Stop.
  const ac = new AbortController();
  const stopper: StepFn = async () => {
    ac.abort();
    return { end: { finishReason: "tool_calls", chars: 0 }, calls: [call("list_dir", { path: "" })] };
  };
  await assert.rejects(runTurn(ws, new Pending(), [{ role: "user", content: "x" }], stopper, () => {}, ac.signal), (e: unknown) => e instanceof NetError && e.kind === "stopped");
  rm(dir);
});

test("a turn caps tool results at 1 MiB in total", async () => {
  const { ws, dir } = tmp("a".repeat(200 << 10));
  let total = 0;
  const step: StepFn = async (msgs) => {
    const last = msgs[msgs.length - 1] as { role: string; content: string };
    if (last.role === "tool") total += Buffer.byteLength(last.content);
    return { end: { finishReason: "tool_calls", chars: 0 }, calls: [call("read_file", { path: "f.txt" })] };
  };
  await runTurn(ws, new Pending(), [{ role: "user", content: "x" }], step, () => {});
  assert.ok(total <= (1 << 20) + 200, `tool results ${total} stay under the turn budget`);
  rm(dir);
});

test("streamed tool calls are assembled by index with caps", () => {
  const calls: ToolCall[] = [];
  addPieces(calls, [{ index: 0, id: "call_1", name: "read_file", args: '{"pa' }]);
  addPieces(calls, [{ index: 0, id: "", name: "", args: 'th":"a.py"}' }, { index: 1, id: "c2", name: "search", args: '{"query":"x"}' }]);
  assert.deepEqual(calls, [
    { id: "call_1", name: "read_file", arguments: '{"path":"a.py"}' },
    { id: "c2", name: "search", arguments: '{"query":"x"}' },
  ]);
  assert.throws(() => addPieces([], [{ index: 8, id: "", name: "x", args: "" }]), (e: unknown) => e instanceof NetError && e.kind === "too_large");
  assert.throws(() => addPieces([], [{ index: 0, id: "", name: "x", args: "a".repeat((1 << 20) + 1) }]), (e: unknown) => e instanceof NetError && e.kind === "too_large");
});
