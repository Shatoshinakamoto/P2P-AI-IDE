// chat.js -- the P2P-AI chat view's webview script (side track X1).
// Purpose: shows the chat and sends what the person types to the extension. It has no network access (CSP
// default-src 'none') and never sees the key or the gateway address.
// Security: every message is shown with textContent, never innerHTML (CWE-79); only the extension's own message
// shapes are handled; anything else is ignored (CWE-20).
// @ts-check
(function () {
  // eslint-disable-next-line no-undef
  const vscode = acquireVsCodeApi();
  const $ = (id) => /** @type {HTMLElement} */ (document.getElementById(id));
  const log = $("log");
  const input = /** @type {HTMLTextAreaElement} */ ($("input"));
  let current = null; // the answer bubble being streamed into

  function bubble(role, text) {
    const div = document.createElement("div");
    div.className = "msg " + (role === "user" ? "user" : "assistant");
    // SECURITY: text only (CWE-79).
    div.textContent = text;
    log.appendChild(div);
    log.scrollTop = log.scrollHeight;
    return div;
  }
  function showError(text) {
    const e = $("error");
    e.textContent = text || "";
    e.hidden = !text;
  }
  function setBusy(busy) {
    $("send").hidden = busy;
    $("stop").hidden = !busy;
    input.disabled = busy;
  }

  window.addEventListener("message", (ev) => {
    const m = ev.data;
    if (!m || typeof m !== "object") return;
    switch (m.type) {
      case "state":
        $("gateway").textContent = typeof m.gateway === "string" ? m.gateway : "Add a gateway";
        $("model").textContent = typeof m.model === "string" ? m.model : "Pick a model";
        $("dev").hidden = !m.dev;
        log.replaceChildren();
        current = null;
        if (Array.isArray(m.messages)) for (const x of m.messages) if (x && typeof x.content === "string") bubble(x.role, x.content);
        setBusy(!!m.busy);
        break;
      case "user":
        showError("");
        bubble("user", String(m.text));
        current = bubble("assistant", "");
        break;
      case "delta":
        if (!current) current = bubble("assistant", "");
        current.textContent += String(m.text);
        log.scrollTop = log.scrollHeight;
        break;
      case "end":
        current = null;
        break;
      case "error":
        if (current && current.textContent === "") current.remove();
        current = null;
        showError(String(m.text));
        break;
      case "busy":
        setBusy(!!m.busy);
        break;
    }
  });

  function send() {
    const text = input.value;
    if (!text.trim()) return;
    input.value = "";
    vscode.postMessage({ type: "send", text });
  }
  $("form").addEventListener("submit", (e) => {
    e.preventDefault();
    send();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      send();
    }
  });
  $("stop").addEventListener("click", () => vscode.postMessage({ type: "stop" }));
  $("new").addEventListener("click", () => vscode.postMessage({ type: "new" }));
  $("gateway").addEventListener("click", () => vscode.postMessage({ type: "pickGateway" }));
  $("model").addEventListener("click", () => vscode.postMessage({ type: "pickModel" }));
  vscode.postMessage({ type: "ready" });
})();
