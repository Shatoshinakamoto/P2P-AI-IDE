/**
 * chatView.ts -- the P2P-AI chat view in VS Code's side bar (side track X1, tools X2).
 *
 * Purpose: a chat with the active gateway's models, like the IDE's chat panel: pick a gateway and a model, type,
 * watch the answer stream in, stop it, start a new chat. With **Tools** on and a trusted local folder open (X2), the
 * model can look at the folder and propose edits through the agent tools (./agent/tools.ts); proposals are listed
 * under "Changes to review" and only written when the person accepts them (./review.ts).
 * Inputs / trust boundary: messages posted by the view's webview (untrusted: allow-listed types, capped text);
 * gateway answers (untrusted: plain text only). Output: requests through GatewayClient; text posted to the webview.
 * Security controls:
 *  - The webview has no network and no Node: CSP `default-src 'none'`, scripts only with a per-load nonce, styles
 *    only from the extension's own media folder, `localResourceRoots` limited to that folder (CWE-79, CWE-829).
 *  - The webview renders every message with `textContent`, never as HTML (CWE-79).
 *  - The webview never sees the key or the gateway's address; it gets the gateway's name and a dev flag only.
 *  - Messages from the webview are checked by type and size before use (CWE-20, CWE-400).
 *  - Tools are off in Restricted Mode and for remote or virtual folders; every tool step is shown in the chat; the
 *    agent never writes (OWASP LLM01, LLM06; see review.ts).
 * Dependencies: the `vscode` API, ./gateway/client.ts, ./gateways.ts, ./agent/tools.ts, ./review.ts.
 */
import * as vscode from "vscode";
import { GatewayClient } from "./gateway/client.ts";
import { NetError, modelOk, type Msg } from "./gateway/checks.ts";
import type { GatewayStore, SavedGateway } from "./gateways.ts";
import { parseFromView, type FromView } from "./viewMessages.ts";
import { runTurn, toolDefs } from "./agent/tools.ts";
import type { ChangeReview } from "./review.ts";

const MODEL_KEY = (id: string): string => `p2pAi.model.${id}`;
const TOOLS_KEY = "p2pAi.tools";

/** What the view says when the tools can't be used. */
const BLOCK_TEXT = {
  untrusted: "Tools are off in Restricted Mode. Trust this folder to let the assistant read it.",
  no_folder: "Open a folder to let the assistant read it and propose changes.",
  not_local: "Tools work on local folders only.",
} as const;

/** A random nonce for the webview's script tag (CSPRNG). */
function nonce(): string {
  return Buffer.from(crypto.getRandomValues(new Uint8Array(18))).toString("base64");
}

export class ChatViewProvider implements vscode.WebviewViewProvider {
  static readonly viewId = "p2pAi.chat";
  private view: vscode.WebviewView | undefined;
  private messages: Msg[] = [];
  /** What the view shows: the messages plus tool steps, in order (tool steps are never sent to the model). */
  private display: { role: "user" | "assistant" | "tool"; content: string; ok?: boolean }[] = [];
  private turn: AbortController | undefined;

  private readonly ctx: vscode.ExtensionContext;
  private readonly store: GatewayStore;
  private readonly review: ChangeReview;

  constructor(ctx: vscode.ExtensionContext, store: GatewayStore, review: ChangeReview) {
    this.ctx = ctx;
    this.store = store;
    this.review = review;
    store.onDidChange(() => this.postState());
    review.onDidChangeList(() => this.post({ type: "changes", changes: review.views() }));
  }

  /** The person's Tools switch (on by default). */
  toolsOn(): boolean {
    return this.store.state.get<boolean>(TOOLS_KEY) !== false;
  }

  async setTools(on: boolean): Promise<void> {
    await this.store.state.update(TOOLS_KEY, on);
    this.postState();
  }

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    const media = vscode.Uri.joinPath(this.ctx.extensionUri, "media");
    // SECURITY: scripts on, but only from our media folder, and no command URIs (CWE-829).
    view.webview.options = { enableScripts: true, enableCommandUris: false, localResourceRoots: [media] };
    view.webview.html = this.html(view.webview, media);
    view.webview.onDidReceiveMessage((m) => this.onMessage(parseFromView(m)));
    view.onDidDispose(() => {
      this.turn?.abort();
      this.view = undefined;
    });
  }

  /** Starts a new chat (also the `P2P-AI: New Chat` command). */
  newChat(): void {
    this.turn?.abort();
    this.messages = [];
    this.display = [];
    this.postState();
  }

  /** Lets the person pick a model of the active gateway (also a command). */
  async pickModel(): Promise<void> {
    const gw = this.store.active();
    if (!gw) return void vscode.window.showWarningMessage("Add a gateway first (P2P-AI: Add Gateway).");
    try {
      const client = new GatewayClient({ url: gw.url, key: await this.store.key(gw.id), caPem: gw.caPem });
      const { ids, ms } = await client.models();
      if (ids.length === 0) return void vscode.window.showWarningMessage("This gateway lists no models.");
      const pick = await vscode.window.showQuickPick(ids, { title: `Models on ${gw.name} (${ms} ms)`, placeHolder: "Pick a model" });
      if (pick && modelOk(pick)) {
        await this.store.state.update(MODEL_KEY(gw.id), pick);
        this.postState();
      }
    } catch (e) {
      vscode.window.showErrorMessage(e instanceof NetError ? e.message : "could not list the models");
    }
  }

  private model(gw: SavedGateway | undefined): string | undefined {
    if (!gw) return undefined;
    const m = this.store.state.get<string>(MODEL_KEY(gw.id));
    return typeof m === "string" && modelOk(m) ? m : undefined;
  }

  private async onMessage(m: FromView | undefined): Promise<void> {
    if (!m) return;
    switch (m.type) {
      case "ready":
        return this.postState();
      case "stop":
        return this.turn?.abort();
      case "new":
        return this.newChat();
      case "pickGateway":
        return void vscode.commands.executeCommand("p2pAi.pickGateway");
      case "pickModel":
        return this.pickModel();
      case "send":
        return void (await this.send(m.text));
      case "tools":
        return this.setTools(m.on);
      case "review":
        return void (await this.review.open(m.rel));
      case "accept": {
        const err = this.review.accept(m.rel);
        if (err) this.post({ type: "error", text: err });
        return;
      }
      case "reject":
        return void this.review.reject(m.rel);
    }
  }

  /** Sets the active gateway's model without a picker (used by the test-mode hooks only). */
  async setModel(model: string): Promise<boolean> {
    const gw = this.store.active();
    if (!gw || !modelOk(model)) return false;
    await this.store.state.update(MODEL_KEY(gw.id), model);
    this.postState();
    return true;
  }

  /** Runs one chat turn. Returns the answer text, or the error message shown. */
  async send(text: string): Promise<{ answer?: string; error?: string }> {
    if (this.turn) return { error: "busy" }; // one turn at a time
    const gw = this.store.active();
    const model = this.model(gw);
    if (!gw || !model) {
      const error = gw ? "pick a model first" : "add a gateway first (P2P-AI: Add Gateway)";
      this.post({ type: "error", text: error });
      return { error };
    }
    this.messages.push({ role: "user", content: text });
    this.display.push({ role: "user", content: text });
    const answer: Msg = { role: "assistant", content: "" };
    this.post({ type: "user", text });
    const turn = new AbortController();
    this.turn = turn;
    this.post({ type: "busy", busy: true });
    try {
      const client = new GatewayClient({ url: gw.url, key: await this.store.key(gw.id), caPem: gw.caPem });
      const onText = (t: string): void => {
        answer.content += t;
        const last = this.display[this.display.length - 1];
        if (last?.role === "assistant") last.content += t;
        else this.display.push({ role: "assistant", content: t });
        this.post({ type: "delta", text: t });
      };
      const ws = this.toolsOn() ? this.review.folder() : undefined;
      let finish: string | null;
      if (ws) {
        // X2: an agent turn. Tool steps are shown as they happen; proposals go to the review list, never to disk.
        const end = await runTurn(
          ws,
          this.review.pending,
          this.messages,
          (msgs, onDelta) => client.agentStep(model, msgs, toolDefs(), onDelta, turn.signal),
          (ev) => {
            if (ev.kind === "delta") onText(ev.text);
            else {
              const line = ev.target ? `${ev.name} ${ev.target}` : ev.name;
              this.display.push({ role: "tool", content: line, ok: ev.ok });
              this.post({ type: "tool", text: line, ok: ev.ok });
              if (ev.name === "edit_file" || ev.name === "create_file") this.review.notify();
            }
          },
          turn.signal,
        );
        finish = end.finishReason;
      } else {
        finish = (await client.chatStream(model, this.messages, onText, turn.signal)).finishReason;
      }
      this.messages.push(answer);
      this.post({ type: "end", finish });
      return { answer: answer.content };
    } catch (e) {
      // A stopped or failed turn keeps what arrived, so the chat can go on.
      if (answer.content) this.messages.push(answer);
      else this.messages.pop();
      const error = e instanceof NetError ? e.message : "the request failed";
      this.post({ type: "error", text: error });
      return { error };
    } finally {
      this.turn = undefined;
      this.post({ type: "busy", busy: false });
    }
  }

  private postState(): void {
    const gw = this.store.active();
    this.post({
      type: "state",
      gateway: gw ? gw.name : null,
      // INSECURE — DEV ONLY marker for plain http on loopback, shown in the view.
      dev: gw ? gw.url.startsWith("http://") : false,
      model: this.model(gw) ?? null,
      messages: this.display,
      busy: !!this.turn,
      tools: this.toolsOn(),
      // Why the tools can't run here (shown under the switch), or null.
      toolsNote: (() => {
        const b = this.review.block();
        return b ? BLOCK_TEXT[b] : null;
      })(),
      folder: this.review.block() === undefined ? (vscode.workspace.workspaceFolders?.[0]?.name ?? null) : null,
      changes: this.review.views(),
    });
  }

  private post(m: Record<string, unknown>): void {
    void this.view?.webview.postMessage(m);
  }

  private html(webview: vscode.Webview, media: vscode.Uri): string {
    const n = nonce();
    const css = webview.asWebviewUri(vscode.Uri.joinPath(media, "chat.css"));
    const js = webview.asWebviewUri(vscode.Uri.joinPath(media, "chat.js"));
    // SECURITY: no network, no inline script without the nonce, no eval, no frames (CWE-79, NIST SC-18).
    const csp = `default-src 'none'; style-src ${webview.cspSource}; script-src 'nonce-${n}'; img-src ${webview.cspSource}; font-src ${webview.cspSource}; base-uri 'none'; form-action 'none'`;
    return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="stylesheet" href="${css}">
<title>P2P-AI Chat</title>
</head>
<body>
<header>
  <button id="gateway" type="button" title="Pick a gateway"></button>
  <button id="model" type="button" title="Pick a model"></button>
  <button id="new" type="button" title="New chat">New</button>
</header>
<p id="dev" hidden>INSECURE — DEV ONLY: plain http to this computer</p>
<div id="toolsbar"><label><input id="tools" type="checkbox"> Tools</label> <span id="toolsnote"></span></div>
<section id="changes" hidden><h2>Changes to review</h2><ul id="changelist"></ul></section>
<main id="log" aria-live="polite"></main>
<p id="error" role="alert" hidden></p>
<form id="form">
  <textarea id="input" rows="3" placeholder="Ask the gateway's model (Ctrl+Enter to send)"></textarea>
  <div class="row"><button id="send" type="submit">Send</button><button id="stop" type="button" hidden>Stop</button></div>
</form>
<script nonce="${n}" src="${js}"></script>
</body>
</html>`;
  }
}
