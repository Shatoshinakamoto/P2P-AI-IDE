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
  const gws = []; let active = null; let dirs = []; const pending = new Map(); const keys = new Map(); const chats = new Map(); let nextId = 1;
  const hex = () => (nextId++).toString(16).padStart(16, "0");
  const view = (g) => ({ id: g.id, name: g.name, url: g.url, kind: g.kind, dev: g.kind !== "tandemforge" && g.url.startsWith("http://"), pinned: false, has_key: keys.has(g.id) });
  const callbacks = new Map(); let cbId = 1;
  const mp = { linked: false, name: "", missions: [],
    sw: { autopilot_available: true, improve_available: true, disabled: false, autopilot: false, auto_improve: false, resource_scope: "own" },
    sync: { state: "none", version: 0, delta_state: "", delta_version: 0, delta_added: 0, delta_modified: 0, delta_deleted: 0, delta_conflicts: [] },
    proposals: [{ id: "prp_1", title: "Add CI", kind: "integration", rationale: "<b>bold</b> idea", effort: "small", steps: [], sources: ["https://example.org/docs"], state: "new" }] };
  // Phase I5: the fake repository. The first status is refused (as for a repository with a filter program).
  const scm = { refuse: true, staged: new Set(), log: [],
    files: [{ rel: "src/orders.py", index: ".", work: "M", conflict: false, untracked: false, actionable: true },
      { rel: "<img src=x onerror=window.__xss5=1>.md", index: ".", work: "?", conflict: false, untracked: true, actionable: true }] };
  const scmView = () => scm.files.map((f) => (scm.staged.has(f.rel) ? { ...f, index: f.untracked ? "A" : f.work, work: ".", untracked: false } : f));
  const runResult = {
    runner: "gateway", place: "node-1", summary: "", exit_code: 1, timed_out: false, duration_ms: 2300,
    stdout: "collected 4 items\n<b>bold</b><script>window.__xss5=1</script>\n3 passed, 1 failed", stderr: "E assert 1 == 2",
    truncated: false, tests: { passed: 3, failed: 1, skipped: 0, total: 4, failures: ["tests/test_orders.py::test_<b>total</b>"] }, error: "",
  };
  // Phase I6a: the fake Open VSX and extension store. One theme installs; one extension that runs code is refused.
  const ext = { installed: [] };
  const extTheme = { name: "night-theme", publisher: "smoke", displayName: "Night Theme", version: "1.0.0", engines: { vscode: "^1.80.0" },
    contributes: { themes: [{ id: "Smoke Night", label: "Smoke Night", uiTheme: "vs-dark", path: "./themes/night.json" }] } };
  const extThemeFile = JSON.stringify({ name: "Smoke Night", type: "dark", colors: { "editor.background": "#123456" }, tokenColors: [] });
  window.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => { const id = cbId++; callbacks.set(id, cb); return id; },
    unregisterCallback: (id) => callbacks.delete(id),
    invoke: async (cmd, args) => {
      window.__calls.push({ cmd, args: cmd === "chat_send" || cmd === "agent_send" ? { model: args.model, messages: args.messages, runs: args.runs, runner: args.runner } : args });
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
        case "agent_send": {
          const cb = callbacks.get(args.onEvent.id);
          if (args.runs) {
            // Phase I5b: the assistant asks to run; nothing runs until agent_decide answers this id.
            const id = hex();
            cb({ index: 0, message: { kind: "approval", id, summary: "run_tests pytest <i>now</i>", runner: args.runner } });
            const allow = await new Promise((res) => { window.__decide = { id, res }; });
            if (allow) {
              cb({ index: 1, message: { kind: "run", id, result: { ...runResult, summary: "run_tests pytest", exit_code: 0, tests: { passed: 4, failed: 0, skipped: 0, total: 4, failures: [] } } } });
              cb({ index: 2, message: { kind: "tool", name: "run_tests", target: "run_tests pytest", ok: true } });
            } else {
              cb({ index: 1, message: { kind: "tool", name: "run_tests", target: "run_tests pytest", ok: false } });
              cb({ index: 2, message: { kind: "delta", text: "" } });
            }
            cb({ index: 3, message: { kind: "delta", text: allow ? "All 4 tests pass." : "OK, not running it." } });
            cb({ index: 4, end: true });
            return { steps: 2, tool_calls: 1, pending: pending.size, finish_reason: "stop" };
          }
          // Phase I3: text, a tool step, hostile text, and two proposed changes that are NOT written.
          const evs = [
            { kind: "delta", text: "Hello " },
            { kind: "tool", name: "read_file", target: "src/orders.py", ok: true },
            { kind: "tool", name: "read_file", target: ".env", ok: false },
            { kind: "delta", text: "from the gateway. " },
            { kind: "delta", text: "<img src=x onerror=\"window.__xss=1\">" },
            { kind: "delta", text: "\n```js\nconst a = 1;\n```" },
          ];
          evs.forEach((message, index) => cb({ index, message }));
          cb({ index: evs.length, end: true });
          pending.set("src/orders.py", { before: files.get("src/orders.py"), after: "def orders():\n    return [\"<b>fixed</b>\"]\n", created: false });
          pending.set("src/new_util.py", { before: "", after: "x = 1\n", created: true });
          return { steps: 2, tool_calls: 2, pending: pending.size, finish_reason: "stop" };
        }
        case "changes_list": return [...pending].map(([rel, c]) => ({ rel, created: c.created, added: 1, removed: c.created ? 0 : 1 }));
        case "change_get": { const c = pending.get(args.rel); if (!c) throw "that change is no longer pending"; return { rel: args.rel, ...c }; }
        case "change_accept": { const c = pending.get(args.rel); files.set(args.rel, c.after); pending.delete(args.rel); return sha(c.after); }
        case "change_reject": pending.delete(args.rel); return null;
        // ---- Phase I4: a gateway project with missions, held in memory like the Go gateway would.
        case "mission_project": return mp.linked
          ? { gateway_name: "Home", project_id: "prj_1", project_name: mp.name, switches: mp.sw, sync: mp.sync, note: "" }
          : { gateway_name: "Home", project_id: null, project_name: "", switches: null, sync: null, note: "" };
        case "projects_list": return [{ id: "prj_1", name: "shop-api", role: "owner", version: 0, file_count: 0 }];
        case "project_create": if (!args.name) throw "a project name is 1 to 128 characters"; mp.linked = true; mp.name = args.name; return { id: "prj_1", name: args.name, role: "owner", version: 0, file_count: 0 };
        case "project_link": mp.linked = args.projectId !== null; return null;
        case "project_push": mp.sync.state = "ready"; return { uploaded: 3, deleted: 0, unchanged: 0, skipped: [{ rel: ".env", reason: "may hold secrets, kept on this computer" }], version: 1, copy: "pending" };
        case "switches_set": mp.sw = { ...mp.sw, autopilot: args.autopilot, auto_improve: args.autoImprove }; return mp.sw;
        case "missions_list": return mp.missions;
        case "mission_start": {
          const m = { id: "msn_1", kind: args.kind, project_id: "prj_1", model: args.model, goal: args.goal, state: "running", reason: "", cycle: 1, max_cycles: 20, tokens: 1200,
            run_id: "run_1", waiting_approval: true, checks_passed: 0, checks_total: args.checks.length, tasks_done: 0, tasks_total: 1, proposals_created: 0, updated_at: 1,
            tasks: [{ title: "<img src=x onerror=\"window.__xss3=1\">Fix orders()", state: "working" }], checks: [] };
          mp.missions = [m]; return m;
        }
        case "mission_control": mp.missions = mp.missions.map((m) => (m.id === args.id ? { ...m, state: args.action === "stop" ? "stopped" : args.action === "pause" ? "paused" : "running" } : m)); return null;
        case "approvals_list": return [{ id: "apr_1", tool: "write_file", access: "write", arguments: "{\n  \"path\": \"src/orders.py\",\n  \"content\": \"<script>window.__xss4=1</script>\"\n}", arguments_sha256: "a".repeat(64), expires_at: 1 }];
        case "approval_decide": mp.missions = mp.missions.map((m) => ({ ...m, waiting_approval: false })); return null;
        case "proposals_list": return mp.proposals;
        case "proposal_act": mp.proposals = mp.proposals.map((p) => (p.id === args.id ? { ...p, state: args.action === "accept" ? "accepted" : args.action } : p)); return null;
        case "project_collect": mp.sync = { ...mp.sync, delta_state: "committed", delta_version: 2, delta_added: 0, delta_modified: 1, delta_deleted: 0 }; return mp.sync;
        case "project_pull": pending.set("src/orders.py", { before: files.get("src/orders.py"), after: "def orders():\n    return load()\n", created: false }); return { proposed: 1, same: 2, only_local: 0, skipped: [] };
        case "scm_status":
          if (scm.refuse) { scm.refuse = false; return { available: false, reason: "this repository's own git settings name programs to run (filters, signing or includes), so source control is off for it", status: null }; }
          return { available: true, reason: "", status: { branch: "main", initial: false, ahead: 1, behind: 0, files: scmView(), truncated: false } };
        case "scm_log": return scm.log;
        case "scm_stage": if (args.rel === null) scm.files.forEach((f) => scm.staged.add(f.rel)); else scm.staged.add(args.rel); return null;
        case "scm_unstage": if (args.rel === null) scm.staged.clear(); else scm.staged.delete(args.rel); return null;
        case "scm_diff": return { rel: args.rel, before: "def orders():\n    return []\n", after: files.get(args.rel) ?? "", deleted: false };
        case "scm_commit": {
          if (scm.staged.size === 0) throw "stage at least one change before committing";
          scm.log.unshift({ id: "abc1234", subject: args.message, author: "Dev", time: 1 });
          scm.files = scm.files.filter((f) => !scm.staged.has(f.rel)); scm.staged.clear(); return "abc1234";
        }
        case "agent_decide": {
          if (!window.__decide || window.__decide.id !== args.id) throw "that run request is no longer waiting";
          const d = window.__decide; window.__decide = null; d.res(args.allow); return null;
        }
        case "runners_list": return [
          { id: "gateway", available: true, reason: "" },
          { id: "local-os", available: true, reason: "" },
          { id: "container", available: true, reason: "" },
        ];
        // Phase I5d: the Node.js image is missing until Get image.
        case "container_images": return [
          { id: "python", label: "Python 3.12 (UBI 9)", present: true, get: "pull" },
          { id: "node", label: "Node.js 22 (UBI 9)", present: !!window.__nodeImage, get: "pull" },
          { id: "go", label: "Go toolset (UBI 9)", present: true, get: "pull" },
          { id: "rust", label: "Rust toolset (built here from UBI 9)", present: false, get: "build" },
        ];
        case "container_prepare": {
          if (!["python", "node", "go", "rust"].includes(args.id)) throw "pick an image from the list";
          const cb = callbacks.get(args.onStage.id);
          cb({ index: 0, message: "pulling" });
          cb({ index: 1, end: true });
          if (args.id === "node") window.__nodeImage = true;
          return null;
        }
        case "run_start": {
          // Phase I5b: stages arrive on the channel; the result carries hostile output to check it is text.
          if (!["gateway", "local-os", "container"].includes(args.runner)) throw "this runner is not available yet";
          const cb = callbacks.get(args.onStage.id);
          ["sending", "copying", "running"].forEach((message, index) => cb({ index, message }));
          cb({ index: 3, end: true });
          return { ...runResult, summary: args.input.kind === "tests" ? `run_tests ${args.input.framework}` : `run_command ${args.input.argv.join(" ")}` };
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
        case "ext_search": return [
          { namespace: "smoke", name: "night-theme", version: "1.0.0", display_name: "Night Theme", description: "<img src=x onerror=window.__xss6=1> a dark theme", downloads: 1234 },
          { namespace: "smoke", name: "runs-code", version: "2.0.0", display_name: "Runs Code", description: "needs a host", downloads: 5 }];
        case "ext_install": {
          if (args.name === "runs-code") throw "this extension runs Node.js code; the Node extension host arrives in phase I6c";
          const e = { id: "smoke.night-theme", namespace: "smoke", name: "night-theme", version: "1.0.0", display_name: "Night Theme", description: "", kind: "declarative", enabled: true, installed_at: 1, sha256: "b".repeat(64), verified_publisher: true, folder: "smoke.night-theme-1.0.0" };
          ext.installed = [e]; return e;
        }
        case "ext_list": return ext.installed;
        case "ext_set_enabled": ext.installed = ext.installed.map((e) => (e.id === args.id ? { ...e, enabled: args.enabled } : e)); return ext.installed.find((e) => e.id === args.id);
        case "ext_uninstall": ext.installed = ext.installed.filter((e) => e.id !== args.id); return null;
        case "ext_manifest": if (!ext.installed.some((e) => e.id === args.id && e.enabled)) throw "extension not found"; return extTheme;
        case "ext_file": {
          if (!ext.installed.some((e) => e.id === args.id && e.enabled)) throw "extension not found";
          if (args.path !== "themes/night.json") throw "file not found";
          return new TextEncoder().encode(extThemeFile).buffer;
        }
        // Phase I7: TandemForge on this computer. The fake engine accepts one pairing code; the core keeps it.
        case "tandemforge_status": return { running: true, reason: null, id: gws.find((g) => g.kind === "tandemforge")?.id ?? null };
        case "tandemforge_connect": {
          if (!/^tfa_[A-Za-z0-9_-]{40,100}$/.test(args.code.trim())) throw "paste the pairing code from TandemForge (Settings > Connected apps); it starts with tfa_";
          if (args.code.trim() !== "tfa_" + "Q".repeat(43)) throw "TandemForge did not accept this code; connect the IDE again in TandemForge";
          const g = { id: hex(), name: "TandemForge (this computer)", url: "http://127.0.0.1:41234", kind: "tandemforge" };
          gws.push(g); keys.set(g.id, args.code.trim()); active = g.id; return { latency_ms: 3, models: ["llama3.2:1b"] };
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
  check("status bar shows the gateway chip and the mission switches (off until a project says otherwise)",
    (await page.getByRole("button", { name: "No gateway" }).isVisible()) &&
    (await page.getByRole("button", { name: /Autopilot off/ }).isVisible()) &&
    (await page.getByRole("button", { name: /Auto Improve off/ }).isVisible()));

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

  // ---- Phase I5: source control.
  await page.getByRole("button", { name: "Source control", exact: true }).click();
  await page.getByText(/source control is off for it/).waitFor();
  check("a refused repository says why and offers no actions", (await page.getByRole("button", { name: /^Stage / }).count()) === 0);
  await page.getByRole("button", { name: "Check again" }).click();
  await page.getByRole("region", { name: "Changes", exact: true }).getByText("src/orders.py").waitFor();
  const scmText = await page.getByRole("region", { name: "Changes", exact: true }).innerText();
  check("source control lists the branch's changes; odd file names are text", /onerror/.test(scmText) && (await page.evaluate(() => window.__xss5)) === undefined, scmText.replace(/\s+/g, " ").slice(0, 120));
  check("Commit is off until something is staged and a message is written", await page.getByRole("button", { name: /^Commit/ }).isDisabled());
  await page.getByRole("button", { name: "Stage src/orders.py" }).click();
  await page.getByRole("region", { name: "Staged changes" }).getByText("src/orders.py").waitFor();
  const stageCall = (await page.evaluate(() => window.__calls)).find((c) => c.cmd === "scm_stage");
  check("staging sends one file's path", stageCall?.args.rel === "src/orders.py", JSON.stringify(stageCall?.args));
  await page.getByRole("region", { name: "Staged changes" }).getByText("src/orders.py").click();
  await page.getByText("left: last commit · right: on disk (view only)").waitFor();
  check("a file's diff against the last commit is view-only (no Accept or Reject)", (await page.getByRole("button", { name: "Accept and save" }).count()) === 0);
  await page.getByRole("dialog").getByRole("button", { name: "Close" }).click();
  await page.getByLabel("Commit message").fill("Fix orders <b>now</b>");
  await page.getByRole("button", { name: /^Commit 1 staged/ }).click();
  await page.getByRole("region", { name: "History" }).getByText("abc1234").waitFor();
  const commitCall = (await page.evaluate(() => window.__calls)).find((c) => c.cmd === "scm_commit");
  check("Commit sends the message and the history shows it as text", commitCall?.args.message === "Fix orders <b>now</b>" && /Fix orders <b>now<\/b>/.test(await page.getByRole("region", { name: "History" }).innerText()));
  await page.getByRole("region", { name: "Staged changes" }).getByText("Staged (0)").waitFor();
  check("after the commit the message box is empty and Staged is empty", (await page.getByLabel("Commit message").inputValue()) === "");

  // The Extensions view (phase I6a) is tested in full near the end; here it only has to open with an empty list.
  await page.getByRole("button", { name: "Extensions", exact: true }).click();
  await page.getByText("No extensions installed.").waitFor();
  check("the Extensions view opens with nothing installed", true);
  await page.getByRole("button", { name: "Missions: Autopilot and Auto Improve", exact: true }).click();
  await page.getByText("Missions run on a gateway. Add one first.").waitFor();
  check("Missions without a gateway points to the Gateways view", true);

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
  const sent = window_calls(await page.evaluate(() => window.__calls), "agent_send");
  check("with a folder open and Tools on, the turn runs with tools and carries the model and message", sent?.model === "tiny-model" && sent.messages.at(-1).content.startsWith("Explain the timeout"), JSON.stringify(sent ?? {}).slice(0, 120));
  // ---- Phase I3: tool steps and the review of proposed changes.
  check("tool steps are shown in the chat (name and target only)", /read_file src\/orders\.py/.test(shown) && /failed: read_file \.env/.test(shown), shown.replace(/\s+/g, " ").slice(0, 160));
  const changesBox = page.getByRole("list", { name: "Pending changes" });
  await changesBox.getByText("src/orders.py").waitFor();
  check("proposed edits are listed under Changes and not written yet", (await page.evaluate(() => window.__file("src/orders.py"))) === "def orders():\n    return []\n");
  // Open the file in the editor first, so the accept must refresh its tab.
  await page.getByRole("button", { name: "Explorer" }).click();
  await page.getByRole("navigation", { name: "Files" }).getByText("orders.py").click();
  await page.locator(".view-lines").getByText("return []").waitFor();
  await page.getByRole("button", { name: "Agent chat", exact: true }).click();
  await page.getByRole("button", { name: "Review src/orders.py" }).click();
  const dlg = page.getByRole("dialog", { name: "Review change to src/orders.py" });
  await dlg.waitFor();
  await dlg.locator(".monaco-diff-editor").waitFor();
  await dlg.getByText("fixed").first().waitFor();
  check("the diff view shows the proposed text as text (no element from it)", (await dlg.locator("b").count()) === 0);
  await page.screenshot({ path: process.env.SHOT_DIFF ?? join(here, "..", "ui-smoke-diff.png") });
  await dlg.getByRole("button", { name: "Accept and save" }).click();
  await dlg.waitFor({ state: "detached" });
  check("Accept writes the reviewed file", (await page.evaluate(() => window.__file("src/orders.py"))).includes("fixed"));
  await page.locator(".view-lines").getByText("fixed").waitFor({ timeout: 5000 });
  check("an open clean tab reloads after the change is accepted", true);
  await page.getByRole("button", { name: "Reject src/new_util.py" }).click();
  await changesBox.waitFor({ state: "detached" });
  check("Reject drops a change without writing it", (await page.evaluate(() => window.__file("src/new_util.py"))) === undefined);
  await page.getByRole("button", { name: "Tools", exact: true }).click();
  await page.getByLabel("Message", { exact: true }).fill("plain question");
  await page.keyboard.press("Control+Enter");
  await page.getByRole("log", { name: "Messages" }).getByText("plain question").waitFor();
  await page.getByRole("button", { name: "Stop" }).waitFor({ state: "detached" });
  const plain = window_calls(await page.evaluate(() => window.__calls), "chat_send");
  check("with Tools off the turn is a plain chat, and tool lines are not sent to the model", plain?.messages.at(-1).content === "plain question" && plain.messages.every((m) => m.role === "user" || m.role === "assistant"), JSON.stringify(plain?.messages.map((m) => m.role)));
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

  // ---- Phase I4: Missions on a gateway project.
  await page.getByRole("button", { name: "Missions: Autopilot and Auto Improve", exact: true }).click();
  await page.getByRole("button", { name: "Create project and link" }).click();
  await page.getByRole("button", { name: /Send folder to the project/ }).click();
  const lastSend = page.getByLabel("Last send");
  await lastSend.waitFor();
  check("sending the folder reports what was sent and what stayed home", /3 sent/.test(await lastSend.innerText()) && /1 kept on this computer/.test(await lastSend.innerText()));
  await page.getByRole("checkbox", { name: "Autopilot" }).check();
  await page.getByRole("button", { name: /Autopilot on/ }).waitFor();
  check("switching Autopilot on for the project shows in the status bar", window_calls(await page.evaluate(() => window.__calls), "switches_set")?.autopilot === true);
  await page.getByLabel("Mission goal").fill("Make the tests pass");
  await page.getByRole("button", { name: "Start Autopilot" }).click();
  await page.getByRole("list", { name: "Missions" }).getByText("Make the tests pass").waitFor();
  const start = window_calls(await page.evaluate(() => window.__calls), "mission_start");
  check("Autopilot starts with the goal, model and a tests check", start?.kind === "autopilot" && start.model === "tiny-model" && start.checks[0]?.framework === "pytest", JSON.stringify(start));
  await page.getByRole("button", { name: "Review approval" }).click();
  const appr = page.getByLabel("Approval write_file");
  await appr.waitFor();
  check("an approval shows the call's arguments as text (no element, no script)", (await appr.locator("pre").innerText()).includes("<script>") && (await appr.locator("script").count()) === 0 && (await page.evaluate(() => window.__xss4)) === undefined);
  await appr.getByRole("button", { name: "Approve" }).click();
  await page.getByRole("button", { name: "Review approval" }).waitFor({ state: "detached" });
  const dec = window_calls(await page.evaluate(() => window.__calls), "approval_decide");
  check("Approve sends the decision bound to the arguments' digest", dec?.decision === "approve" && dec.argumentsSha256 === "a".repeat(64), JSON.stringify(dec));
  const props = page.getByRole("list", { name: "Proposals" });
  check("proposals show text only and their sources are not links", (await props.locator("b").count()) === 0 && (await props.locator("a").count()) === 0);
  await props.getByRole("button", { name: "Dismiss" }).click();
  await props.getByText(/dismiss/).waitFor();
  check("a proposal can be dismissed", true);
  await page.getByRole("button", { name: "Stop", exact: true }).click();
  await page.getByRole("button", { name: "Stop for good" }).click();
  await page.getByRole("list", { name: "Missions" }).getByText("stopped").waitFor();
  check("stopping a mission takes two clicks", true);
  check("task titles from the gateway are shown as text", (await page.evaluate(() => window.__xss3)) === undefined);
  await page.getByRole("button", { name: "Collect changes" }).click();
  await page.getByLabel("Last collect").getByText(/committed/).waitFor();
  check("Collect changes shows the new version", true);
  await page.getByRole("button", { name: "Review changes" }).click();
  await page.getByText("src/orders.py").first().waitFor();
  check("pulled files wait in Changes to review and are not written", (await page.evaluate(() => window.__file("src/orders.py"))).includes("fixed") && !(await page.evaluate(() => window.__file("src/orders.py"))).includes("load()"));

  await page.getByRole("button", { name: "Explorer" }).click();
  // The tree kept its expanded folders while other views were shown.
  await page.getByRole("navigation", { name: "Files" }).getByText("server.py").click({ timeout: 5000 });
  check("explorer keeps expanded folders across views", true);
  const saves = (await page.evaluate(() => window.__calls)).filter((c) => c.cmd === "chat_save").map((c) => c.args.chat.id);
  check("switching views keeps the chat in progress (one chat, saved twice)", saves.length === 2 && saves[0] === "" && saves[1] !== "", JSON.stringify(saves));

  // ---- Phase I5b: Run and test view, and runs the assistant asks for.
  await page.getByRole("button", { name: "Run and test", exact: true }).click();
  const runnerSel = page.getByLabel("Where to run");
  await runnerSel.locator("option", { hasText: "Local sandbox" }).waitFor({ state: "attached" });
  const disabledRunners = await runnerSel.locator("option[disabled]").allInnerTexts();
  const allRunners = await runnerSel.locator("option").allInnerTexts();
  check("the gateway, local sandbox and container runners are offered", disabledRunners.length === 0 && allRunners.length === 3 && allRunners.some((t) => /Docker or Podman/.test(t)) && (await runnerSel.inputValue()) === "gateway", JSON.stringify(allRunners));
  await page.getByRole("button", { name: "Run pytest tests" }).click();
  const res = page.getByRole("region", { name: "Run result" });
  await res.waitFor();
  const resText = await res.innerText();
  check("a test run shows the verdict, counts and failing tests", /Failed \(exit code 1\)/.test(resText) && /3 passed, 1 failed/.test(resText) && resText.includes("test_<b>total</b>"), resText.replace(/\s+/g, " ").slice(0, 160));
  check("run output is shown as text (no element, no script)", (await res.locator("b, script").count()) === 0 && resText.includes("<script>") && (await page.evaluate(() => window.__xss5)) === undefined);
  const rs1 = window_calls(await page.evaluate(() => window.__calls), "run_start");
  check("a test run sends the runner, framework and time limit", rs1?.runner === "gateway" && rs1.input.kind === "tests" && rs1.input.framework === "pytest" && rs1.input.timeout_s === 120, JSON.stringify(rs1));
  await page.getByRole("radio", { name: "Command" }).check();
  await page.getByLabel(/Program and arguments/).fill('echo "a b" ; ls $HOME');
  await page.getByRole("button", { name: "Run command" }).click();
  await res.getByText(/run_command echo a b/).waitFor();
  const rs2 = window_calls(await page.evaluate(() => window.__calls), "run_start");
  check("a command is sent as words; shell signs are plain words", JSON.stringify(rs2?.input.argv) === JSON.stringify(["echo", "a b", ";", "ls", "$HOME"]), JSON.stringify(rs2?.input));
  // Phase I5c: the local sandbox.
  await runnerSel.selectOption("local-os");
  await page.getByText(/your home folder hidden/).waitFor();
  await page.getByRole("button", { name: "Run command" }).click();
  await res.waitFor();
  check("a run on the local sandbox goes to the local runner", window_calls(await page.evaluate(() => window.__calls), "run_start")?.runner === "local-os");
  // Phase I5d: the container runner, with an image that has to be fetched first.
  await runnerSel.selectOption("container");
  await page.getByText(/throw-away container/).waitFor();
  const imageSel = page.getByLabel("Image", { exact: true });
  await imageSel.selectOption("node");
  const getBtn = page.getByRole("button", { name: "Get image" });
  await getBtn.waitFor();
  check("a missing image offers Get image and no run", await page.getByRole("button", { name: "Run command" }).isDisabled());
  await getBtn.click();
  await getBtn.waitFor({ state: "detached" });
  check("Get image asks for that image by its id", window_calls(await page.evaluate(() => window.__calls), "container_prepare")?.id === "node");
  await page.getByRole("button", { name: "Run command" }).click();
  await res.waitFor();
  const rs3 = window_calls(await page.evaluate(() => window.__calls), "run_start");
  check("a container run sends the runner and the chosen image", rs3?.runner === "container" && rs3.image === "node", JSON.stringify(rs3));
  await page.getByRole("radio", { name: "Tests" }).check();
  await page.getByLabel("Test framework").selectOption("cargo");
  await page.getByRole("button", { name: "Get image" }).waitFor();
  check("cargo tests need the Rust image, which is built here", /Building it downloads/.test(await page.getByText(/is not on this computer yet/).innerText()));
  await page.getByLabel("Test framework").selectOption("pytest");
  await runnerSel.selectOption("local-os");

  await page.getByRole("button", { name: "Agent chat", exact: true }).click();
  await page.getByRole("button", { name: "New chat" }).click();
  const histBtn = page.getByRole("button", { name: "Chat history" });
  if ((await histBtn.getAttribute("aria-pressed")) === "true") await histBtn.click();
  const toolsBtn = page.getByRole("button", { name: "Tools", exact: true });
  if ((await toolsBtn.getAttribute("aria-pressed")) !== "true") await toolsBtn.click();
  const runsBtn = page.getByRole("button", { name: "Runs", exact: true });
  check("Runs is off until the person turns it on", (await runsBtn.getAttribute("aria-pressed")) === "false");
  await runsBtn.click();
  await page.getByLabel("Message", { exact: true }).fill("run the tests");
  await page.keyboard.press("Control+Enter");
  const card = page.getByRole("alertdialog", { name: "Run request" });
  await card.waitFor();
  check("a run request shows exactly what would run and where, as text", (await card.locator("pre").innerText()) === "run_tests pytest <i>now</i>" && (await card.locator("i").count()) === 0 && /local sandbox on this computer/.test(await card.innerText()));
  check("nothing runs before Allow", window_calls(await page.evaluate(() => window.__calls), "agent_send")?.runs === true && !(await page.evaluate(() => window.__calls)).some((c) => c.cmd === "agent_decide"));
  await card.getByRole("button", { name: "Allow run" }).click();
  await page.getByRole("log", { name: "Messages" }).getByText("All 4 tests pass.").waitFor();
  const dec1 = window_calls(await page.evaluate(() => window.__calls), "agent_decide");
  check("Allow answers the request's own id", dec1?.allow === true && typeof dec1.id === "string" && dec1.id.length > 8, JSON.stringify(dec1));
  check("the chat's runs use the runner chosen in Run and test", window_calls(await page.evaluate(() => window.__calls), "agent_send")?.runner === "local-os");
  check("the chat shows the run and its result", /ran run_tests pytest: exit code 0, tests 4 passed, 0 failed/.test(await page.getByRole("log", { name: "Messages" }).innerText()));
  await page.getByLabel("Message", { exact: true }).fill("again");
  await page.keyboard.press("Control+Enter");
  await card.waitFor();
  await card.getByRole("button", { name: "Decline" }).click();
  await page.getByRole("log", { name: "Messages" }).getByText("OK, not running it.").waitFor();
  check("Decline answers no and the card goes away", window_calls(await page.evaluate(() => window.__calls), "agent_decide")?.allow === false && (await card.count()) === 0);
  await page.screenshot({ path: process.env.SHOT_RUN ?? join(here, "..", "ui-smoke-run.png") });


  // Phase I6a: Extensions view. Search results are text; Install installs a theme; code extensions are refused.
  await page.getByRole("button", { name: "Extensions", exact: true }).click();
  await page.getByLabel("Search Open VSX").fill("theme");
  await page.getByLabel("Search Open VSX").press("Enter");
  await page.getByRole("button", { name: "Install Night Theme" }).waitFor();
  check("extension search results render as text (no markup from Open VSX)", (await page.evaluate(() => window.__xss6)) === undefined && /a dark theme/.test(await page.getByRole("region", { name: "Search results" }).innerText()));
  await page.getByRole("button", { name: "Install Runs Code" }).click();
  await page.getByText(/Node extension host arrives in phase I6c/).first().waitFor();
  check("an extension that runs code is refused with the phase that adds its host", true);
  await page.getByRole("button", { name: "Install Night Theme" }).click();
  await page.getByRole("region", { name: "Installed extensions" }).getByText("Night Theme").waitFor();
  check("Install puts the theme in the installed list with its verified badge", (await page.getByLabel("Verified publisher on Open VSX").count()) === 1);
  await page.getByRole("option", { name: /Smoke Night/ }).waitFor({ state: "attached" });
  await page.getByRole("combobox", { name: "Colour theme" }).selectOption("Smoke Night");
  const bg = async () => page.evaluate(() => getComputedStyle(document.querySelector(".monaco-editor .monaco-editor-background") ?? document.body).backgroundColor);
  await page.waitForFunction(() => getComputedStyle(document.querySelector(".monaco-editor .monaco-editor-background") ?? document.body).backgroundColor === "rgb(18, 52, 86)", null, { timeout: 8000 }).catch(() => {});
  check("picking the installed theme recolours the editor", (await bg()) === "rgb(18, 52, 86)", await bg());
  await page.screenshot({ path: process.env.SHOT_EXT ?? join(here, "..", "ui-smoke-ext.png") });
  // Remove while its theme is in use: the view moves the editor back to Dark Modern first.
  await page.getByRole("button", { name: "Remove Night Theme" }).click();
  await page.getByText("No extensions installed.").waitFor();
  check("Remove takes the extension off the list and out of the theme picker", (await page.getByRole("option", { name: /Smoke Night/ }).count()) === 0);
  check("removing the theme in use goes back to Dark Modern", (await page.getByRole("combobox", { name: "Colour theme" }).inputValue()) === "Dark Modern" && (await bg()) !== "rgb(18, 52, 86)", await bg());

  // Phase I7: pair with TandemForge on this computer. A wrong code is refused; the right one is stored, never shown.
  await page.getByRole("button", { name: "Gateways", exact: true }).click();
  const tf = page.getByRole("region", { name: "TandemForge on this computer" });
  await tf.getByText("Running. Pair the IDE to use its local models.").waitFor();
  check("TandemForge running here is found without typing a port", true);
  await tf.getByRole("button", { name: "Pair with TandemForge" }).click();
  await tf.getByLabel("TandemForge pairing code").fill("tfa_" + "W".repeat(43));
  await tf.getByRole("button", { name: "Pair" }).click();
  await page.getByText(/TandemForge did not accept this code/).first().waitFor();
  check("a code TandemForge does not know is refused, and the field is cleared", (await tf.getByLabel("TandemForge pairing code").inputValue()) === "");
  await tf.getByLabel("TandemForge pairing code").fill("  tfa_" + "Q".repeat(43) + "  ");
  await tf.getByRole("button", { name: "Pair" }).click();
  const tfSaved = page.getByRole("list", { name: "Saved gateways" });
  await tfSaved.getByText("TandemForge (this computer)").waitFor();
  await tf.getByText(/Running and paired/).waitFor();
  const tfRow = tfSaved.locator("li", { hasText: "TandemForge (this computer)" });
  const tfText = await tfRow.innerText();
  check("paired TandemForge is listed and active, code in the keychain, never shown", /paired \(code in keychain\)/.test(tfText) && (await tfRow.getByRole("radio").isChecked()) && !(await page.content()).includes("Q".repeat(43)), tfText.replace(/\s+/g, " "));
  check("the loopback TandemForge entry is not marked INSECURE and has no key or CA buttons", !/INSECURE/.test(tfText) && (await tfRow.getByRole("button", { name: /key|CA/i }).count()) === 0, tfText.replace(/\s+/g, " "));
  await page.screenshot({ path: process.env.SHOT_TF ?? join(here, "..", "ui-smoke-tf.png") });

  await page.screenshot({ path: process.env.SHOT ?? join(here, "..", "ui-smoke.png") });
} catch (e) {
  check("flow completed", false, String(e).split("\n").slice(0, 6).join(" | "));
}

const cspErrors = errors.filter((e) => /Content Security Policy/i.test(e));
check("no CSP violations under the shipped CSP", cspErrors.length === 0, cspErrors.slice(0, 2).join(" | "));
check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));

await browser.close();
server.close();
const failed = checks.filter((c) => !c.ok).length;
console.log(`\n${checks.length - failed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
