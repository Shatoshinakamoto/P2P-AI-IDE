// missions-smoke.cjs -- draws the Missions view's webview (media/missions.js + missions.css, body from
// src/missionsView.ts) in Chromium with a stub of VS Code's webview API, feeds it a sample state (linked project,
// a running Autopilot mission waiting for approval, a proposal, an approval with hostile-looking arguments), clicks
// Approve and prints what the view posted. Writes a screenshot for the docs and test records.
// Usage: node tests/ui/missions-smoke.cjs <out.png>. Needs Playwright with Chromium (preinstalled in the cloud test
// containers; not a project dependency). No network: the page is built from local files only.
// Security: checks that markup in gateway text is shown as text (CWE-79) and that the view posts no digest of its own.
const { chromium } = require("playwright");
const fs = require("fs");
const path = require("path");
const ext = path.resolve(__dirname, "../..");
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 380, height: 900 } });
  const html = fs.readFileSync(ext + "/src/missionsView.ts", "utf8").match(/<body>([\s\S]*?)<script/)[1];
  const vars = ":root{--vscode-font-family:sans-serif;--vscode-foreground:#ccc;--vscode-button-background:#0e639c;--vscode-button-foreground:#fff;--vscode-button-secondaryBackground:#3a3d41;--vscode-button-secondaryForeground:#fff;--vscode-input-background:#3c3c3c;--vscode-input-foreground:#ccc;--vscode-editor-inactiveSelectionBackground:#2a2d2e;--vscode-errorForeground:#f48771;--vscode-textCodeBlock-background:#111}body{background:#1e1e1e}";
  await p.setContent(`<html><head><style>${vars}${fs.readFileSync(ext + "/media/missions.css", "utf8")}</style></head><body>${html}</body></html>`);
  await p.evaluate(() => {
    window.sent = [];
    window.acquireVsCodeApi = () => ({ postMessage: (m) => window.sent.push(m) });
  });
  await p.addScriptTag({ content: fs.readFileSync(ext + "/media/missions.js", "utf8") });
  await p.evaluate(() => {
    const post = (d) => window.dispatchEvent(new MessageEvent("message", { data: d }));
    post({
      type: "state", gateway: "Home gateway", folder: "shop", block: null, project: { id: "p1", name: "Shop" }, busy: false,
      switches: { autopilot_available: true, improve_available: true, disabled: false, autopilot: true, auto_improve: false },
      sync: { state: "ready", delta_state: "committed", delta_added: 0, delta_modified: 1, delta_deleted: 0 },
      missions: [{ id: "m1", kind: "autopilot", goal: "Make the tests in tests/ pass <img src=x onerror=alert(1)>", state: "running", cycle: 2, max_cycles: 6, run_id: "run1", waiting_approval: true, checks_passed: 1, checks_total: 2, tasks_done: 2, tasks_total: 4 }],
      proposals: [{ id: "pr1", title: "Add a test for refunds", effort: "small", rationale: "Refund totals have no test.", steps: ["Write tests/test_refunds.py", "Cover partial refunds"], sources: ["orders.py"] }],
    });
    post({ type: "pushed", report: { uploaded: 12, deleted: 0, unchanged: 30, version: 4, copy: "ready", skipped: [{ rel: ".env", reason: "may hold secrets, kept on this computer" }] } });
    post({ type: "approvals", runId: "run1", approvals: [{ id: "ap1", tool: "run_command", access: "exec", arguments: '{\n  "argv": [\n    "python",\n    "-m",\n    "pytest"\n  ]\n}' }] });
  });
  await p.click("text=Approve");
  const sent = await p.evaluate(() => window.sent);
  const imgs = await p.evaluate(() => document.querySelectorAll("img").length);
  console.log(JSON.stringify(sent));
  const ok = imgs === 0 && JSON.stringify(sent) === JSON.stringify([{ type: "ready" }, { type: "decide", runId: "run1", approvalId: "ap1", decision: "approve" }]);
  await p.screenshot({ path: process.argv[2], fullPage: true });
  console.log(ok ? "missions-smoke: pass" : "missions-smoke: fail");
  if (!ok) process.exitCode = 1;
  await b.close();
})();
