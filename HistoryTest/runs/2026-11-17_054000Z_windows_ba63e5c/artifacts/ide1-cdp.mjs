// ide1-cdp.mjs -- read console errors and page state of the running P2P-AI IDE window through the WebView2
// debug port (dev check only; the port is opened by an env var for this one launch, on 127.0.0.1 only).
// Usage: node ide1-cdp.mjs  (prints JSON lines). No dependencies (Node 22 global WebSocket and fetch).
const list = await (await fetch("http://127.0.0.1:9229/json/list")).json();
const page = list.find((p) => p.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0;
const send = (method, params = {}) => ws.send(JSON.stringify({ id: ++id, method, params }));
const out = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.method === "Runtime.exceptionThrown") out.push({ exception: d.params.exceptionDetails.exception?.description ?? d.params.exceptionDetails.text, url: d.params.exceptionDetails.url, line: d.params.exceptionDetails.lineNumber });
  else if (d.method === "Runtime.consoleAPICalled") out.push({ console: d.params.type, args: d.params.args.map((a) => a.value ?? a.description).join(" ") });
  else if (d.method === "Log.entryAdded") out.push({ log: d.params.entry.level, text: d.params.entry.text, url: d.params.entry.url });
  else if (d.method === "Network.loadingFailed") out.push({ netfail: d.params.errorText, blocked: d.params.blockedReason });
  else if (d.method === "Network.responseReceived") out.push({ resp: d.params.response.status, url: d.params.response.url, mime: d.params.response.mimeType });
  else if (d.id === 99) out.push({ state: d.result?.result?.value });
};
ws.onopen = () => {
  for (const m of ["Runtime.enable", "Log.enable", "Network.enable", "Page.enable"]) send(m);
  send("Page.reload", { ignoreCache: true });
  setTimeout(() => ws.send(JSON.stringify({ id: 99, method: "Runtime.evaluate", params: { returnByValue: true, expression: "JSON.stringify({ready: document.readyState, root: document.getElementById('root')?.innerHTML.length, body: document.body.innerHTML.slice(0,300), tauri: typeof window.__TAURI_INTERNALS__})" } })), 6000);
  setTimeout(() => { for (const o of out) console.log(JSON.stringify(o)); process.exit(0); }, 7500);
};
