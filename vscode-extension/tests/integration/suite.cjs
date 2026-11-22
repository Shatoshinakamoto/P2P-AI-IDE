// suite.cjs -- runs inside VS Code's extension host (loaded by run.mjs). Checks the built extension end to end
// against a fake gateway on 127.0.0.1: activation, commands, the chat view, a streamed answer, the key kept out of
// what the extension returns, a refused key and a refused address. Throws on the first failure.
const vscode = require("vscode");
const http = require("node:http");
const assert = require("node:assert/strict");

const KEY = "integration-key-0123456789";

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
      const asked = JSON.parse(body).messages.at(-1).content;
      res.setHeader("Content-Type", "text/event-stream");
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
  for (const c of ["p2pAi.addGateway", "p2pAi.pickGateway", "p2pAi.pickModel", "p2pAi.setKey", "p2pAi.removeGateway", "p2pAi.newChat"]) {
    assert.ok(cmds.includes(c), `command ${c}`);
  }
  assert.equal(ext.exports, undefined, "no API is exported to other extensions (keys stay inside)");

  const srv = await fakeGateway();
  const url = `http://127.0.0.1:${srv.address().port}`;
  try {
    assert.equal(await vscode.commands.executeCommand("p2pAi.test.addGateway", "Bad", "http://gw.example.org", KEY), "the address must be https://host[:port] (plain http only for this computer)");
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
  } finally {
    srv.close();
  }
};
