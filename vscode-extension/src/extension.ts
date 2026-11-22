/**
 * extension.ts -- entry point of the P2P-AI extension for VS Code and VSCodium (ADR-029 side track X, phase X1).
 *
 * Purpose: registers the chat view and the gateway commands:
 *   P2P-AI: Add Gateway, Pick Gateway, Pick Model, Set Gateway Key, Remove Gateway, New Chat.
 * Inputs / trust boundary: what the person types into VS Code input boxes and the chat view; gateway answers.
 * Security controls: keys only in SecretStorage and only typed into password input boxes (never echoed, CWE-549);
 * a CA file is read only from a file the person picks, capped and checked (CWE-295); see gateways.ts, chatView.ts
 * and gateway/client.ts for the rest.
 * Dependencies: the `vscode` API.
 */
import * as vscode from "vscode";
import { ChatViewProvider } from "./chatView.ts";
import { GatewayStore, caLooksValid } from "./gateways.ts";

/** Called by VS Code when the chat view or a P2P-AI command is first used. */
export function activate(ctx: vscode.ExtensionContext): void {
  const store = new GatewayStore(ctx);
  const chat = new ChatViewProvider(ctx, store);
  ctx.subscriptions.push(
    vscode.window.registerWebviewViewProvider(ChatViewProvider.viewId, chat, { webviewOptions: { retainContextWhenHidden: false } }),
    vscode.commands.registerCommand("p2pAi.addGateway", () => addGateway(store)),
    vscode.commands.registerCommand("p2pAi.pickGateway", () => pickGateway(store)),
    vscode.commands.registerCommand("p2pAi.pickModel", () => chat.pickModel()),
    vscode.commands.registerCommand("p2pAi.setKey", () => setKey(store)),
    vscode.commands.registerCommand("p2pAi.removeGateway", () => removeGateway(store)),
    vscode.commands.registerCommand("p2pAi.newChat", () => chat.newChat()),
  );
  // Test hooks for the integration test (tests/integration). SECURITY: registered only when VS Code itself runs
  // the extension in test mode (`--extensionTestsPath`, which a normal install never does) AND the test runner
  // set P2P_AI_EXT_TEST=1. They add nothing a person can't do through the commands above, and never return a key.
  if (ctx.extensionMode === vscode.ExtensionMode.Test && process.env.P2P_AI_EXT_TEST === "1") {
    ctx.subscriptions.push(
      vscode.commands.registerCommand("p2pAi.test.addGateway", (name: string, url: string, key: string) => store.add(String(name), String(url), String(key))),
      vscode.commands.registerCommand("p2pAi.test.setModel", (m: string) => chat.setModel(String(m))),
      vscode.commands.registerCommand("p2pAi.test.send", (t: string) => chat.send(String(t))),
      vscode.commands.registerCommand("p2pAi.test.gateways", () => store.list().map((g) => ({ name: g.name, url: g.url }))),
    );
  }
}

export function deactivate(): void {}

async function addGateway(store: GatewayStore): Promise<void> {
  const name = await vscode.window.showInputBox({ title: "Add gateway (1/4)", prompt: "A name for this gateway", ignoreFocusOut: true });
  if (name === undefined) return;
  const url = await vscode.window.showInputBox({
    title: "Add gateway (2/4)",
    prompt: "Address: https://host[:port] (plain http only for a gateway on this computer)",
    placeHolder: "https://gateway.example.org",
    ignoreFocusOut: true,
  });
  if (url === undefined) return;
  // SECURITY: password: true keeps the key off the screen (CWE-549).
  const key = await vscode.window.showInputBox({ title: "Add gateway (3/4)", prompt: "The key the gateway gave you", password: true, ignoreFocusOut: true });
  if (key === undefined) return;
  let caPem: string | undefined;
  const ca = await vscode.window.showQuickPick(["No, use the normal certificate check", "Yes, pick a CA file (.pem)"], {
    title: "Add gateway (4/4)",
    placeHolder: "Does this gateway use its own certificate authority?",
    ignoreFocusOut: true,
  });
  if (ca === undefined) return;
  if (ca.startsWith("Yes")) {
    const files = await vscode.window.showOpenDialog({ canSelectMany: false, filters: { "CA certificate": ["pem", "crt"] } });
    if (!files?.[0]) return;
    const stat = await vscode.workspace.fs.stat(files[0]);
    // SECURITY: size checked before reading (CWE-400).
    if (stat.size > 64 << 10) return void vscode.window.showErrorMessage("the CA file must be at most 64 KiB");
    caPem = new TextDecoder().decode(await vscode.workspace.fs.readFile(files[0]));
    if (!caLooksValid(caPem)) return void vscode.window.showErrorMessage("the CA file must hold PEM certificates");
  }
  const err = await store.add(name, url, key, caPem);
  if (err) vscode.window.showErrorMessage(err);
  else vscode.window.showInformationMessage(`Gateway "${name.trim()}" added. Pick a model in the P2P-AI chat view.`);
}

async function pickGateway(store: GatewayStore): Promise<void> {
  const all = store.list();
  if (all.length === 0) return addGateway(store);
  const items = [
    ...all.map((g) => ({ label: g.name, description: g.url.startsWith("http://") ? "INSECURE — DEV ONLY" : g.url, id: g.id })),
    { label: "$(add) Add gateway", description: "", id: "" },
  ];
  const pick = await vscode.window.showQuickPick(items, { title: "P2P-AI gateway" });
  if (!pick) return;
  if (pick.id === "") return addGateway(store);
  await store.setActive(pick.id);
}

async function chooseSaved(store: GatewayStore, title: string): Promise<string | undefined> {
  const all = store.list();
  if (all.length === 0) {
    vscode.window.showWarningMessage("No gateways saved yet.");
    return undefined;
  }
  const pick = await vscode.window.showQuickPick(all.map((g) => ({ label: g.name, description: g.url, id: g.id })), { title });
  return pick?.id;
}

async function setKey(store: GatewayStore): Promise<void> {
  const id = await chooseSaved(store, "Set the key for which gateway?");
  if (!id) return;
  const key = await vscode.window.showInputBox({ title: "New key", password: true, ignoreFocusOut: true });
  if (key === undefined) return;
  const err = await store.setKey(id, key);
  if (err) vscode.window.showErrorMessage(err);
}

async function removeGateway(store: GatewayStore): Promise<void> {
  const id = await chooseSaved(store, "Remove which gateway? Its key is deleted too.");
  if (!id) return;
  const ok = await vscode.window.showWarningMessage("Remove this gateway and delete its key?", { modal: true }, "Remove");
  if (ok === "Remove") await store.remove(id);
}
