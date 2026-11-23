/**
 * missionsView.ts -- the Missions view in the P2P-AI side bar (side track X3).
 *
 * Purpose: the VS Code version of the IDE's Missions view (phase I4, docs/features/missions.md). It links the open
 * folder to a project on the active gateway, sends the folder there, switches Autopilot and Auto Improve, starts and
 * controls missions, decides their approval requests, triages Auto Improve proposals, and brings the work back into
 * the chat's "Changes to review" list (nothing is written until the person accepts each file).
 *
 * Inputs / trust boundary: messages from the view's webview (untrusted: checked by `parseFromMissions`); gateway
 * answers (untrusted: turned into fixed, cleaned view shapes by missions/projects.ts); the open folder (trusted,
 * local folders only, as for the chat tools).
 *
 * Security controls:
 *  - Webview: CSP `default-src 'none'`, nonce script from the extension's media folder, no command URIs; every
 *    gateway string is drawn with `textContent` (CWE-79). The webview never sees the key or the gateway address.
 *  - Missions need a trusted local folder (Workspace Trust), because a push sends the folder's files (CWE-200).
 *  - Approvals: the decision carries the SHA-256 of the arguments this extension fetched and showed, never a digest
 *    sent by the webview, so the person approves exactly what was shown (CWE-345, OWASP LLM06).
 *  - One push, collect or pull at a time; ids pass the id pattern before they go into a URL (CWE-74, CWE-918).
 *  - Audit lines with fixed words only (no paths, goals or ids) in the "P2P-AI" output channel (NIST AU-3,
 *    CWE-117, CWE-532).
 * Dependencies: the `vscode` API, ./gateway/client.ts, ./gateways.ts, ./missions/projects.ts, ./review.ts.
 */
import * as vscode from "vscode";
import { GatewayClient } from "./gateway/client.ts";
import { NetError, modelOk } from "./gateway/checks.ts";
import { AcctError } from "./accounts.ts";
import type { GatewayStore, SavedGateway } from "./gateways.ts";
import {
  acceptance,
  approvalView,
  folderKey,
  idOk,
  missionView,
  projectView,
  proposalView,
  pull,
  push,
  switches,
  syncView,
  type ApprovalView,
  type CheckInput,
} from "./missions/projects.ts";
import type { ChangeReview } from "./review.ts";
import { parseFromMissions, type FromMissions } from "./viewMessages.ts";

const LINKS_KEY = "p2pAi.links";
const MAX_LINKS = 200;

/** Which gateway project belongs to a folder (in the extension's state file; the folder path itself is not stored). */
interface Link {
  folder: string;
  gateway_id: string;
  project_id: string;
  project_name: string;
}

function nonce(): string {
  return Buffer.from(crypto.getRandomValues(new Uint8Array(18))).toString("base64");
}

export class MissionsViewProvider implements vscode.WebviewViewProvider {
  static readonly viewId = "p2pAi.missions";
  private view: vscode.WebviewView | undefined;
  private busy = false;
  /** The approvals last shown, by id: their digests are what a decision sends. */
  private shown = new Map<string, ApprovalView>();
  private timer: ReturnType<typeof setInterval> | undefined;

  private readonly ctx: vscode.ExtensionContext;
  private readonly store: GatewayStore;
  private readonly review: ChangeReview;
  private readonly log: vscode.LogOutputChannel;

  constructor(ctx: vscode.ExtensionContext, store: GatewayStore, review: ChangeReview, log: vscode.LogOutputChannel) {
    this.ctx = ctx;
    this.store = store;
    this.review = review;
    this.log = log;
    store.onDidChange(() => void this.refresh());
    ctx.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(() => void this.refresh()));
  }

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    const media = vscode.Uri.joinPath(this.ctx.extensionUri, "media");
    // SECURITY: scripts only from our media folder, no command URIs (CWE-829).
    view.webview.options = { enableScripts: true, enableCommandUris: false, localResourceRoots: [media] };
    view.webview.html = this.html(view.webview, media);
    view.webview.onDidReceiveMessage((m) => void this.onMessage(parseFromMissions(m)));
    // While the view is visible, missions are refreshed every 10 s (cheap GETs; stops when hidden).
    const tick = (): void => {
      if (this.timer) clearInterval(this.timer);
      this.timer = view.visible ? setInterval(() => void this.refresh(true), 10_000) : undefined;
    };
    view.onDidChangeVisibility(tick);
    tick();
    view.onDidDispose(() => {
      if (this.timer) clearInterval(this.timer);
      this.view = undefined;
    });
  }

  private post(m: Record<string, unknown>): void {
    void this.view?.webview.postMessage(m);
  }

  /** Fixed audit words only (NIST AU-3); VS Code's log adds the timestamp. */
  private audit(what: string, outcome: string): void {
    this.log.info(`audit: ${what} ${outcome}`);
  }

  // ---- links

  private links(): Link[] {
    const raw = this.store.state.get<unknown>(LINKS_KEY);
    if (!Array.isArray(raw)) return [];
    // SECURITY: stored values are re-checked like everything else (CWE-20).
    return raw.filter(
      (l): l is Link =>
        typeof l === "object" && l !== null && /^[0-9a-f]{32}$/.test(l.folder) && idOk(l.gateway_id) && idOk(l.project_id) && typeof l.project_name === "string",
    );
  }

  private async setLink(folder: string, gid: string, project: { id: string; name: string } | undefined): Promise<void> {
    const links = this.links().filter((l) => !(l.folder === folder && l.gateway_id === gid));
    if (project) {
      links.push({ folder, gateway_id: gid, project_id: project.id, project_name: [...project.name].filter((c) => !/[\x00-\x1f\x7f-\x9f]/.test(c)).slice(0, 128).join("") });
    }
    await this.store.state.update(LINKS_KEY, links.slice(-MAX_LINKS));
  }

  /** The gateway, a client for it, the folder and its link (any may be missing). */
  private async context(): Promise<{ gw?: SavedGateway; client?: GatewayClient; folder?: string; link?: Link; block?: string; credError?: string }> {
    const gw = this.store.active();
    const ws = this.review.folder();
    const block = this.review.block();
    const out: { gw?: SavedGateway; client?: GatewayClient; folder?: string; link?: Link; block?: string; credError?: string } = { gw };
    if (block) out.block = block === "untrusted" ? "Missions are off in Restricted Mode. Trust this folder to send it to a gateway." : block === "no_folder" ? "Open a folder to use missions." : "Missions work on local folders only.";
    if (ws) {
      out.folder = folderKey(ws.root);
      if (gw) out.link = this.links().find((l) => l.folder === out.folder && l.gateway_id === gw.id);
    }
    if (gw?.kind === "nexus") {
      out.block = `${gw.name} runs locally on this computer; use Chat for local models (Missions run on a remote P2P-AI gateway).`;
      return out;
    }
    if (gw) {
      try {
        out.client = new GatewayClient(await this.store.target(gw));
      } catch (e) {
        out.client = undefined;
        // Phase A3: an account that cannot give a token says why (a fixed message, CWE-209).
        if (e instanceof AcctError) out.credError = e.message;
      }
    }
    return out;
  }

  /** Sends the view everything it shows. `quiet` skips the error line (for the timer). */
  async refresh(quiet = false): Promise<void> {
    if (!this.view) return;
    const c = await this.context();
    const state: Record<string, unknown> = {
      type: "state",
      gateway: c.gw?.name ?? null,
      folder: c.block ? null : (vscode.workspace.workspaceFolders?.[0]?.name ?? null),
      block: c.block ?? (c.gw ? (c.client ? null : (c.credError ?? "This gateway has no key yet; run P2P-AI: Set Gateway Key.")) : "Add a gateway first (P2P-AI: Add Gateway)."),
      project: c.link ? { id: c.link.project_id, name: c.link.project_name } : null,
      busy: this.busy,
    };
    if (c.client && c.link && !c.block) {
      const pid = c.link.project_id;
      try {
        const [exp, flags] = await Promise.all([c.client.api("GET", "/v1/experimental"), c.client.api("GET", `/v1/projects/${pid}/experimental`)]);
        state.switches = switches(exp, flags);
      } catch (e) {
        state.note = `missions: ${msg(e)}`;
      }
      try {
        state.sync = syncView(await c.client.api("GET", `/v1/projects/${pid}/workspace/sync`));
      } catch {
        state.sync = { state: "off" };
      }
      try {
        const v = (await c.client.api("GET", "/v1/missions")) as { data?: unknown };
        state.missions = (Array.isArray(v?.data) ? v.data : []).slice(0, 200).map(missionView).filter((m) => m && m.project_id === pid);
      } catch (e) {
        if (!quiet) state.note = msg(e);
      }
      try {
        const v = (await c.client.api("GET", `/v1/projects/${pid}/proposals`)) as { data?: unknown };
        state.proposals = (Array.isArray(v?.data) ? v.data : []).slice(0, 200).map(proposalView).filter(Boolean);
      } catch {
        state.proposals = [];
      }
    }
    this.post(state);
  }

  /** Runs one push, collect or pull at a time. */
  private async exclusive(f: () => Promise<void>): Promise<void> {
    if (this.busy) return void this.post({ type: "error", text: "a send, collect or pull is already running" });
    this.busy = true;
    this.post({ type: "busy", busy: true });
    try {
      await f();
    } finally {
      this.busy = false;
      this.post({ type: "busy", busy: false });
    }
  }

  private async onMessage(m: FromMissions | undefined): Promise<void> {
    if (!m) return;
    try {
      await this.handle(m);
    } catch (e) {
      this.post({ type: "error", text: msg(e) });
    }
  }

  /** Handles one checked message. Errors become a fixed line in the view. */
  async handle(m: FromMissions): Promise<unknown> {
    if (m.type === "ready" || m.type === "refresh") return this.refresh();
    const c = await this.context();
    if (c.block) throw new NetError("bad_request", c.block);
    if (!c.gw) throw new NetError("bad_request", "add a gateway first (P2P-AI: Add Gateway)");
    if (!c.client) throw c.credError ? new NetError("bad_request", c.credError) : new NetError("no_key");
    const client = c.client;
    const gid = c.gw.id;
    const folder = c.folder as string;
    if (m.type === "projects") {
      const v = (await client.api("GET", "/v1/projects")) as { data?: unknown };
      const list = (Array.isArray(v?.data) ? v.data : []).slice(0, 500).map(projectView).filter(Boolean);
      this.post({ type: "projects", projects: list });
      return list;
    }
    if (m.type === "models") {
      const { ids } = await client.models();
      this.post({ type: "models", models: ids });
      return ids;
    }
    if (m.type === "create") {
      const name = m.name.trim();
      if ([...name].length > 128 || /[\x00-\x1f\x7f-\x9f]/.test(name)) throw new NetError("bad_request", "a project name is 1 to 128 characters on one line");
      const p = projectView(await client.api("POST", "/v1/projects", { name }));
      if (!p) throw new NetError("bad_answer");
      await this.setLink(folder, gid, { id: p.id, name: p.name });
      this.audit("project", "created");
      return this.refresh();
    }
    if (m.type === "link") {
      await this.setLink(folder, gid, { id: m.id, name: m.name });
      return this.refresh();
    }
    if (m.type === "unlink") {
      await this.setLink(folder, gid, undefined);
      return this.refresh();
    }
    if (!c.link) throw new NetError("bad_request", "pick a gateway project first");
    const pid = c.link.project_id;
    const ws = this.review.folder();
    if (!ws) throw new NetError("bad_request", "open a trusted local folder first");
    switch (m.type) {
      case "push": {
        let report: unknown;
        await this.exclusive(async () => {
          report = await push(client, ws, pid);
          this.audit("folder", "pushed");
          this.post({ type: "pushed", report });
        });
        await this.refresh();
        return report;
      }
      case "collect": {
        await this.exclusive(async () => {
          await client.api("POST", `/v1/projects/${pid}/workspace/sync/delta`, {});
        });
        return this.refresh();
      }
      case "pull": {
        let report: unknown;
        await this.exclusive(async () => {
          report = await pull(client, ws, this.review.pending, pid);
          this.review.notify();
          this.audit("changes", "pulled");
          this.post({ type: "pulled", report });
        });
        // Show the chat's "Changes to review" list, where each file is opened and accepted.
        if ((report as { proposed: number }).proposed > 0) void vscode.commands.executeCommand("p2pAi.chat.focus");
        return report;
      }
      case "switches": {
        await client.api("PUT", `/v1/projects/${pid}/experimental`, { autopilot: m.autopilot, auto_improve: m.autoImprove });
        this.audit("switches", "set");
        return this.refresh();
      }
      case "start": {
        if (!modelOk(m.model)) throw new NetError("bad_request", "pick a model from the list");
        const goal = m.goal.trim();
        if ([...goal].length < 3 || [...goal].length > 1000 || /[\x00-\x09\x0b-\x1f\x7f-\x9f]/.test(goal)) throw new NetError("bad_request", "describe the goal in 3 to 1000 characters");
        const body: Record<string, unknown> = { project_id: pid, kind: m.kind, goal, model: m.model };
        if (m.kind === "autopilot") {
          const a = acceptance(m.checks as CheckInput[]);
          if (!Array.isArray(a)) throw new NetError("bad_request", a.error);
          body.acceptance = a;
        } else if (m.checks.length > 0) throw new NetError("bad_request", "a mission is Autopilot or Auto Improve");
        const v = missionView(await client.api("POST", "/v1/missions", body));
        this.audit("mission", "started");
        await this.refresh();
        return v;
      }
      case "control": {
        await client.api("POST", `/v1/missions/${m.id}/${m.action}`, {});
        this.audit("mission", "control");
        return this.refresh();
      }
      case "approvals": {
        const v = (await client.api("GET", `/v1/agents/runs/${m.runId}/approvals`)) as { data?: unknown };
        const list = (Array.isArray(v?.data) ? v.data : []).slice(0, 16).map(approvalView).filter((a): a is ApprovalView => !!a);
        this.shown = new Map(list.map((a) => [a.id, a]));
        this.post({ type: "approvals", runId: m.runId, approvals: list });
        return list;
      }
      case "decide": {
        // SECURITY: the digest is the one fetched with the approval shown, never one from the webview (CWE-345).
        const a = this.shown.get(m.approvalId);
        if (!a) throw new NetError("bad_request", "open the approval again");
        await client.api("POST", `/v1/agents/runs/${m.runId}/approvals`, { approval_id: a.id, decision: m.decision, arguments_sha256: a.arguments_sha256 });
        this.shown.delete(a.id);
        this.audit("approval", "decided");
        return this.refresh();
      }
      case "proposal": {
        // Snooze is for 7 days (the gateway wants 1 to 90). Nothing a proposal says is run.
        await client.api("POST", `/v1/projects/${pid}/proposals/${m.id}/${m.action}`, m.action === "snooze" ? { days: 7 } : {});
        this.audit("proposal", "triaged");
        return this.refresh();
      }
    }
    return undefined;
  }

  private html(webview: vscode.Webview, media: vscode.Uri): string {
    const n = nonce();
    const css = webview.asWebviewUri(vscode.Uri.joinPath(media, "missions.css"));
    const js = webview.asWebviewUri(vscode.Uri.joinPath(media, "missions.js"));
    // SECURITY: no network, nonce script only, no frames or forms posting anywhere (CWE-79, NIST SC-18).
    const csp = `default-src 'none'; style-src ${webview.cspSource}; script-src 'nonce-${n}'; img-src ${webview.cspSource}; font-src ${webview.cspSource}; base-uri 'none'; form-action 'none'`;
    return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="stylesheet" href="${css}">
<title>P2P-AI Missions</title>
</head>
<body>
<p id="block" hidden></p>
<p id="error" role="alert" hidden></p>
<section id="link">
  <h2>Project</h2>
  <p id="where"></p>
  <div class="row"><button id="projects" type="button">Pick a project</button><button id="unlink" type="button" hidden>Unlink</button></div>
  <select id="projectlist" hidden></select>
  <div class="row"><input id="newname" placeholder="New project name" maxlength="128"><button id="create" type="button">Create project</button></div>
</section>
<section id="linked" hidden>
  <div class="row"><button id="push" type="button">Send folder to the project</button><button id="refresh" type="button">Refresh</button></div>
  <p id="report"></p>
  <ul id="skipped"></ul>
  <p id="sync"></p>
  <div class="row"><button id="collect" type="button">Collect changes</button><button id="pull" type="button">Review changes</button></div>
  <h2>Switches</h2>
  <div class="row"><label><input id="sw-autopilot" type="checkbox"> Autopilot</label><label><input id="sw-improve" type="checkbox"> Auto Improve</label></div>
  <p id="switchnote"></p>
  <h2>Start a mission</h2>
  <div class="row"><select id="kind"><option value="autopilot">Autopilot</option><option value="improve">Auto Improve</option></select><select id="model"><option value="">Pick a model</option></select></div>
  <textarea id="goal" rows="3" maxlength="1000" placeholder="Goal, for example: make the tests in tests/ pass"></textarea>
  <div id="checksbox">
    <p class="hint">Checks (Autopilot): one per line, either a framework (cargo, go, make, npm, pytest, unittest) or a command split by spaces, e.g. <code>python -m pytest -q</code></p>
    <textarea id="checks" rows="2" placeholder="pytest"></textarea>
  </div>
  <div class="row"><button id="start" type="button">Start</button></div>
  <h2>Missions</h2>
  <ul id="missions"></ul>
  <h2>Proposals</h2>
  <ul id="proposals"></ul>
</section>
<script nonce="${n}" src="${js}"></script>
</body>
</html>`;
  }
}

/** A person-facing message for any error (fixed texts only, CWE-209). */
function msg(e: unknown): string {
  return e instanceof NetError ? e.message : "the request failed";
}
