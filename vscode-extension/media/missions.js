// missions.js -- the P2P-AI Missions view's webview script (side track X3).
// Purpose: shows the linked project, its switches, missions, approvals and proposals, and sends the person's
// clicks to the extension. It has no network access (CSP default-src 'none') and never sees the key, the gateway
// address or file contents.
// Security: every gateway string is drawn with textContent, never innerHTML (CWE-79); approval arguments are shown
// verbatim in a <pre> and never run; proposal sources are plain text, not links; only the extension's own message
// shapes are handled (CWE-20).
// @ts-check
(function () {
  // eslint-disable-next-line no-undef
  const vscode = acquireVsCodeApi();
  const $ = (id) => /** @type {HTMLElement} */ (document.getElementById(id));
  const post = (m) => vscode.postMessage(m);
  /** @param {string} tag @param {string} [text] @param {string} [cls] */
  function el(tag, text, cls) {
    const e = document.createElement(tag);
    if (text !== undefined) e.textContent = text; // SECURITY: text only (CWE-79)
    if (cls) e.className = cls;
    return e;
  }
  /** @param {string} label @param {() => void} on */
  function button(label, on) {
    const b = /** @type {HTMLButtonElement} */ (el("button", label));
    b.type = "button";
    b.addEventListener("click", on);
    return b;
  }
  function showError(text) {
    $("error").textContent = text || "";
    $("error").hidden = !text;
  }
  let stopArmed = ""; // a mission id whose Stop was pressed once

  function drawState(m) {
    showError(m.note || "");
    $("block").textContent = m.block || "";
    $("block").hidden = !m.block;
    $("link").hidden = !!m.block;
    const p = m.project && typeof m.project.id === "string" ? m.project : null;
    $("where").textContent = p ? "Linked to " + String(p.name || p.id) + " on " + String(m.gateway) : m.folder ? "This folder is not linked to a project on " + String(m.gateway) + "." : "";
    $("unlink").hidden = !p;
    $("linked").hidden = !p || !!m.block;
    setBusy(!!m.busy);
    const sw = m.switches;
    /** @type {HTMLInputElement} */ ($("sw-autopilot")).checked = !!(sw && sw.autopilot);
    /** @type {HTMLInputElement} */ ($("sw-improve")).checked = !!(sw && sw.auto_improve);
    $("switchnote").textContent = !sw ? "" : sw.disabled ? "The gateway's administrator has switched missions off." : !sw.autopilot_available ? "This gateway does not offer Autopilot." : "";
    const s = m.sync;
    $("sync").textContent = !s ? "" : s.state === "off" ? "Node copy: off on this gateway." : "Node copy: " + String(s.state || "none") + (s.delta_state ? "; last collect: " + String(s.delta_state) + " (+" + Number(s.delta_added) + " ~" + Number(s.delta_modified) + " −" + Number(s.delta_deleted) + ")" : "");
    drawMissions(Array.isArray(m.missions) ? m.missions : []);
    drawProposals(Array.isArray(m.proposals) ? m.proposals : []);
  }

  function drawMissions(list) {
    const ul = $("missions");
    ul.replaceChildren();
    if (list.length === 0) ul.appendChild(el("li", "No missions yet.", "muted"));
    for (const x of list) {
      if (!x || typeof x.id !== "string") continue;
      const li = el("li", undefined, "card");
      li.appendChild(el("div", (x.kind === "improve" ? "Auto Improve" : "Autopilot") + " · " + String(x.state) + (x.max_cycles ? " · cycle " + Number(x.cycle) + "/" + Number(x.max_cycles) : ""), "title"));
      li.appendChild(el("div", String(x.goal)));
      if (x.reason) li.appendChild(el("div", String(x.reason), "muted"));
      if (x.checks_total) li.appendChild(el("div", "Checks " + Number(x.checks_passed) + "/" + Number(x.checks_total) + (x.tasks_total ? ", tasks " + Number(x.tasks_done) + "/" + Number(x.tasks_total) : ""), "muted"));
      const row = el("div", undefined, "row");
      if (x.waiting_approval && x.run_id) row.appendChild(button("Review approval", () => post({ type: "approvals", runId: x.run_id })));
      if (x.state === "running") row.appendChild(button("Pause", () => post({ type: "control", id: x.id, action: "pause" })));
      if (x.state === "paused") row.appendChild(button("Resume", () => post({ type: "control", id: x.id, action: "resume" })));
      if (x.state === "running" || x.state === "paused" || x.state === "waiting") {
        row.appendChild(
          button(stopArmed === x.id ? "Stop for good" : "Stop", () => {
            if (stopArmed === x.id) {
              stopArmed = "";
              post({ type: "control", id: x.id, action: "stop" });
            } else {
              stopArmed = x.id;
              post({ type: "refresh" });
            }
          }),
        );
      }
      li.appendChild(row);
      const box = el("div");
      box.id = "approvals-" + x.run_id;
      li.appendChild(box);
      ul.appendChild(li);
    }
  }

  function drawApprovals(runId, list) {
    const box = document.getElementById("approvals-" + runId);
    if (!box) return;
    box.replaceChildren();
    if (list.length === 0) box.appendChild(el("p", "Nothing is waiting for a decision.", "muted"));
    for (const a of list) {
      const d = el("div", undefined, "approval");
      d.appendChild(el("div", String(a.tool) + " (" + String(a.access) + ")", "title"));
      // SECURITY: the exact arguments, as text, never run here (OWASP LLM06).
      d.appendChild(el("pre", String(a.arguments)));
      const row = el("div", undefined, "row");
      row.appendChild(button("Approve", () => post({ type: "decide", runId, approvalId: a.id, decision: "approve" })));
      row.appendChild(button("Reject", () => post({ type: "decide", runId, approvalId: a.id, decision: "reject" })));
      d.appendChild(row);
      box.appendChild(d);
    }
  }

  function drawProposals(list) {
    const ul = $("proposals");
    ul.replaceChildren();
    if (list.length === 0) ul.appendChild(el("li", "No proposals.", "muted"));
    for (const p of list) {
      if (!p || typeof p.id !== "string") continue;
      const li = el("li", undefined, "card");
      li.appendChild(el("div", String(p.title) + (p.effort ? " · " + String(p.effort) : ""), "title"));
      if (p.rationale) li.appendChild(el("div", String(p.rationale)));
      if (Array.isArray(p.steps) && p.steps.length) {
        const ol = el("ol");
        for (const s of p.steps) ol.appendChild(el("li", String(s)));
        li.appendChild(ol);
      }
      if (Array.isArray(p.sources) && p.sources.length) li.appendChild(el("div", "Sources: " + p.sources.map(String).join(", "), "muted"));
      const row = el("div", undefined, "row");
      for (const [label, action] of [["Accept", "accept"], ["Dismiss", "dismiss"], ["Snooze 7 days", "snooze"]]) row.appendChild(button(label, () => post({ type: "proposal", id: p.id, action })));
      li.appendChild(row);
      ul.appendChild(li);
    }
  }

  function setBusy(b) {
    for (const id of ["push", "collect", "pull"]) /** @type {HTMLButtonElement} */ ($(id)).disabled = b;
  }

  function drawReport(text, skipped) {
    $("report").textContent = text;
    const ul = $("skipped");
    ul.replaceChildren();
    for (const s of Array.isArray(skipped) ? skipped.slice(0, 50) : []) ul.appendChild(el("li", String(s.rel) + ": " + String(s.reason), "muted"));
  }

  window.addEventListener("message", (ev) => {
    const m = ev.data;
    if (!m || typeof m !== "object") return;
    switch (m.type) {
      case "state":
        drawState(m);
        break;
      case "error":
        showError(String(m.text));
        break;
      case "busy":
        setBusy(!!m.busy);
        break;
      case "projects": {
        const sel = /** @type {HTMLSelectElement} */ ($("projectlist"));
        sel.replaceChildren(el("option", "Pick a project…"));
        for (const p of Array.isArray(m.projects) ? m.projects : []) {
          const o = /** @type {HTMLOptionElement} */ (el("option", String(p.name || p.id) + (p.role ? " (" + String(p.role) + ")" : "")));
          o.value = String(p.id);
          o.dataset.name = String(p.name || "");
          sel.appendChild(o);
        }
        sel.hidden = false;
        break;
      }
      case "models": {
        const sel = /** @type {HTMLSelectElement} */ ($("model"));
        sel.replaceChildren(el("option", "Pick a model"));
        for (const id of Array.isArray(m.models) ? m.models : []) {
          const o = /** @type {HTMLOptionElement} */ (el("option", String(id)));
          o.value = String(id);
          sel.appendChild(o);
        }
        break;
      }
      case "pushed": {
        const r = m.report || {};
        drawReport("Sent " + Number(r.uploaded) + ", deleted " + Number(r.deleted) + ", unchanged " + Number(r.unchanged) + "; version " + Number(r.version) + "; node copy: " + String(r.copy), r.skipped);
        break;
      }
      case "pulled": {
        const r = m.report || {};
        drawReport(Number(r.proposed) + " file(s) wait under Changes to review in the chat; " + Number(r.same) + " unchanged, " + Number(r.onlyLocal) + " only on this computer (kept).", r.skipped);
        break;
      }
      case "approvals":
        drawApprovals(String(m.runId), Array.isArray(m.approvals) ? m.approvals : []);
        break;
    }
  });

  $("projects").addEventListener("click", () => post({ type: "projects" }));
  $("projectlist").addEventListener("change", (e) => {
    const sel = /** @type {HTMLSelectElement} */ (e.target);
    const o = sel.selectedOptions[0];
    if (o && o.value) post({ type: "link", id: o.value, name: o.dataset.name || "" });
    sel.hidden = true;
  });
  $("create").addEventListener("click", () => {
    const name = /** @type {HTMLInputElement} */ ($("newname")).value.trim();
    if (name) post({ type: "create", name });
  });
  $("unlink").addEventListener("click", () => post({ type: "unlink" }));
  $("push").addEventListener("click", () => post({ type: "push" }));
  $("refresh").addEventListener("click", () => post({ type: "refresh" }));
  $("collect").addEventListener("click", () => post({ type: "collect" }));
  $("pull").addEventListener("click", () => post({ type: "pull" }));
  const sw = () => post({ type: "switches", autopilot: /** @type {HTMLInputElement} */ ($("sw-autopilot")).checked, autoImprove: /** @type {HTMLInputElement} */ ($("sw-improve")).checked });
  $("sw-autopilot").addEventListener("change", sw);
  $("sw-improve").addEventListener("change", sw);
  $("kind").addEventListener("change", () => {
    $("checksbox").hidden = /** @type {HTMLSelectElement} */ ($("kind")).value !== "autopilot";
  });
  $("model").addEventListener("focus", () => {
    if (/** @type {HTMLSelectElement} */ ($("model")).options.length <= 1) post({ type: "models" });
  });
  const FRAMEWORKS = ["cargo", "go", "make", "npm", "pytest", "unittest"];
  $("start").addEventListener("click", () => {
    const kind = /** @type {HTMLSelectElement} */ ($("kind")).value;
    const lines = kind === "autopilot" ? /** @type {HTMLTextAreaElement} */ ($("checks")).value.split("\n").map((l) => l.trim()).filter(Boolean) : [];
    // One check per line: a framework name, or a command split on spaces (never run through a shell).
    const checks = lines.slice(0, 8).map((l, i) => (FRAMEWORKS.includes(l) ? { id: "check-" + (i + 1), kind: "tests", framework: l } : { id: "check-" + (i + 1), kind: "command", argv: l.split(/\s+/) }));
    post({ type: "start", kind, goal: /** @type {HTMLTextAreaElement} */ ($("goal")).value, model: /** @type {HTMLSelectElement} */ ($("model")).value, checks });
  });
  post({ type: "ready" });
})();
