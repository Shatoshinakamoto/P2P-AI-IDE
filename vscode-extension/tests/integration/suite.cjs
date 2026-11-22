// suite.cjs -- runs inside VS Code's extension host (loaded by run.mjs). Checks the built extension end to end
// against a fake gateway on 127.0.0.1: activation, commands, the chat view, a streamed answer, the key kept out of
// what the extension returns, a refused key and a refused address; then (X2) an agent turn that edits a file, with
// the edit kept pending until it is accepted from the diff review, and .env never sent; then (X3) missions against
// an in-memory gateway project: create and link, send the folder, switches, start, approve, bring back, review,
// triage a proposal. Throws on the first failure.
const vscode = require("vscode");
const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");

const KEY = "integration-key-0123456789";
const seen = []; // request bodies the fake gateway received
const crypto = require("node:crypto");
const sha = (b) => crypto.createHash("sha256").update(b).digest("hex");

/** An in-memory gateway project API (X3). Returns true when it handled the request. */
const proj = { files: {}, blobs: {}, version: 1, flags: { autopilot: false, auto_improve: false }, missions: [], decided: [], proposals: [{ id: "pr1", title: "Add a test", state: "open" }] };
const ARGS = { argv: ["python", "-m", "pytest"] };
const ARGS_SHA = sha(JSON.stringify(ARGS));
async function projectApi(req, res, raw) {
  const u = req.url;
  const json = (v, code = 200) => {
    res.statusCode = code;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify(v));
    return true;
  };
  const body = () => (raw.length ? JSON.parse(raw.toString()) : {});
  if (u === "/v1/projects" && req.method === "GET") return json({ data: [{ id: "p1", name: "Shop", role: "owner" }] });
  if (u === "/v1/projects" && req.method === "POST") return json({ id: "p1", name: body().name, role: "owner" });
  if (u === "/v1/experimental") return json({ autopilot: { available: true }, auto_improve: { available: true }, disabled: false });
  if (u === "/v1/projects/p1/experimental") {
    if (req.method === "PUT") Object.assign(proj.flags, body());
    return json(proj.flags);
  }
  if (u === "/v1/projects/p1/workspace") return json({ version: proj.version });
  if (u === "/v1/projects/p1/workspace/files") return json({ data: Object.entries(proj.files).map(([path, t]) => ({ path, bytes: Buffer.byteLength(t), sha256: sha(t) })) });
  if (u.startsWith("/v1/projects/p1/workspace/files/") && u.endsWith("?download=1")) {
    res.end(proj.files[decodeURIComponent(u.slice(32, -11))] ?? "");
    return true;
  }
  if (u === "/v1/files") {
    const text = raw.toString("latin1");
    const m = /filename="([^"]+)"\r\nContent-Type: application\/octet-stream\r\n\r\n([\s\S]*)\r\n--p2pide[0-9a-f]{32}--\r\n$/.exec(text);
    const id = "file_" + Object.keys(proj.blobs).length;
    proj.blobs[id] = Buffer.from(m[2], "latin1").toString("utf8");
    return json({ id, state: "ready" });
  }
  if (u === "/v1/projects/p1/workspace/commit") {
    const b = body();
    if (proj.forceConflict || b.base_version !== proj.version) return json({ error: { code: "version_conflict", message: "secret detail" } }, 409);
    for (const c of b.changes) {
      if (c.delete) delete proj.files[c.path];
      else proj.files[c.path] = proj.blobs[c.file_id];
    }
    return json({ version: ++proj.version });
  }
  if (u === "/v1/projects/p1/workspace/sync" && req.method === "POST") return json({ state: "ready" });
  if (u === "/v1/projects/p1/workspace/sync") return json({ state: "ready", version: proj.version });
  if (u === "/v1/projects/p1/workspace/sync/delta") return json({ state: "pending" });
  if (u === "/v1/missions" && req.method === "POST") {
    const b = body();
    proj.missions.push({ id: "m1", project_id: b.project_id, kind: b.kind, goal: b.goal, model: b.model, state: "running", run_id: "run1", waiting_approval: true, acceptance: b.acceptance });
    return json(proj.missions.at(-1));
  }
  if (u === "/v1/missions") return json({ data: proj.missions });
  if (u === "/v1/agents/runs/run1/approvals" && req.method === "GET") return json({ data: [{ id: "ap1", tool: "run_command", access: "exec", arguments: ARGS, arguments_sha256: ARGS_SHA }] });
  if (u === "/v1/agents/runs/run1/approvals") {
    proj.decided.push(body());
    return json({});
  }
  if (u === "/v1/projects/p1/proposals") return json({ data: proj.proposals });
  if (u.startsWith("/v1/projects/p1/proposals/pr1/")) {
    proj.proposals = [];
    return json({});
  }
  return false;
}

function fakeGateway() {
  return new Promise((resolve) => {
    const srv = http.createServer(async (req, res) => {
      if (req.headers.authorization !== `Bearer ${KEY}`) {
        res.statusCode = 401;
        return res.end("{}");
      }
      if (req.url === "/v1/models") return res.end(JSON.stringify({ data: [{ id: "fake-model" }] }));
      const chunks = [];
      for await (const c of req) chunks.push(c);
      const raw = Buffer.concat(chunks);
      if (req.url !== "/v1/chat/completions") {
        if (!(await projectApi(req, res, raw))) {
          res.statusCode = 404;
          res.end("{}");
        }
        return;
      }
      const body = raw.toString("utf8");
      seen.push(body);
      const parsed = JSON.parse(body);
      const last = parsed.messages.at(-1);
      res.setHeader("Content-Type", "text/event-stream");
      if (parsed.tools && last.role === "user") {
        // The agent's first step: ask for an edit and a read of the secret file.
        const calls = [
          { index: 0, id: "c1", type: "function", function: { name: "edit_file", arguments: JSON.stringify({ path: "hello.py", old_text: "a - b", new_text: "a + b" }) } },
          { index: 1, id: "c2", type: "function", function: { name: "read_file", arguments: JSON.stringify({ path: ".env" }) } },
        ];
        res.write(`data: ${JSON.stringify({ choices: [{ delta: { tool_calls: calls } }] })}\n\n`);
        res.write(`data: ${JSON.stringify({ choices: [{ delta: {}, finish_reason: "tool_calls" }] })}\n\n`);
        return res.end("data: [DONE]\n\n");
      }
      const asked = last.role === "tool" ? "fixed the sign" : last.content;
      for (const t of ["You said: ", asked]) res.write(`data: ${JSON.stringify({ choices: [{ delta: { content: t } }] })}\n\n`);
      res.end("data: [DONE]\n\n");
    });
    srv.listen(0, "127.0.0.1", () => resolve(srv));
  });
}

exports.run = async function run() {
  const ext = vscode.extensions.all.find((e) => e.packageJSON?.name === "p2p-ai" && e.packageJSON?.publisher === "p2p-ai");
  assert.ok(ext, "extension is loaded");
  // Opening the view activates the extension and resolves the webview.
  await vscode.commands.executeCommand("p2pAi.chat.focus");
  for (let i = 0; i < 50 && !ext.isActive; i++) await new Promise((r) => setTimeout(r, 100));
  assert.ok(ext.isActive, "activated by opening the chat view");
  const cmds = await vscode.commands.getCommands(true);
  for (const c of ["p2pAi.addGateway", "p2pAi.pickGateway", "p2pAi.pickModel", "p2pAi.setKey", "p2pAi.removeGateway", "p2pAi.newChat", "p2pAi.acceptChange", "p2pAi.rejectChange"]) {
    assert.ok(cmds.includes(c), `command ${c}`);
  }
  assert.equal(ext.exports, undefined, "no API is exported to other extensions (keys stay inside)");

  const srv = await fakeGateway();
  const url = `http://127.0.0.1:${srv.address().port}`;
  try {
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.addGateway", "Bad", "http://gw.example.org", KEY), "the address must be https://host[:port] (plain http only for this computer)");
    // The plain-chat checks first, with Tools off (they are on by default when a folder is open).
    await vscode.commands.executeCommand("p2pAi.test.setTools", false);
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.addGateway", "Fake", url, KEY), undefined);
    const list = await vscode.commands.executeCommand("p2pAi.test.gateways");
    assert.deepEqual(list, [{ name: "Fake", url }]);
    assert.ok(!JSON.stringify(list).includes(KEY), "the key is not in the saved list");
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.send", "hi"), { error: "pick a model first" });
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.setModel", "fake-model"), true);
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.send", "hello <b>there</b>"), { answer: "You said: hello <b>there</b>" });
    // A second gateway with a wrong key is refused by the fake gateway with 401.
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.addGateway", "Wrong key", url.replace("127.0.0.1", "localhost"), "wrong-key-0123456789"), undefined);
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.setModel", "fake-model"), true);
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.send", "hi"), { error: "the gateway refused the key" });

    // X2: agent tools on the open folder. Back to the working gateway, plain chat first (Tools off), then Tools on.
    const project = process.env.P2P_AI_TEST_PROJECT;
    assert.ok(project && vscode.workspace.workspaceFolders?.[0]?.uri.fsPath === fs.realpathSync(project), "the test folder is open");
    await vscode.commands.executeCommand("p2pAi.test.removeAll");
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.addGateway", "Fake", url, KEY), undefined);
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.setModel", "fake-model"), true);
    await vscode.commands.executeCommand("p2pAi.test.setTools", false);
    seen.length = 0;
    await vscode.commands.executeCommand("p2pAi.newChat");
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.send", "plain"), { answer: "You said: plain" });
    assert.ok(!JSON.parse(seen[0]).tools, "Tools off: no tools are offered");
    await vscode.commands.executeCommand("p2pAi.test.setTools", true);
    seen.length = 0;
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.send", "fix the add"), { answer: "You said: fixed the sign" });
    assert.equal(seen.length, 2, "two model steps");
    const step2 = JSON.parse(seen[1]).messages;
    assert.deepEqual(step2.filter((m) => m.role === "tool").map((m) => m.content), [
      "edit proposed; the user will review it (not written yet)",
      "error: that file is hidden from the assistant (it may hold secrets)",
    ]);
    assert.ok(!seen.join("").includes("do-not-send"), ".env never reaches the gateway");
    const file = path.join(project, "hello.py");
    assert.equal(fs.readFileSync(file, "utf8"), "def add(a, b):\n    return a - b\n", "nothing written before the review");
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.changes"), [{ rel: "hello.py", created: false, added: 1, removed: 1 }]);
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.review", "hello.py"), true, "the diff opens");
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.review", "../outside.txt"), false, "only pending changes open");
    const tab = vscode.window.tabGroups.activeTabGroup.activeTab;
    assert.ok(tab && tab.input instanceof vscode.TabInputTextDiff && tab.input.modified.scheme === "p2p-ai-review", "a read-only diff tab is shown");
    assert.equal(tab.input.modified.path, "/after/hello.py");
    const proposed = await vscode.workspace.openTextDocument(tab.input.modified);
    assert.equal(proposed.getText(), "def add(a, b):\n    return a + b\n");
    // Accept from the diff editor's title-bar command.
    await vscode.commands.executeCommand("p2pAi.acceptChange", tab.input.modified);
    assert.equal(fs.readFileSync(file, "utf8"), "def add(a, b):\n    return a + b\n", "written after Accept");
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.changes"), []);
    // A file changed on disk after the agent read it is not overwritten.
    seen.length = 0;
    fs.writeFileSync(file, "def add(a, b):\n    return a - b\n");
    await vscode.commands.executeCommand("p2pAi.test.send", "again");
    fs.writeFileSync(file, "# edited by hand\ndef add(a, b):\n    return a - b\n");
    assert.match(await vscode.commands.executeCommand("p2pAi.test.accept", "hello.py"), /changed on disk/);
    assert.equal(fs.readFileSync(file, "utf8"), "# edited by hand\ndef add(a, b):\n    return a - b\n", "the hand edit is kept");
    await vscode.commands.executeCommand("p2pAi.rejectChange", vscode.Uri.from({ scheme: "p2p-ai-review", path: "/after/hello.py" }));
    assert.deepEqual(await vscode.commands.executeCommand("p2pAi.test.changes"), []);

    // X3: missions. The view is opened so its handler runs exactly as it does for clicks.
    await vscode.commands.executeCommand("p2pAi.missions.focus");
    const ms = async (m) => {
      const r = await vscode.commands.executeCommand("p2pAi.test.missions", m);
      if (r && r.error) console.log(`missions ${m.type}: ${r.error}`);
      return r;
    };
    assert.deepEqual(await ms({ type: "push" }), { error: "pick a gateway project first" });
    assert.deepEqual(await ms({ type: "link", id: "../p1", name: "x" }), { error: "refused by the message check" }, "ids are checked before use");
    await ms({ type: "create", name: "Shop" });
    const pushRes = await ms({ type: "push" });
    const pushed = pushRes.ok;
    assert.equal(pushed?.uploaded, 1, JSON.stringify(pushRes));
    assert.deepEqual(pushed.skipped, [{ rel: ".env", reason: "may hold secrets, kept on this computer" }]);
    assert.deepEqual(Object.keys(proj.files), ["hello.py"], ".env stays home");
    const swRes = await ms({ type: "switches", autopilot: true, autoImprove: false });
    assert.equal(proj.flags.autopilot, true, JSON.stringify(swRes));
    assert.deepEqual(await ms({ type: "start", kind: "autopilot", goal: "fix the sign", model: "fake-model", checks: [] }), { error: "Autopilot needs 1 to 8 checks that say when the work is done" });
    const started = (await ms({ type: "start", kind: "autopilot", goal: "fix the sign", model: "fake-model", checks: [{ id: "check-1", kind: "tests", framework: "pytest" }] })).ok;
    assert.equal(started.id, "m1");
    assert.deepEqual(proj.missions[0].acceptance, [{ id: "check-1", kind: "tests", framework: "pytest" }]);
    assert.deepEqual(await ms({ type: "decide", runId: "run1", approvalId: "ap1", decision: "approve" }), { error: "open the approval again" }, "only an approval that was shown can be decided");
    const shown = (await ms({ type: "approvals", runId: "run1" })).ok;
    assert.equal(shown[0].arguments, JSON.stringify(ARGS, null, 2));
    await ms({ type: "decide", runId: "run1", approvalId: "ap1", decision: "approve", argumentsSha256: "0".repeat(64) });
    assert.deepEqual(proj.decided, [{ approval_id: "ap1", decision: "approve", arguments_sha256: ARGS_SHA }], "the fetched digest is sent, not one from the view");
    // The node's work lands in the project; bring it back for review.
    proj.files["hello.py"] = "def add(a, b):\n    return a + b  # by autopilot\n";
    proj.files[".env"] = "STOLEN=1\n";
    await ms({ type: "collect" });
    const pulled = (await ms({ type: "pull" })).ok;
    assert.equal(pulled.proposed, 1);
    assert.equal(fs.readFileSync(file, "utf8"), "# edited by hand\ndef add(a, b):\n    return a - b\n", "a pull writes nothing");
    assert.equal(fs.readFileSync(path.join(project, ".env"), "utf8"), "API_KEY=do-not-send\n", ".env is never brought back");
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.accept", "hello.py"), "ok");
    assert.equal(fs.readFileSync(file, "utf8"), "def add(a, b):\n    return a + b  # by autopilot\n");
    await ms({ type: "proposal", id: "pr1", action: "dismiss" });
    assert.deepEqual(proj.proposals, []);
    // A conflicting commit shows the fixed message, never the gateway's text.
    proj.forceConflict = true;
    fs.writeFileSync(file, "changed again\n");
    assert.deepEqual(await ms({ type: "push" }), { error: "the gateway project changed meanwhile; try again" }, "fixed text, never the gateway's own message");
  } finally {
    srv.close();
  }
};
