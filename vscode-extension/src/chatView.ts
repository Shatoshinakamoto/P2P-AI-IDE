/**
 * chatView.ts -- the P2P-AI chat view in VS Code's side bar (side track X1).
 *
 * Purpose: a chat with the active gateway's models, like the IDE's chat panel: pick a gateway and a model, type,
 * watch the answer stream in, stop it, start a new chat.
 * Inputs / trust boundary: messages posted by the view's webview (untrusted: allow-listed types, capped text);
 * gateway answers (untrusted: plain text only). Output: requests through GatewayClient; text posted to the webview.
 * Security controls:
 *  - The webview has no network and no Node: CSP `default-src 'none'`, scripts only with a per-load nonce, styles
 *    only from the extension's own media folder, `localResourceRoots` limited to that folder (CWE-79, CWE-829).
 *  - The webview renders every message with `textContent`, never as HTML (CWE-79).
 *  - The webview never sees the key or the gateway's address; it gets the gateway's name and a dev flag only.
 *  - Messages from the webview are checked by type and size before use (CWE-20, CWE-400).
 * Dependencies: the `vscode` API, ./gateway/client.ts, ./gateways.ts.
 */
import * as vscode from "vscode";
import { GatewayClient } from "./gateway/client.ts";
import { NetError, modelOk, type Msg } from "./gateway/checks.ts";
import type { GatewayStore, SavedGateway } from "./gateways.ts";
import { parseFromView, type FromView } from "./viewMessages.ts";

const MODEL_KEY = (id: string): string => `p2pAi.model.${id}`;

/** A random nonce for the webview's script tag (CSPRNG). */
function nonce(): string {
  return Buffer.from(crypto.getRandomValues(new Uint8Array(18))).toString("base64");
}

export class ChatViewProvider implements vscode.WebviewViewProvider {
  static readonly viewId = "p2pAi.chat";
  private view: vscode.WebviewView | undefined;
  private messages: Msg[] = [];
  private turn: AbortController | undefined;

  private readonly ctx: vscode.ExtensionContext;
  private readonly store: GatewayStore;

  constructor(ctx: vscode.ExtensionContext, store: GatewayStore) {
    this.ctx = ctx;
    this.store = store;
    store.onDidChange(() => this.postState());
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
        await this.ctx.globalState.update(MODEL_KEY(gw.id), pick);
        this.postState();
      }
    } catch (e) {
      vscode.window.showErrorMessage(e instanceof NetError ? e.message : "could not list the models");
    }
  }

  private model(gw: SavedGateway | undefined): string | undefined {
    if (!gw) return undefined;
    const m = this.ctx.globalState.get<string>(MODEL_KEY(gw.id));
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
    }
  }

  /** Sets the active gateway's model without a picker (used by the test-mode hooks only). */
  async setModel(model: string): Promise<boolean> {
    const gw = this.store.active();
    if (!gw || !modelOk(model)) return false;
    await this.ctx.globalState.update(MODEL_KEY(gw.id), model);
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
    const answer: Msg = { role: "assistant", content: "" };
    this.post({ type: "user", text });
    const turn = new AbortController();
    this.turn = turn;
    this.post({ type: "busy", busy: true });
    try {
      const client = new GatewayClient({ url: gw.url, key: await this.store.key(gw.id), caPem: gw.caPem });
      const end = await client.chatStream(
        model,
        this.messages,
        (t) => {
          answer.content += t;
          this.post({ type: "delta", text: t });
        },
        turn.signal,
      );
      this.messages.push(answer);
      this.post({ type: "end", finish: end.finishReason });
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
      messages: this.messages,
      busy: !!this.turn,
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
