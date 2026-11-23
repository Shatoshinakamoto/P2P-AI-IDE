/**
 * extension.ts -- entry point of the P2P-AI extension for VS Code and VSCodium (ADR-029 side track X, X1 and X2).
 *
 * Purpose: registers the chat view, the Missions view (X3), the diff review of proposed changes and the commands:
 *   P2P-AI: Add Gateway, Pick Gateway, Pick Model, Set Gateway Key, Remove Gateway, New Chat,
 *   Accept Change, Reject Change (the last two also in the diff editor's title bar), and for P2P-AI accounts
 *   (phase A3): Sign In with a P2P-AI Account, Change Account Password, Sign Out of a P2P-AI Account, Use a P2P-AI
 *   Account for a Gateway, Use the Key for a Gateway.
 * Inputs / trust boundary: what the person types into VS Code input boxes and the chat view; gateway answers.
 * Security controls: keys only in SecretStorage and only typed into password input boxes (never echoed, CWE-549);
 * a CA file is read only from a file the person picks, capped and checked (CWE-295); account sign-in happens in the
 * system browser (the extension never sees the password) and passwords for "change password" are typed into password
 * boxes and dropped after the call; see accounts.ts, gateways.ts, chatView.ts and gateway/client.ts for the rest.
 * Dependencies: the `vscode` API.
 */
import * as vscode from "vscode";
import { parseFromMissions } from "./viewMessages.ts";
import { ChatViewProvider } from "./chatView.ts";
import { GatewayStore, caLooksValid } from "./gateways.ts";
import { AcctError, type HomeView } from "./accounts.ts";
import { ChangeReview } from "./review.ts";
import { FileState } from "./fileState.ts";
import { MissionsViewProvider } from "./missionsView.ts";

/** Called by VS Code when the chat view or a P2P-AI command is first used. */
export function activate(ctx: vscode.ExtensionContext): void {
  // Non-secret settings live in the extension's own state file (see fileState.ts for why not globalState).
  const state = new FileState(ctx, ["p2pAi.gateways", "p2pAi.activeGateway", "p2pAi.tools", "p2pAi.model."]);
  const store = new GatewayStore(ctx, state);
  const review = new ChangeReview(ctx);
  const chat = new ChatViewProvider(ctx, store, review);
  // X3: audit lines (fixed words, VS Code adds the time) go to the "P2P-AI" output channel.
  const log = vscode.window.createOutputChannel("P2P-AI", { log: true });
  const missions = new MissionsViewProvider(ctx, store, review, log);
  // Accept or reject from the diff editor's title bar: the command gets the diff's URI, and only a URI of a pending
  // change is acted on (SECURITY: no other file can be named this way, CWE-22).
  const decide = (accept: boolean) => (uri?: vscode.Uri) => {
    const rel = review.relOf(uri ?? vscode.window.activeTextEditor?.document.uri);
    if (!rel) return void vscode.window.showWarningMessage("Open a proposed change from the P2P-AI chat first.");
    if (!accept) return void review.reject(rel);
    const err = review.accept(rel);
    if (err) vscode.window.showErrorMessage(err);
  };
  ctx.subscriptions.push(
    vscode.window.registerWebviewViewProvider(ChatViewProvider.viewId, chat, { webviewOptions: { retainContextWhenHidden: false } }),
    vscode.window.registerWebviewViewProvider(MissionsViewProvider.viewId, missions, { webviewOptions: { retainContextWhenHidden: false } }),
    log,
    vscode.commands.registerCommand("p2pAi.addGateway", () => addGateway(store)),
    vscode.commands.registerCommand("p2pAi.pickGateway", () => pickGateway(store)),
    vscode.commands.registerCommand("p2pAi.pickModel", () => chat.pickModel()),
    vscode.commands.registerCommand("p2pAi.setKey", () => setKey(store)),
    vscode.commands.registerCommand("p2pAi.removeGateway", () => removeGateway(store)),
    vscode.commands.registerCommand("p2pAi.signIn", () => signIn(store, log)),
    vscode.commands.registerCommand("p2pAi.changeAccountPassword", () => changePassword(store, log)),
    vscode.commands.registerCommand("p2pAi.signOut", () => signOut(store, log)),
    vscode.commands.registerCommand("p2pAi.useAccount", () => useAccount(store, log)),
    vscode.commands.registerCommand("p2pAi.useKey", () => useKey(store, log)),
    vscode.commands.registerCommand("p2pAi.newChat", () => chat.newChat()),
    vscode.commands.registerCommand("p2pAi.acceptChange", decide(true)),
    vscode.commands.registerCommand("p2pAi.rejectChange", decide(false)),
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
      vscode.commands.registerCommand("p2pAi.test.removeAll", async () => {
        for (const g of store.list()) await store.remove(g.id);
      }),
      vscode.commands.registerCommand("p2pAi.test.setTools", (on: boolean) => chat.setTools(on === true)),
      vscode.commands.registerCommand("p2pAi.test.changes", () => review.views()),
      vscode.commands.registerCommand("p2pAi.test.review", (rel: string) => review.open(String(rel))),
      // Missions: the same checked handler the view uses, with a message object shaped like the view's.
      vscode.commands.registerCommand("p2pAi.test.missions", async (m: unknown) => {
        const parsed = parseFromMissions(m);
        if (!parsed) return { error: "refused by the message check" };
        try {
          return { ok: await missions.handle(parsed) };
        } catch (e) {
          return { error: e instanceof Error ? e.message : "failed" };
        }
      }),
      vscode.commands.registerCommand("p2pAi.test.accept", (rel: string) => review.accept(String(rel)) ?? "ok"),
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

// ---------------------------------------------------------------------------------------------------------------
// P2P-AI accounts (phase A3; accounts.ts)
// ---------------------------------------------------------------------------------------------------------------

const errText = (e: unknown): string => (e instanceof AcctError ? e.message : "the account action failed");

/** Picks one remembered account service (or undefined). */
async function pickHome(store: GatewayStore, title: string, signedInOnly: boolean): Promise<HomeView | undefined> {
  const homes = (await store.accounts.views()).filter((h) => !signedInOnly || h.signedIn);
  if (homes.length === 0) {
    vscode.window.showWarningMessage(signedInOnly ? "No P2P-AI account is signed in (P2P-AI: Sign In with a P2P-AI Account)." : "No P2P-AI account service is remembered yet.");
    return undefined;
  }
  const items = homes.map((h) => ({
    label: h.email || h.issuer,
    description: `${h.dev ? "INSECURE — DEV ONLY · " : ""}${h.signedIn ? "signed in" : "signed out"}${h.mustChange ? " · choose your own password" : ""}`,
    detail: h.issuer,
    home: h,
  }));
  return (await vscode.window.showQuickPick(items, { title }))?.home;
}

/** Signs in at an account service in the system browser; a cancellable notification waits for it. */
async function signIn(store: GatewayStore, log: vscode.LogOutputChannel): Promise<void> {
  const known = await store.accounts.views();
  const address = await vscode.window.showInputBox({
    title: "Sign in with a P2P-AI account",
    prompt: "The account service address (the gateway's operator gives it to you). Sign-in happens in your browser; VS Code never sees your password.",
    placeHolder: "https://accounts.example.org",
    value: known[0]?.issuer ?? "",
    ignoreFocusOut: true,
  });
  if (address === undefined) return;
  try {
    const view = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Finish signing in in your browser...", cancellable: true },
      async (_p, token) => {
        const stop = new AbortController();
        token.onCancellationRequested(() => stop.abort());
        // SECURITY: the system browser, never a webview (RFC 8252 8.12); accounts.ts opens only the home's own page.
        const open = async (url: string): Promise<boolean> => {
          if (await vscode.env.openExternal(vscode.Uri.parse(url, true))) return true;
          // No browser opened: offer the link to copy (it holds no secret; the PKCE verifier never leaves the extension).
          const pick = await vscode.window.showWarningMessage("No browser opened. Copy the sign-in link and open it yourself?", "Copy sign-in link");
          if (pick) await vscode.env.clipboard.writeText(url);
          return pick !== undefined;
        };
        return store.accounts.signIn(address, open, stop.signal);
      },
    );
    // SECURITY: audit line with fixed words only (NIST AU-2, CWE-117).
    log.info("account signed_in");
    if (view.mustChange) {
      const go = await vscode.window.showWarningMessage("Signed in. This account must choose its own password before it can use gateways.", "Change password");
      if (go) await changePasswordFor(store, log, view.id);
    } else vscode.window.showInformationMessage(`Signed in${view.email ? ` as ${view.email}` : ""}. Link a gateway with 'P2P-AI: Use a P2P-AI Account for a Gateway'.`);
  } catch (e) {
    log.info("account sign_in_failed");
    vscode.window.showErrorMessage(errText(e));
  }
}

async function changePassword(store: GatewayStore, log: vscode.LogOutputChannel): Promise<void> {
  const h = await pickHome(store, "Change the password of which account?", true);
  if (h) await changePasswordFor(store, log, h.id);
}

/** Replaces the (one-time) password. SECURITY: password boxes (CWE-549); the values are dropped after the call. */
async function changePasswordFor(store: GatewayStore, log: vscode.LogOutputChannel, id: string): Promise<void> {
  const current = await vscode.window.showInputBox({ title: "Change password (1/2)", prompt: "Current (or one-time) password", password: true, ignoreFocusOut: true });
  if (current === undefined) return;
  const next = await vscode.window.showInputBox({ title: "Change password (2/2)", prompt: "New password, 15 or more characters", password: true, ignoreFocusOut: true });
  if (next === undefined) return;
  try {
    await store.accounts.changePassword(id, current, next);
    log.info("account password_changed");
    vscode.window.showInformationMessage("Password changed. Your other signed-in devices were signed out.");
  } catch (e) {
    log.info("account password_change_failed");
    vscode.window.showErrorMessage(errText(e));
  }
}

async function signOut(store: GatewayStore, log: vscode.LogOutputChannel): Promise<void> {
  const h = await pickHome(store, "Sign out of which account?", false);
  if (!h) return;
  try {
    await store.accounts.signOut(h.id);
    log.info("account signed_out");
    vscode.window.showInformationMessage("Signed out. Gateways linked to this account need a sign-in again (or their key).");
  } catch (e) {
    vscode.window.showErrorMessage(errText(e));
  }
}

/** Links a saved gateway to a signed-in account and the gateway's account id. */
async function useAccount(store: GatewayStore, log: vscode.LogOutputChannel): Promise<void> {
  const id = await chooseSaved(store, "Use a P2P-AI account for which gateway?");
  if (!id) return;
  const h = await pickHome(store, "Which account?", true);
  if (!h) return;
  const audience = await vscode.window.showInputBox({
    title: "The gateway's account id",
    prompt: "The gateway's account id (its GATEWAY_ID); the gateway's operator gives it to you.",
    placeHolder: "gw-example-01",
    ignoreFocusOut: true,
  });
  if (audience === undefined) return;
  const err = await store.setAccount(id, h.issuer, audience);
  if (err) return void vscode.window.showErrorMessage(err);
  log.info("account gateway_linked");
  vscode.window.showInformationMessage("This gateway now signs in with the account. Its key, if any, stays saved for 'P2P-AI: Use the Key for a Gateway'.");
}

async function useKey(store: GatewayStore, log: vscode.LogOutputChannel): Promise<void> {
  const linked = store.list().filter((g) => g.account);
  if (linked.length === 0) return void vscode.window.showWarningMessage("No gateway uses an account.");
  const pick = await vscode.window.showQuickPick(linked.map((g) => ({ label: g.name, description: g.account?.home, id: g.id })), { title: "Use the key again for which gateway?" });
  if (!pick) return;
  const err = await store.setAccount(pick.id, "", "");
  if (err) return void vscode.window.showErrorMessage(err);
  log.info("account gateway_unlinked");
}
