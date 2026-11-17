// tests/ui-smoke.mjs -- UI smoke test of the built webview (dist/) in headless Chromium, with the six Tauri
// commands replaced by an in-memory fake workspace. Run: `npm run build && npm run ui-smoke`.
//
// Purpose: proves the layout renders, the explorer lists, Monaco opens and edits files, Ctrl+S saves with the loaded
// version, a conflicting save shows the banner, search opens a match, and nothing logs a page error. Phase I2: a
// gateway is added (key goes in, never comes back), checked, made active; a chat answer streams in over a Tauri
// channel and is rendered as text (an <img onerror> in the answer must not become an element); the chat is saved,
// found by history search and deleted with two clicks.
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
  // Gateways and chats of the fake core. Keys are kept apart, as the OS keychain would, and never returned.
  const gws = []; let active = null; let dirs = []; const keys = new Map(); const chats = new Map(); let nextId = 1;
  const hex = () => (nextId++).toString(16).padStart(16, "0");
  const view = (g) => ({ id: g.id, name: g.name, url: g.url, kind: g.kind, dev: g.url.startsWith("http://"), pinned: false, has_key: keys.has(g.id) });
  const callbacks = new Map(); let cbId = 1;
  window.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => { const id = cbId++; callbacks.set(id, cb); return id; },
    unregisterCallback: (id) => callbacks.delete(id),
    invoke: async (cmd, args) => {
      window.__calls.push({ cmd, args: cmd === "chat_send" ? { model: args.model, messages: args.messages } : args });
      switch (cmd) {
        case "gateways_list": return { gateways: gws.map(view), active };
        case "gateway_add": {
          if (!/^https:\/\/[a-z0-9.-]+(:\d+)?$|^http:\/\/127\.0\.0\.1:\d+$/.test(args.url)) throw "use https://host[:port] (or http://127.0.0.1:port for a dev gateway), with no path";
          const g = { id: hex(), name: args.name || "gateway", url: args.url, kind: "manual" };
          gws.push(g); if (args.key) keys.set(g.id, args.key); if (!active) active = g.id; return g.id;
        }
        case "directories_list": return dirs;
        case "directories_set": {
          if (args.directories.some((d) => !/^https:\/\/[a-z0-9.-]+\.[a-z]+\/?$/.test(d.trim()))) throw "a directory must be a public https://host[:port] address with no user name";
          dirs = [...new Set(args.directories.map((d) => d.trim().replace(/\/$/, "")))]; return dirs;
        }
        case "public_list": return {
          gateways: [{ gateway_id: "fe812c12f3ab4ce6ac5db69ac352f906cb1b11ef", region: "eu", models: ["llama3.2:1b", "<img src=x onerror=\"window.__xss2=1\">"],
            access: { mode: "signup", api_url: "https://pub.example.org/", signup_url: "https://pub.example.org/signup", features: ["chat", "openai_api"], terms_url: "", logs_prompts: true, retention_days: 30 },
            latency_ms: 41, directories: 1 }],
          rejected: { bad_signature: 1 },
        };
        case "gateway_add_public": {
          if (args.gatewayId !== "fe812c12f3ab4ce6ac5db69ac352f906cb1b11ef") throw "that gateway is not in the last list; refresh it";
          const g = { id: hex(), name: "Public fe812c12 (eu)", url: "https://pub.example.org", kind: "public" };
          gws.push(g); return g.id;
        }
        case "gateway_set_active": active = args.id; return null;
        case "gateway_set_key": keys.set(args.id, args.key); return null;
        case "gateway_remove": { const i = gws.findIndex((g) => g.id === args.id); gws.splice(i, 1); keys.delete(args.id); if (active === args.id) active = gws[0]?.id ?? null; return null; }
        case "gateway_check": if (!keys.has(args.id)) throw "this gateway has no key yet; add one in the Gateways view"; return { latency_ms: 12, models: ["tiny-model", "big-model"] };
        case "chat_send": {
          const cb = callbacks.get(args.onEvent.id);
          const pieces = ["Hello ", "from the gateway. ", "<img src=x onerror=\"window.__xss=1\">", "\n```js\nconst a = 1;\n```"];
          pieces.forEach((text, index) => cb({ index, message: { kind: "delta", text } }));
          cb({ index: pieces.length, end: true });
          return { finish_reason: "stop", chars: pieces.join("").length };
        }
        case "chat_stop": return null;
        case "chat_save": { const c = { ...args.chat, id: args.chat.id || hex() }; if (!c.title) c.title = c.messages.find((m) => m.role === "user")?.content.split("\n")[0] ?? "New chat"; chats.set(c.id, c); return c; }
        case "chats_list": return [...chats.values()].map((c) => ({ id: c.id, title: c.title, updated: 1, count: c.messages.length }));
        case "chat_load": return chats.get(args.id);
        case "chat_delete": chats.delete(args.id); return null;
        case "chats_search": { const q = args.query.toLowerCase(); return [...chats.values()].filter((c) => c.messages.some((m) => m.content.toLowerCase().includes(q))).map((c) => ({ id: c.id, title: c.title, preview: q })); }
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
/** The last recorded call of one command. */
const window_calls = (calls, cmd) => calls.filter((c) => c.cmd === cmd).at(-1)?.args;
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
  check("status bar shows the gateway chip and disabled mission switches",
    (await page.getByRole("button", { name: "No gateway" }).isVisible()) &&
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
  for (const [label, phase] of [["Missions: Autopilot and Auto Improve", "I4"], ["Extensions", "I6"]]) {
    await page.getByRole("button", { name: label, exact: true }).click();
    await page.getByText(new RegExp(`Arrives in phase ${phase}`)).waitFor();
  }
  check("later-phase views say when they arrive", true);

  // ---- Phase I2: gateways.
  await page.getByRole("button", { name: "Agent chat", exact: true }).click();
  await page.getByText("Chat needs a gateway.").waitFor();
  check("chat without a gateway points to the Gateways view", true);
  await page.getByRole("button", { name: "Open Gateways" }).click();
  await page.getByRole("button", { name: "Add gateway" }).click();
  await page.getByLabel("Gateway name").fill("Home");
  await page.getByLabel("Gateway address").fill("https://gw.example.org/path");
  await page.getByLabel("Gateway key").fill("p2p_secret_key_123");
  await page.getByRole("button", { name: "Add by address" }).click();
  await page.getByText(/with no path/).first().waitFor();
  check("a bad gateway address is refused with the core's message", true);
  await page.getByLabel("Gateway address").fill("https://gw.example.org");
  await page.getByLabel("Gateway key").fill("p2p_secret_key_123");
  await page.getByRole("button", { name: "Add by address" }).click();
  await page.getByRole("list", { name: "Saved gateways" }).getByText("key in keychain").waitFor();
  const listed = await page.getByRole("list", { name: "Saved gateways" }).innerText();
  check("added gateway is listed with its key in the keychain, key never shown", /Home/.test(listed) && !/p2p_secret_key_123/.test(await page.content()), listed.replace(/\s+/g, " "));
  check("status bar chip names the active gateway", await page.getByRole("button", { name: /^Home/ }).isVisible());
  await page.getByRole("button", { name: "Check" }).click();
  await page.getByText("12 ms, 2 models").waitFor();
  check("Check shows latency and model count", true);
  await page.getByRole("button", { name: "Add gateway" }).click();
  await page.getByLabel("Gateway name").fill("Dev");
  await page.getByLabel("Gateway address").fill("http://127.0.0.1:8080");
  await page.getByRole("button", { name: "Add by address" }).click();
  await page.getByRole("list", { name: "Saved gateways" }).getByText("INSECURE — DEV ONLY").waitFor();
  check("a plain-http loopback gateway is marked INSECURE — DEV ONLY", true);
  await page.getByRole("button", { name: "Remove Dev" }).click();
  await page.getByRole("button", { name: "Click again to remove Dev" }).click();
  await page.getByRole("list", { name: "Saved gateways" }).getByText("Dev").waitFor({ state: "detached" });
  check("removing a gateway takes two clicks", true);

  // ---- Phase I2b: public gateway list.
  await page.getByText(/fetches nothing until you add one/).waitFor();
  check("public list fetches nothing until a directory is added", !window_calls(await page.evaluate(() => window.__calls), "public_list"));
  await page.getByLabel("Directory address").fill("https://192.168.1.5");
  await page.getByRole("button", { name: "Add directory" }).click();
  await page.getByText(/must be a public https/).first().waitFor();
  check("a private directory address is refused with the core's message", true);
  await page.getByLabel("Directory address").fill("https://dir.example.org/");
  await page.getByRole("button", { name: "Add directory" }).click();
  await page.getByRole("list", { name: "Directories" }).getByText("https://dir.example.org").waitFor();
  await page.getByRole("button", { name: "Refresh list" }).click();
  const pub = page.getByRole("list", { name: "Public gateway list" });
  await pub.getByText("41 ms").waitFor();
  const pubText = await pub.innerText();
  const pubImgs = await pub.locator("img").count();
  check("public list shows latency, access mode and the prompt-logging notice", /Sign-up gives a key/.test(pubText) && /Logs prompts, kept 30 days/.test(pubText), pubText.replace(/\s+/g, " ").slice(0, 160));
  check("advert text is shown as text (no element, no script, links not clickable)", pubImgs === 0 && (await pub.locator("a").count()) === 0 && (await page.evaluate(() => window.__xss2)) === undefined);
  await page.getByText(/1 entry skipped/).waitFor();
  check("skipped adverts are counted by reason", true);
  await page.getByRole("button", { name: "Add public gateway fe812c12" }).click();
  await page.getByRole("list", { name: "Saved gateways" }).getByText("Public fe812c12 (eu)").waitFor();
  const addCall = window_calls(await page.evaluate(() => window.__calls), "gateway_add_public");
  check("adding a public gateway sends only its advert id (the core takes the address)", addCall && Object.keys(addCall).sort().join(",") === "gatewayId,key" && addCall.key === null, JSON.stringify(addCall));
  await pub.getByText("Added").waitFor();
  check("an added public gateway is marked Added in the list", true);

  // ---- Phase I2: chat.
  await page.getByRole("button", { name: "Agent chat", exact: true }).click();
  await page.getByLabel("Model").locator("option", { hasText: "tiny-model" }).waitFor({ state: "attached" });
  await page.getByLabel("Message", { exact: true }).fill("Explain the timeout\nplease");
  await page.keyboard.press("Control+Enter");
  await page.getByRole("log", { name: "Messages" }).getByText(/from the gateway/).waitFor();
  const log = page.getByRole("log", { name: "Messages" });
  const imgs = await log.locator("img").count();
  const shown = await log.innerText();
  const xss = await page.evaluate(() => window.__xss);
  check("streamed answer is shown as text (no element from the answer, no script ran)", imgs === 0 && shown.includes("<img src=x") && xss === undefined, `imgs=${imgs}`);
  check("code fences in an answer render as a code block", (await log.locator("pre").innerText()).includes("const a = 1;"));
  const sent = window_calls(await page.evaluate(() => window.__calls), "chat_send");
  check("the request carries the chosen model and the user's message", sent?.model === "tiny-model" && sent.messages.at(-1).content.startsWith("Explain the timeout"), JSON.stringify(sent ?? {}).slice(0, 120));
  await page.screenshot({ path: process.env.SHOT_CHAT ?? join(here, "..", "ui-smoke-chat.png") });
  await page.getByRole("button", { name: "Chat history" }).click();
  await page.getByRole("list", { name: "Saved chats" }).getByText("Explain the timeout").waitFor();
  check("the chat is saved in history with a title from the first message", true);
  await page.getByLabel("Search chats").fill("FROM THE GATEWAY");
  await page.getByRole("button", { name: "Search", exact: true }).last().click();
  await page.getByRole("list", { name: "Saved chats" }).getByText("Explain the timeout").waitFor();
  check("history search finds the chat by answer text", true);
  await page.getByRole("button", { name: "Delete Explain the timeout" }).click();
  await page.getByRole("button", { name: "Confirm delete Explain the timeout" }).click();
  await page.getByText(/No chats match|No chats yet/).waitFor();
  check("deleting a chat takes two clicks", true);

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
