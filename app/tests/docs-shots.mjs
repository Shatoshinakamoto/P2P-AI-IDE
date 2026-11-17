// tests/docs-shots.mjs -- takes the documentation screenshots of the IDE (welcome, editor, search, source control,
// gateways, agent chat, change review, missions, run and test, run approval) for README.md and docs/user-guide.md.
// Run: `npm run build && node tests/docs-shots.mjs [output folder]` (default: ../docs/images).
//
// Purpose: keep the pictures in docs/images/ reproducible. It reuses the fake Tauri bridge of tests/ui-smoke.mjs (the
// in-memory workspace the UI test checks), taken as source text so the two never drift apart, and replaces only the
// workspace files and the agent's answers with a small, friendly sample project.
// Trust boundary: documentation only; nothing here ships in the app. The page is served from dist/ on 127.0.0.1 under
// the app's real CSP, and no request leaves the machine. The sample gateway key below is a placeholder, not a secret.
// Security: ui-smoke's sample data holds hostile strings on purpose (it proves they render as text, CWE-79); `tidy`
// swaps them for plain text in the pictures only.
// Dependencies: playwright (preinstalled in the test environment, not a project dependency), node:http, node:fs.
import { createRequire } from "node:module";
import { createServer } from "node:http";
import { readFile, stat, mkdir } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const { chromium } = createRequire(import.meta.url)("playwright");
const here = fileURLToPath(new URL(".", import.meta.url));
const dist = join(here, "..", "dist");
const out = process.argv[2] ?? join(here, "..", "..", "docs", "images");
await mkdir(out, { recursive: true });
const conf = JSON.parse(await readFile(join(here, "..", "src-tauri", "tauri.conf.json"), "utf8"));
const csp = conf.app.security.csp;

// The fake bridge is the `fake` arrow function of ui-smoke.mjs.
const smoke = await readFile(join(here, "ui-smoke.mjs"), "utf8");
const from = smoke.indexOf("const fake = () => {");
const to = smoke.indexOf("\nconst checks = [];");
if (from < 0 || to < 0) throw new Error("ui-smoke.mjs changed shape: const fake = () => {...} not found");
const fakeSource = smoke.slice(from + "const fake = ".length, to).trim().replace(/;$/, "");

const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".ttf": "font/ttf", ".svg": "image/svg+xml", ".png": "image/png" };
const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://127.0.0.1");
  const file = normalize(join(dist, url.pathname === "/" ? "index.html" : decodeURIComponent(url.pathname)));
  // SECURITY (test server): only files under dist/ are served, on loopback only.
  if (!file.startsWith(dist)) return res.writeHead(403).end();
  try {
    if (!(await stat(file)).isFile()) throw new Error("not a file");
    res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream", "content-security-policy": csp });
    res.end(await readFile(file));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const port = server.address().port;

/** Runs in the page after the fake: a friendlier sample project and agent, and plain text for ui-smoke's hostile strings. */
function tidy() {
  const files = new Map([
    ["src/server.py", "\"\"\"Shop API: a tiny HTTP service for orders.\"\"\"\nimport os\n\nfrom orders import list_orders\n\nREQUEST_TIMEOUT = 5  # seconds\n\n\ndef fetch(url: str) -> bytes:\n    \"\"\"Fetch a URL with the shop's timeout.\"\"\"\n    import urllib.request\n    with urllib.request.urlopen(url, timeout=REQUEST_TIMEOUT) as r:\n        return r.read()\n\n\ndef handle(path: str) -> dict:\n    if path == \"/orders\":\n        return {\"orders\": list_orders()}\n    return {\"error\": \"not found\"}\n"],
    ["src/orders.py", "\"\"\"Orders kept in memory.\"\"\"\n\nORDERS = [{\"id\": 1, \"item\": \"tea\", \"qty\": 2}]\n\n\ndef list_orders() -> list:\n    return list(ORDERS)\n"],
    ["tests/test_orders.py", "from src.orders import list_orders\n\n\ndef test_list_orders():\n    assert list_orders()[0][\"item\"] == \"tea\"\n"],
    ["README.md", "# Shop API\n\nA tiny example service. Run `python -m src.server`.\n\nTimeouts: REQUEST_TIMEOUT in src/server.py.\n"],
    ["requirements.txt", "pytest==8.3.4\n"],
  ]);
  const sha = async (t) => [...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(t)))].map((b) => b.toString(16).padStart(2, "0")).join("");
  const list = (rel) => {
    const prefix = rel === "" ? "" : rel + "/";
    const names = new Map();
    for (const p of files.keys()) {
      if (!p.startsWith(prefix)) continue;
      const rest = p.slice(prefix.length);
      const i = rest.indexOf("/");
      names.set(i >= 0 ? rest.slice(0, i) : rest, i >= 0 ? "dir" : "file");
    }
    const entries = [...names].map(([name, kind]) => ({ name, rel: prefix + name, kind, size: kind === "file" ? files.get(prefix + name).length : 0 }));
    entries.sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === "dir" ? -1 : 1));
    return { entries, truncated: false };
  };
  const pending = new Map();
  const clean = (v) => {
    if (typeof v === "string") {
      if (!v.includes("<")) return v;
      if (v === "<b>bold</b> idea") return "Run the tests on every push, so a broken change is caught before review.";
      return v.replace(/<script>.*?<\/script>/g, "").replace(/<img[^>]*>/g, "").replace(/<[^>]*>/g, "");
    }
    if (Array.isArray(v)) return v.filter((x) => !(typeof x === "string" && /^<img/.test(x))).map(clean);
    if (v && typeof v === "object") return Object.fromEntries(Object.entries(v).filter(([, x]) => !(typeof x === "object" && x && typeof x.rel === "string" && x.rel.startsWith("<img"))).map(([k, x]) => [k, clean(x)]));
    return v;
  };
  const core = window.__TAURI_INTERNALS__;
  const invoke = core.invoke.bind(core);
  const callbacks = new Map();
  const transform = core.transformCallback.bind(core);
  core.transformCallback = (fn) => { const id = transform((p) => fn(clean(p))); callbacks.set(id, (p) => fn(clean(p))); return id; };
  core.invoke = async (cmd, args) => {
    switch (cmd) {
      case "list_dir": return list(args.rel);
      case "read_file": { const t = files.get(args.rel); if (t === undefined) throw "file or folder not found"; return { text: t, version: await sha(t) }; }
      case "write_file": files.set(args.rel, args.text); return sha(args.text);
      case "search": {
        const q = args.query.toLowerCase(); const matches = [];
        for (const [rel, t] of files) t.split("\n").forEach((l, i) => { if (l.toLowerCase().includes(q)) matches.push({ rel, line: i + 1, preview: l.trim() }); });
        return { matches, truncated: false };
      }
      case "scm_status": {
        await invoke(cmd, args); // ui-smoke's first answer is a refusal; take it here so the picture shows the list
        const r = await invoke(cmd, args);
        return clean({ ...r, status: r.status && { ...r.status, files: [
          { rel: "src/server.py", index: ".", work: "M", conflict: false, untracked: false, actionable: true },
          { rel: "src/orders.py", index: "M", work: ".", conflict: false, untracked: false, actionable: true },
          { rel: "tests/test_timeout.py", index: ".", work: "?", conflict: false, untracked: true, actionable: true }] } });
      }
      case "scm_log": return [{ id: "4f2a9c1", subject: "Add the orders endpoint", author: "You", time: 1794800000 }, { id: "a71be03", subject: "First commit", author: "You", time: 1794700000 }];
      case "agent_send": {
        if (args.runs) return invoke(cmd, args);
        const cb = callbacks.get(args.onEvent.id);
        const evs = [
          { kind: "tool", name: "read_file", target: "src/server.py", ok: true },
          { kind: "tool", name: "search", target: "REQUEST_TIMEOUT", ok: true },
          { kind: "delta", text: "The timeout is fixed at 5 seconds in `src/server.py`. I suggest reading it from the `SHOP_TIMEOUT` environment variable, with 5 as the default:\n\n```python\nREQUEST_TIMEOUT = float(os.environ.get(\"SHOP_TIMEOUT\", \"5\"))\n```\n\nThe change is below for you to review. Nothing is written until you accept it." },
        ];
        evs.forEach((message, index) => cb({ index, message }));
        cb({ index: evs.length, end: true });
        const before = files.get("src/server.py");
        pending.set("src/server.py", { before, after: before.replace("REQUEST_TIMEOUT = 5  # seconds", "# Seconds; set SHOP_TIMEOUT to change it.\nREQUEST_TIMEOUT = float(os.environ.get(\"SHOP_TIMEOUT\", \"5\"))"), created: false });
        return { steps: 3, tool_calls: 2, pending: pending.size, finish_reason: "stop" };
      }
      case "changes_list": return [...pending].map(([rel, c]) => ({ rel, created: c.created, added: 2, removed: 1 }));
      case "change_get": { const c = pending.get(args.rel); if (!c) throw "that change is no longer pending"; return { rel: args.rel, ...c }; }
      case "change_accept": { const c = pending.get(args.rel); files.set(args.rel, c.after); pending.delete(args.rel); return sha(c.after); }
      case "change_reject": pending.delete(args.rel); return null;
      default: return clean(await invoke(cmd, args));
    }
  };
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1400, height: 860 }, deviceScaleFactor: 1, colorScheme: "dark" });
await page.addInitScript(`(${fakeSource})();`);
await page.addInitScript(tidy);
if (conf.app?.security?.freezePrototype) await page.addInitScript(() => Object.freeze(Object.prototype));
await page.goto(`http://127.0.0.1:${port}/`);
const taken = [];
const shot = async (name) => { await page.waitForTimeout(600); await page.screenshot({ path: join(out, `${name}.png`) }); taken.push(`${name}.png`); };
const files = page.getByRole("navigation", { name: "Files" });

await page.getByText("No folder is open.").waitFor({ timeout: 10000 });
await shot("welcome");
await page.getByRole("button", { name: "Open folder" }).first().click();
await files.getByText("README.md").waitFor();
await files.getByText("src").click();
await files.getByText("orders.py").click();
await files.getByText("server.py").click();
await page.locator(".monaco-editor .view-lines").getByText("REQUEST_TIMEOUT").first().waitFor({ timeout: 10000 });
await shot("editor");

await page.getByRole("button", { name: "Search" }).click();
await page.getByLabel("Search in workspace (plain text)").fill("timeout");
await page.keyboard.press("Enter");
await page.getByText(/results? in \d+ files?/).waitFor();
await shot("search");

await page.getByRole("button", { name: "Source control", exact: true }).click();
await page.getByRole("region", { name: "Changes", exact: true }).getByText("src/server.py").waitFor();
await page.getByLabel("Commit message").fill("Read the request timeout from SHOP_TIMEOUT");
await shot("source-control");

await page.getByRole("button", { name: "Agent chat", exact: true }).click();
await page.getByRole("button", { name: "Open Gateways" }).click();
await page.getByRole("button", { name: "Add gateway" }).click();
await page.getByLabel("Gateway name").fill("Home");
await page.getByLabel("Gateway address").fill("https://gateway.example.org");
await page.getByLabel("Gateway key").fill("p2p_example_placeholder_key");
await page.getByRole("button", { name: "Add by address" }).click();
await page.getByRole("list", { name: "Saved gateways" }).getByText("key in keychain").waitFor();
await page.getByRole("button", { name: "Check" }).click();
await page.getByText("12 ms, 2 models").waitFor();
await page.getByLabel("Directory address").fill("https://directory.example.org/");
await page.getByRole("button", { name: "Add directory" }).click();
await page.getByRole("button", { name: "Refresh list" }).click();
await page.getByRole("list", { name: "Public gateway list" }).getByText("41 ms").waitFor();
await shot("gateways");

await page.getByRole("button", { name: "Agent chat", exact: true }).click();
await page.getByLabel("Model").locator("option", { hasText: "tiny-model" }).waitFor({ state: "attached" });
await page.getByLabel("Message", { exact: true }).fill("Can the request timeout be set without editing the code?");
await page.keyboard.press("Control+Enter");
await page.getByRole("list", { name: "Pending changes" }).getByText("src/server.py").waitFor();
await shot("agent-chat");
await page.getByRole("button", { name: "Review src/server.py" }).click();
const dlg = page.getByRole("dialog", { name: "Review change to src/server.py" });
await dlg.locator(".monaco-diff-editor").waitFor();
await shot("review-change");
await dlg.getByRole("button", { name: "Accept and save" }).click();
await dlg.waitFor({ state: "detached" });

await page.getByRole("button", { name: "Missions: Autopilot and Auto Improve", exact: true }).click();
await page.getByRole("button", { name: "Create project and link" }).click();
await page.getByRole("button", { name: /Send folder to the project/ }).click();
await page.getByLabel("Last send").waitFor();
await page.getByRole("checkbox", { name: "Autopilot" }).check();
await page.getByRole("button", { name: /Autopilot on/ }).waitFor();
await page.getByLabel("Mission goal").fill("Add a test for the SHOP_TIMEOUT setting and make every test pass");
await page.getByRole("button", { name: "Start Autopilot" }).click();
await page.getByRole("button", { name: "Review approval" }).waitFor();
await shot("missions");

await page.getByRole("button", { name: "Run and test", exact: true }).click();
await page.getByRole("button", { name: "Run pytest tests" }).click();
await page.getByText(/3 passed/).first().waitFor().catch(() => {});
await shot("run-and-test");

await page.getByRole("button", { name: "Agent chat", exact: true }).click();
await page.getByRole("button", { name: "New chat" }).click();
const runsBtn = page.getByRole("button", { name: "Runs", exact: true });
if ((await runsBtn.getAttribute("aria-pressed")) !== "true") await runsBtn.click();
await page.getByLabel("Message", { exact: true }).fill("Run the tests, please");
await page.keyboard.press("Control+Enter");
await page.getByRole("alertdialog", { name: "Run request" }).waitFor();
await shot("run-approval");

await browser.close();
server.close();
console.log(`${taken.length} screenshots in ${out}:\n${taken.join("\n")}`);
