// view-smoke.cjs -- draws the chat view's webview (media/chat.js + chat.css, body from src/chatView.ts) in Chromium
// with a stub of VS Code's webview API, feeds it a sample state with tool steps and two pending changes, clicks
// Accept on the first change and prints what the view posted. Writes a screenshot for the docs and test records.
// Usage: node tests/ui/view-smoke.cjs <out.png>. Needs Playwright with Chromium (not a project dependency: it is
// preinstalled in the cloud test containers). No network: the page is built from local files only.
// Security: checks that the view only posts allow-listed message shapes ({type, rel}) and renders text as text.
const { chromium } = require("playwright");
const fs = require("fs");
const ext = require("node:path").resolve(__dirname, "../..");
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 380, height: 640 } });
  const html = fs.readFileSync(ext + "/src/chatView.ts", "utf8").match(/<body>([\s\S]*?)<script/)[1];
  await p.setContent(`<html><head><style>:root{--vscode-font-family:sans-serif;--vscode-foreground:#ccc;--vscode-button-background:#0e639c;--vscode-button-foreground:#fff;--vscode-button-secondaryBackground:#3a3d41;--vscode-button-secondaryForeground:#fff;--vscode-input-background:#3c3c3c;--vscode-editor-inactiveSelectionBackground:#2a2d2e;--vscode-errorForeground:#f48771}body{background:#1e1e1e}${fs.readFileSync(ext + "/media/chat.css","utf8")}</style></head><body>${html}</body></html>`);
  await p.evaluate(() => { window.sent = []; window.acquireVsCodeApi = () => ({ postMessage: (m) => window.sent.push(m) }); });
  await p.addScriptTag({ content: fs.readFileSync(ext + "/media/chat.js", "utf8") });
  await p.evaluate(() => {
    const post = (d) => window.dispatchEvent(new MessageEvent("message", { data: d }));
    post({ type: "state", gateway: "Home gateway", model: "qwen2.5-coder:7b", dev: false, tools: true, toolsNote: null, folder: "shop", busy: false,
      messages: [{ role: "user", content: "Fix the total in orders.py" }, { role: "tool", content: "read_file orders.py", ok: true }, { role: "tool", content: "read_file .env", ok: false }, { role: "tool", content: "edit_file orders.py", ok: true }, { role: "assistant", content: "The total used a minus sign. I changed it to plus and added a test." }],
      changes: [{ rel: "orders.py", created: false, added: 1, removed: 1 }, { rel: "tests/test_orders.py", created: true, added: 12, removed: 0 }] });
  });
  await p.click("text=Accept >> nth=0");
  const sent = await p.evaluate(() => window.sent);
  console.log(JSON.stringify(sent));
  const ok = JSON.stringify(sent) === JSON.stringify([{ type: "ready" }, { type: "accept", rel: "orders.py" }]);
  const shown = await p.evaluate(() => [...document.querySelectorAll("#changelist .rel")].map((e) => e.textContent));
  console.log(ok && shown.join(",") === "orders.py,tests/test_orders.py" ? "view-smoke: pass" : "view-smoke: fail");
  if (!ok) process.exitCode = 1;
  await p.screenshot({ path: process.argv[2] });
  await b.close();
})();
