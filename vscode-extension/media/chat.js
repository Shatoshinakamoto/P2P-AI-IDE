// chat.js -- the P2P-AI chat view's webview script (side track X1; tool steps and the change list from X2).
// Purpose: shows the chat and sends what the person types to the extension. It has no network access (CSP
// default-src 'none') and never sees the key, the gateway address or file contents (diffs open in VS Code's own
// diff editor). Change paths it sends back are only looked up among the pending changes by the extension.
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

  function bubble(role, text, ok) {
    const div = document.createElement("div");
    div.className = "msg " + (role === "user" ? "user" : role === "tool" ? "tool" + (ok === false ? " failed" : "") : "assistant");
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
  /** Draws the "Changes to review" list. SECURITY: names via textContent only (CWE-79). */
  function showChanges(list) {
    const ul = $("changelist");
    ul.replaceChildren();
    const items = Array.isArray(list) ? list : [];
    $("changes").hidden = items.length === 0;
    for (const c of items) {
      if (!c || typeof c.rel !== "string") continue;
      const li = document.createElement("li");
      const name = document.createElement("span");
      name.className = "rel";
      name.textContent = c.rel;
      name.title = c.rel;
      const stat = document.createElement("span");
      stat.className = "stat";
      stat.textContent = (c.created ? "new, " : "") + "+" + Number(c.added) + " −" + Number(c.removed);
      li.append(name, stat);
      for (const [label, type] of [["Review", "review"], ["Accept", "accept"], ["Reject", "reject"]]) {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = label;
        b.title = label + " " + c.rel;
        b.addEventListener("click", () => vscode.postMessage({ type, rel: c.rel }));
        li.appendChild(b);
      }
      ul.appendChild(li);
    }
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
        if (Array.isArray(m.messages)) for (const x of m.messages) if (x && typeof x.content === "string") bubble(x.role, x.content, x.ok);
        /** @type {HTMLInputElement} */ ($("tools")).checked = !!m.tools;
        $("toolsnote").textContent = typeof m.toolsNote === "string" ? m.toolsNote : m.tools && typeof m.folder === "string" ? "on " + m.folder : "";
        showChanges(m.changes);
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
      case "tool":
        if (current && current.textContent === "") current.remove();
        bubble("tool", String(m.text), m.ok);
        current = null; // text after a tool step starts a new bubble
        break;
      case "changes":
        showChanges(m.changes);
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
  $("tools").addEventListener("change", (e) => vscode.postMessage({ type: "tools", on: /** @type {HTMLInputElement} */ (e.target).checked }));
  vscode.postMessage({ type: "ready" });
})();
