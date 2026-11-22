// i6c3-eval.mjs -- dev check only: evaluates one expression in the running IDE window through the WebView2 debug
// port (127.0.0.1, opened for this one launch) and prints the JSON result. Usage: node i6c3-eval.mjs "<expr>"
const list = await (await fetch("http://127.0.0.1:9229/json/list")).json();
const page = list.find((p) => p.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
ws.onopen = () => ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression: process.argv[2], returnByValue: true, awaitPromise: true } }));
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) { console.log(JSON.stringify(d.result).slice(0, 20000)); process.exit(0); } };
setTimeout(() => process.exit(1), 10000);
