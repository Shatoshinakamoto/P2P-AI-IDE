// tests/ui-smoke.mjs -- UI smoke test of the built webview (dist/) in headless Chromium, with the six Tauri
// commands replaced by an in-memory fake workspace. Run: `npm run build && npm run ui-smoke`.
//
// Purpose: proves the layout renders, the explorer lists, Monaco opens and edits files, Ctrl+S saves with the loaded
// version, a conflicting save shows the banner, search opens a match, and nothing logs a page error.
// Security checks: the save sends the base version (lost-update guard, CWE-367); the page loads under the same CSP
// the app ships (read from src-tauri/tauri.conf.json, ASVS V14.4) and the test fails on any CSP violation.
// Not covered: the real Rust commands (core tests cover them) and the native window (checked on a PC).
// Dependencies: playwright (preinstalled in the test environment, not a project dependency), node:http.
import { createRequire } from "node:module";
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

// playwright is resolved like require() does (it is installed globally in the test environment, not in package.json).
const { chromium } = createRequire(import.meta.url)("playwright");
const here = fileURLToPath(new URL(".", import.meta.url));
const dist = join(here, "..", "dist");
const conf = JSON.parse(await readFile(join(here, "..", "src-tauri", "tauri.conf.json"), "utf8"));
// The ipc: sources are Tauri's own transport; the fake below does not use the network at all.
const csp = conf.app.security.csp;

const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".ttf": "font/ttf", ".svg": "image/svg+xml", ".png": "image/png" };
// A tiny static server for dist/ on 127.0.0.1, sending the app's CSP header.
const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://127.0.0.1");
  // SECURITY: normalise and keep the path under dist/ (this is a test server, but no traversal either way).
  const rel = normalize(decodeURIComponent(url.pathname)).replace(/^([/\\])+/, "");
  const file = join(dist, rel === "" ? "index.html" : rel);
  if (!file.startsWith(dist)) { res.writeHead(403).end(); return; }
  try {
    await stat(file);
    res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream", "content-security-policy": csp });
    res.end(await readFile(file));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const port = server.address().port;

// The fake Tauri bridge: a workspace held in memory, with the same record shapes and error strings as the core.
const fake = () => {
  const files = new Map([
    ["src/server.py", "import os\n\nREQUEST_TIMEOUT = 5\n\ndef fetch(url):\n    return url\n"],
    ["src/orders.py", "def orders():\n    return []\n"],
    ["README.md", "# Shop API\nTimeout notes.\n"],
  ]);
  const sha = async (t) => {
    const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(t));
    return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
  };
  window.__calls = [];
  let opened = false;
  const list = (rel) => {
    const prefix = rel === "" ? "" : rel + "/";
    const names = new Map();
    for (const p of files.keys()) {
      if (!p.startsWith(prefix)) continue;
      const rest = p.slice(prefix.length);
      const i = rest.indexOf("/");
      if (i >= 0) names.set(rest.slice(0, i), "dir");
      else names.set(rest, "file");
    }
    const entries = [...names].map(([name, kind]) => ({ name, rel: prefix + name, kind, size: kind === "file" ? files.get(prefix + name).length : 0 }));
    entries.sort((a, b) => (a.kind === b.kind ? a.name.toLowerCase().localeCompare(b.name.toLowerCase()) : a.kind === "dir" ? -1 : 1));
    return { entries, truncated: false };
  };
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => 0,
    invoke: async (cmd, args) => {
      window.__calls.push({ cmd, args });
      switch (cmd) {
        case "pick_folder": opened = true; return { name: "shop-api" };
        case "workspace_info": return opened ? { name: "shop-api" } : null;
        case "list_dir": return list(args.rel);
        case "read_file": { const t = files.get(args.rel); if (t === undefined) throw "file or folder not found"; return { text: t, version: await sha(t) }; }
        case "write_file": {
          const cur = files.get(args.rel);
          if (cur !== undefined && args.baseVersion !== (await sha(cur))) throw "the file changed on disk since you opened it; reload it before saving";
          files.set(args.rel, args.text); return sha(args.text);
        }
        case "search": {
          const q = args.query.toLowerCase(); const matches = [];
          for (const [rel, t] of files) t.split("\n").forEach((l, i) => { if (l.toLowerCase().includes(q)) matches.push({ rel, line: i + 1, preview: l.trim() }); });
          return { matches, truncated: false };
        }
        default: throw "unknown command";
      }
    },
  };
  // The test changes a file "outside the IDE" through this hook.
  window.__outsideEdit = (rel, text) => files.set(rel, text);
  window.__file = (rel) => files.get(rel);
};

const checks = [];
const check = (name, ok, detail = "") => { checks.push({ name, ok, detail }); console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` -- ${detail}` : ""}`); };

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 1 });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
await page.addInitScript(fake);
// The shipped window freezes Object.prototype when tauri.conf.json sets freezePrototype; do the same here so a
// library that breaks under it fails this test instead of showing a blank window (phase I1 Windows finding).
if (conf.app?.security?.freezePrototype) await page.addInitScript(() => Object.freeze(Object.prototype));
await page.goto(`http://127.0.0.1:${port}/`);

try {
  await page.getByText("No folder is open.").waitFor({ timeout: 10000 });
  check("welcome state shows Open folder", true);
  check("status bar shows disabled gateway and mission switches",
    (await page.getByRole("button", { name: "No gateway" }).isDisabled()) &&
    (await page.getByRole("button", { name: /Autopilot off/ }).isDisabled()) &&
    (await page.getByRole("button", { name: /Auto Improve off/ }).isDisabled()));

  await page.getByRole("button", { name: "Open folder" }).first().click();
  await page.getByRole("navigation", { name: "Files" }).getByText("README.md").waitFor();
  const order = await page.getByRole("navigation", { name: "Files" }).locator("button").allInnerTexts();
  check("explorer lists folders first", order[0].trim() === "src" && order.includes("README.md"), order.join(", "));

  await page.getByRole("navigation", { name: "Files" }).getByText("src").click();
  await page.getByRole("navigation", { name: "Files" }).getByText("server.py").click();
  await page.locator(".monaco-editor .view-lines").getByText("REQUEST_TIMEOUT").waitFor({ timeout: 10000 });
  check("Monaco opens the file with its text", true);

  // Edit: put the cursor at the start and type, then save with Ctrl+S.
  await page.locator(".monaco-editor .view-lines").click();
  await page.keyboard.press("Control+Home");
  await page.keyboard.type("# edited\n");
  await page.getByRole("button", { name: "Close server.py (unsaved)" }).waitFor();
  check("typing marks the tab unsaved", true);
  await page.keyboard.press("Control+s");
  await page.getByText("Saved server.py").waitFor();
  const saved = await page.evaluate(() => window.__file("src/server.py"));
  const writeCall = await page.evaluate(() => window.__calls.find((c) => c.cmd === "write_file"));
  check("Ctrl+S saves the edited text", saved.startsWith("# edited\nimport os"));
  check("save sends the loaded version (lost-update guard)", typeof writeCall.args.baseVersion === "string" && writeCall.args.baseVersion.length === 64);

  // Multi-cursor: add a cursor on the next line (VS Code's "Add Cursor Below": Shift+Alt+Down on Linux,
  // Ctrl+Alt+Down on Windows) and type on both lines at once.
  await page.keyboard.press("Control+Home");
  await page.keyboard.press(process.platform === "win32" ? "Control+Alt+ArrowDown" : "Shift+Alt+ArrowDown");
  await page.keyboard.type("X");
  await page.keyboard.press("Control+s");
  await page.waitForTimeout(300);
  const multi = await page.evaluate(() => window.__file("src/server.py"));
  check("multi-cursor edits two lines at once", multi.startsWith("X# edited\nXimport os"), JSON.stringify(multi.slice(0, 22)));

  // Conflict: the file changes outside, then a save must be refused and the banner shown.
  await page.evaluate(() => window.__outsideEdit("src/server.py", "changed elsewhere\n"));
  await page.keyboard.type("Y");
  await page.keyboard.press("Control+s");
  await page.getByRole("alert").getByText("changed on disk").waitFor();
  const kept = await page.evaluate(() => window.__file("src/server.py"));
  check("conflicting save is refused and explained", kept === "changed elsewhere\n");
  await page.getByRole("button", { name: /Reload from disk/ }).click();
  await page.locator(".monaco-editor .view-lines").getByText("changed elsewhere").waitFor();
  check("reload replaces the editor copy with the disk version", true);

  // Second tab.
  await page.getByRole("navigation", { name: "Files" }).getByText("README.md").click();
  const tabs = await page.getByRole("tab").allInnerTexts();
  check("two files open in two tabs", tabs.length === 2, tabs.join(", "));
  // The status bar names the new tab's language at once (it lagged one tab behind before; found on Windows).
  const footer = await page.locator("footer").innerText();
  check("status bar shows the new tab's language", /markdown/.test(footer), footer.replace(/\s+/g, " "));

  // Search.
  await page.getByRole("button", { name: "Search" }).click();
  await page.getByLabel("Search in workspace (plain text)").fill("timeout");
  await page.keyboard.press("Enter");
  await page.getByText(/results? in \d+ files?/).waitFor();
  const summary = await page.getByText(/results? in \d+ files?/).innerText();
  check("search finds matches", /^[1-9]\d* results? in/.test(summary), summary);

  // Placeholders for later phases.
  for (const [label, phase] of [["Agent chat", "I2"], ["Gateways", "I2"], ["Missions: Autopilot and Auto Improve", "I4"], ["Extensions", "I6"]]) {
    await page.getByRole("button", { name: label, exact: true }).click();
    await page.getByText(new RegExp(`Arrives in phase ${phase}`)).waitFor();
  }
  check("later-phase views say when they arrive", true);

  await page.getByRole("button", { name: "Explorer" }).click();
  // The tree kept its expanded folders while other views were shown.
  await page.getByRole("navigation", { name: "Files" }).getByText("server.py").click({ timeout: 5000 });
  check("explorer keeps expanded folders across views", true);
  await page.screenshot({ path: process.env.SHOT ?? join(here, "..", "ui-smoke.png") });
} catch (e) {
  check("flow completed", false, String(e).split("\n")[0]);
}

const cspErrors = errors.filter((e) => /Content Security Policy/i.test(e));
check("no CSP violations under the shipped CSP", cspErrors.length === 0, cspErrors.slice(0, 2).join(" | "));
check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));

await browser.close();
server.close();
const failed = checks.filter((c) => !c.ok).length;
console.log(`\n${checks.length - failed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
