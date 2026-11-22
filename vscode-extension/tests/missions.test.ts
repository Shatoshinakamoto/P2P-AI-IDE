// missions.test.ts -- the extension's mission logic (side track X3): the shared cases in core/tests/vectors/missions.json
// (the IDE runs them in core/src/projects_vectors.rs), push and pull against a fake project API, and the client's JSON,
// upload and download calls against a fake gateway on 127.0.0.1. Security: secrets kept home (CWE-200), digests
// checked (CWE-345), nothing written without review (OWASP LLM06), header-safe multipart (CWE-93), fixed error text
// (CWE-209), only /v1/ paths (CWE-918).
import { test } from "node:test";
import assert from "node:assert/strict";
import * as fs from "node:fs";
import * as http from "node:http";
import * as os from "node:os";
import * as path from "node:path";
import { createHash } from "node:crypto";
import { answerError, NetError } from "../src/gateway/checks.ts";
import { GatewayClient, encodePath, uploadName } from "../src/gateway/client.ts";
import { Pending } from "../src/agent/tools.ts";
import { Workspace } from "../src/agent/workspace.ts";
import {
  acceptance,
  approvalView,
  eligible,
  gatewayPathOk,
  idOk,
  missionView,
  parseRemoteFiles,
  planPull,
  planPush,
  projectView,
  proposalView,
  pull,
  push,
  switches,
  syncView,
  type ProjectApi,
} from "../src/missions/projects.ts";

const v = JSON.parse(fs.readFileSync(new URL("../../core/tests/vectors/missions.json", import.meta.url), "utf8"));
const sha = (b: string | Buffer): string => createHash("sha256").update(b).digest("hex");
/** Plain objects, so deepEqual ignores null prototypes. */
const plain = (x: unknown): unknown => JSON.parse(JSON.stringify(x ?? null));

test("shared: path grammar, ids, encoding and upload names", () => {
  for (const c of v.gateway_paths) assert.equal(gatewayPathOk(c.path), c.ok, c.why);
  for (const c of v.ids) assert.equal(idOk(c.id), c.ok, c.why);
  for (const c of v.encode_path) assert.equal(encodePath(c.rel), c.want, c.why);
  for (const c of v.upload_names) assert.equal(uploadName(c.rel) ?? null, c.want, c.why);
});

test("shared: gateway error codes", () => {
  for (const c of v.errors) {
    const e = answerError(c.status, c.body);
    assert.deepEqual([e.kind, e.message], [c.kind, c.message], c.why);
  }
});

test("shared: push and pull plans", () => {
  for (const c of v.eligible) {
    const r = eligible(c.files);
    assert.deepEqual(r.ok, c.ok, c.why);
    assert.deepEqual(r.skipped, c.skipped, c.why);
  }
  for (const c of v.plan_push) {
    const p = planPush(c.local, c.remote, c.skipped_local);
    assert.deepEqual([p.upload, p.delete, p.unchanged], [c.upload, c.delete, c.unchanged], c.why);
  }
  for (const c of v.plan_pull) {
    const p = planPull(c.local, c.remote);
    assert.deepEqual(p.fetch.map((f) => ({ path: f.path, sha256: f.sha256, local_version: f.localVersion ?? null })), c.fetch, c.why);
    assert.deepEqual([p.same, p.onlyLocal], [c.same, c.only_local], c.why);
    assert.deepEqual(p.skipped, c.skipped, c.why);
  }
  for (const c of v.remote_files) assert.deepEqual(parseRemoteFiles(c.answer), c.want, c.why);
});

test("shared: view shapes", () => {
  const views = v.views;
  const check = (name: string, f: (x: unknown) => unknown): void => {
    for (const c of views[name]) assert.deepEqual(plain(f(c.in)), c.want, `${name}: ${c.why}`);
  };
  check("mission", missionView);
  check("approval", approvalView);
  check("proposal", proposalView);
  check("sync", syncView);
  check("project", projectView);
  for (const c of views.switches) assert.deepEqual(switches(c.exp, c.flags), c.want, c.why);
});

test("shared: acceptance checks", () => {
  for (const c of v.acceptance) {
    const r = acceptance(c.checks);
    if ("want" in c) assert.deepEqual(r, c.want, c.why);
    else assert.deepEqual(r, { error: c.error }, c.why);
  }
});

/** An in-memory gateway project for push and pull. */
function fakeProject(files: Record<string, string>): ProjectApi & { files: Record<string, string>; commits: unknown[]; uploads: string[] } {
  let version = 1;
  const blobs = new Map<string, string>();
  const api = {
    files: { ...files },
    commits: [] as unknown[],
    uploads: [] as string[],
    async api(method: string, p: string, body?: unknown): Promise<unknown> {
      if (p === "/v1/projects/p1/workspace") return { version };
      if (p === "/v1/projects/p1/workspace/files") return { data: Object.entries(api.files).map(([k, t]) => ({ path: k, bytes: Buffer.byteLength(t), sha256: sha(t) })) };
      if (p === "/v1/projects/p1/workspace/commit") {
        const b = body as { base_version: number; changes: { path: string; file_id?: string; delete?: boolean }[] };
        if (b.base_version !== version) throw new NetError("gateway", "the gateway project changed meanwhile; try again");
        for (const ch of b.changes) {
          if (ch.delete) delete api.files[ch.path];
          else api.files[ch.path] = blobs.get(ch.file_id as string) as string;
        }
        api.commits.push(b);
        return { version: ++version };
      }
      if (p === "/v1/projects/p1/workspace/sync") return { state: "pending" };
      throw new NetError("not_offered");
    },
    async uploadFile(name: string, bytes: Uint8Array): Promise<unknown> {
      const id = `f${blobs.size}`;
      blobs.set(id, Buffer.from(bytes).toString("utf8"));
      api.uploads.push(name);
      return { id, state: "ready" };
    },
    async download(p: string): Promise<Buffer> {
      const m = /^\/v1\/projects\/p1\/workspace\/files\/(.+)\?download=1$/.exec(p);
      assert.ok(m, p);
      return Buffer.from(api.files[decodeURIComponent(m[1])] ?? "");
    },
  };
  return api;
}

function folder(files: Record<string, string>): { ws: Workspace; dir: string } {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "p2pai-mission-"));
  for (const [k, t] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, k)), { recursive: true });
    fs.writeFileSync(path.join(dir, k), t);
  }
  return { ws: Workspace.open(dir), dir };
}

test("push sends changes, keeps secrets home and commits with the base version", async () => {
  const { ws, dir } = folder({ "a.py": "new a\n", "b.py": "same\n", ".env": "KEY=x\n", "node_modules/x.js": "dep", "src/c.py": "c\n" });
  const gw = fakeProject({ "a.py": "old a\n", "b.py": "same\n", "gone.py": "bye\n" });
  const r = await push(gw, ws, "p1");
  assert.deepEqual([r.uploaded, r.deleted, r.unchanged, r.version, r.copy], [2, 1, 1, 2, "pending"]);
  assert.deepEqual(r.skipped, [{ rel: ".env", reason: "may hold secrets, kept on this computer" }]);
  assert.deepEqual(Object.keys(gw.files).sort(), ["a.py", "b.py", "src/c.py"], "node_modules and .env never sent; gone.py deleted");
  assert.equal(gw.files["a.py"], "new a\n");
  assert.equal((gw.commits[0] as { base_version: number }).base_version, 1);
  await assert.rejects(push(gw, ws, "../p1"), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
  fs.rmSync(dir, { recursive: true, force: true });
});

test("pull brings changes back for review only, checking digests", async () => {
  const { ws, dir } = folder({ "a.py": "mine\n", "keep.py": "k\n", "only_here.py": "x\n" });
  const gw = fakeProject({ "a.py": "theirs\n", "keep.py": "k\n", "new/b.py": "b\n", ".env": "EVIL=1\n" });
  fs.mkdirSync(path.join(dir, "new"));
  const pending = new Pending();
  const r = await pull(gw, ws, pending, "p1");
  assert.deepEqual([r.proposed, r.same, r.onlyLocal], [2, 1, 1]);
  assert.deepEqual(r.skipped, [{ rel: ".env", reason: "a name kept private; not brought back" }]);
  assert.equal(fs.readFileSync(path.join(dir, "a.py"), "utf8"), "mine\n", "nothing written by a pull");
  assert.ok(!fs.existsSync(path.join(dir, ".env")));
  assert.deepEqual(pending.views().map((c) => [c.rel, c.created]), [["a.py", false], ["new/b.py", true]]);
  pending.accept(ws, "a.py");
  assert.equal(fs.readFileSync(path.join(dir, "a.py"), "utf8"), "theirs\n");
  // A download that doesn't match its listed digest is refused.
  const bad = fakeProject({ "a.py": "x\n" });
  bad.download = async () => Buffer.from("tampered\n");
  const r2 = await pull(bad, ws, new Pending(), "p1");
  assert.deepEqual(r2.skipped, [{ rel: "a.py", reason: "changed on the gateway while downloading; pull again" }]);
  fs.rmSync(dir, { recursive: true, force: true });
});

test("client: JSON calls, multipart upload and download against a fake gateway", async () => {
  const KEY = "unit-key-0123456789";
  const seen: { method?: string; url?: string; type?: string; body: Buffer }[] = [];
  const srv = http.createServer(async (req, res) => {
    const parts: Buffer[] = [];
    for await (const c of req) parts.push(c as Buffer);
    seen.push({ method: req.method, url: req.url, type: req.headers["content-type"], body: Buffer.concat(parts) });
    if (req.headers.authorization !== `Bearer ${KEY}`) return res.writeHead(401).end();
    if (req.url === "/v1/projects") return res.end(JSON.stringify({ data: [{ id: "p1", name: "Shop" }] }));
    if (req.url === "/v1/files") return res.end(JSON.stringify({ id: "file_1", state: "ready" }));
    if (req.url === "/v1/projects/p1/workspace/commit") return res.writeHead(409).end(JSON.stringify({ error: { code: "version_conflict", message: "<b>internal detail</b>" } }));
    if (req.url?.startsWith("/v1/projects/p1/workspace/files/")) return res.end("file text");
    if (req.url === "/v1/redirect") return res.writeHead(302, { Location: "http://evil.example/" }).end();
    res.writeHead(404).end("{}");
  });
  await new Promise<void>((r) => srv.listen(0, "127.0.0.1", r));
  const c = new GatewayClient({ url: `http://127.0.0.1:${(srv.address() as { port: number }).port}`, key: KEY });
  try {
    assert.deepEqual(await c.api("GET", "/v1/projects"), { data: [{ id: "p1", name: "Shop" }] });
    await assert.rejects(c.api("POST", "/v1/projects/p1/workspace/commit", { base_version: 1 }), (e: unknown) => e instanceof NetError && e.message === "the gateway project changed meanwhile; try again" && !e.message.includes("internal"));
    await assert.rejects(c.api("GET", "/v1/redirect"), (e: unknown) => e instanceof NetError && e.kind === "status", "redirects are not followed");
    for (const bad of ["/v2/x", "/v1/../admin", "/v1/a#b", "http://evil/v1/x", "/v1/a\nb"]) {
      await assert.rejects(c.api("GET", bad), (e: unknown) => e instanceof NetError && e.kind === "bad_request", bad);
    }
    assert.deepEqual(await c.uploadFile("main.py", Buffer.from("print(1)\n")), { id: "file_1", state: "ready" });
    const up = seen.find((s) => s.url === "/v1/files") as { type: string; body: Buffer };
    const boundary = /boundary=(p2pide[0-9a-f]{32})$/.exec(up.type)?.[1];
    assert.ok(boundary, "a random boundary");
    assert.match(up.body.toString(), new RegExp(`--${boundary}\\r\\nContent-Disposition: form-data; name="purpose"\\r\\n\\r\\nworkspace\\r\\n`));
    assert.match(up.body.toString(), /filename="main.py"\r\nContent-Type: application\/octet-stream\r\n\r\nprint\(1\)\n\r\n/);
    await assert.rejects(c.uploadFile('x".py', Buffer.from("")), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
    assert.equal((await c.download(`/v1/projects/p1/workspace/files/${encodePath("a b.py")}?download=1`)).toString(), "file text");
    assert.ok(seen.some((s) => s.url === "/v1/projects/p1/workspace/files/a%20b.py?download=1"));
  } finally {
    srv.close();
  }
});
