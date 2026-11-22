// suite.cjs -- runs inside VS Code's extension host (loaded by run.mjs). Checks the built extension end to end
// against a fake gateway on 127.0.0.1: activation, commands, the chat view, a streamed answer, the key kept out of
// what the extension returns, a refused key and a refused address; then (X2) an agent turn that edits a file, with
// the edit kept pending until it is accepted from the diff review, and .env never sent. Throws on the first failure.
const vscode = require("vscode");
const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");

const KEY = "integration-key-0123456789";
const seen = []; // request bodies the fake gateway received

function fakeGateway() {
  return new Promise((resolve) => {
    const srv = http.createServer(async (req, res) => {
      if (req.headers.authorization !== `Bearer ${KEY}`) {
        res.statusCode = 401;
        return res.end("{}");
      }
      if (req.url === "/v1/models") return res.end(JSON.stringify({ data: [{ id: "fake-model" }] }));
      let body = "";
      for await (const c of req) body += c;
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
  } finally {
    srv.close();
  }
};
