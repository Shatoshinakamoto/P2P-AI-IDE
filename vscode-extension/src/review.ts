/**
 * review.ts -- the open folder, the agent's pending changes and their diff review in VS Code (side track X2).
 *
 * Purpose: holds the `Workspace` and `Pending` for the chat's agent tools, shows a proposed change in VS Code's own
 * diff editor (read-only, both sides served from memory), and writes it only when the person presses Accept
 * (in the chat view's change list or the diff editor's title bar).
 *
 * Inputs / trust boundary: the first folder of the VS Code workspace (the person's choice); file paths from the
 * chat view (untrusted: looked up among the pending changes, nothing else is opened); proposed text from the model
 * (untrusted: shown as plain text in a read-only document, never run).
 *
 * Security controls:
 *  - Tools only in a trusted workspace (VS Code Workspace Trust) on a local `file:` folder: in Restricted Mode the
 *    agent can't read the folder at all, so files from an unknown source are never sent to a gateway (CWE-200,
 *    OWASP LLM01 prompt injection from a cloned repository).
 *  - Nothing is written without the person's Accept, and the accept checks that the file is unchanged since the
 *    agent read it (OWASP LLM06, CWE-367). Rejected and accepted changes leave memory at once.
 *  - The diff is VS Code's read-only text diff of two in-memory documents: no command URIs, no HTML (CWE-79).
 *  - The pending list is cleared when the workspace folders change, so a change can't land in another folder.
 * Dependencies: the `vscode` API, ./agent/workspace.ts, ./agent/tools.ts.
 */
import * as vscode from "vscode";
import { Pending, type ChangeView } from "./agent/tools.ts";
import { Workspace, WsError } from "./agent/workspace.ts";

export const REVIEW_SCHEME = "p2p-ai-review";

/** Why the tools are off, or undefined when they can be used. */
export type ToolsBlock = "untrusted" | "no_folder" | "not_local" | undefined;

export class ChangeReview implements vscode.TextDocumentContentProvider {
  readonly pending = new Pending();
  private ws: Workspace | undefined;
  private wsFor: string | undefined;
  private gen = 0;
  private readonly changed = new vscode.EventEmitter<void>();
  /** Fires when the pending list changes. */
  readonly onDidChangeList = this.changed.event;
  private readonly docChanged = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.docChanged.event;

  constructor(ctx: vscode.ExtensionContext) {
    ctx.subscriptions.push(
      vscode.workspace.registerTextDocumentContentProvider(REVIEW_SCHEME, this),
      // SECURITY: a new set of folders drops the old folder and its pending changes.
      vscode.workspace.onDidChangeWorkspaceFolders(() => this.reset()),
      vscode.workspace.onDidGrantWorkspaceTrust(() => this.changed.fire()),
      this.changed,
      this.docChanged,
    );
  }

  private reset(): void {
    this.ws = undefined;
    this.wsFor = undefined;
    this.pending.clear();
    this.changed.fire();
  }

  /** Why the tools can't run here, or undefined. */
  block(): ToolsBlock {
    if (!vscode.workspace.isTrusted) return "untrusted";
    const f = vscode.workspace.workspaceFolders?.[0];
    if (!f) return "no_folder";
    if (f.uri.scheme !== "file") return "not_local";
    return undefined;
  }

  /** The open folder for the tools, or undefined (see `block`). */
  folder(): Workspace | undefined {
    if (this.block() !== undefined) return undefined;
    const fsPath = (vscode.workspace.workspaceFolders as readonly vscode.WorkspaceFolder[])[0].uri.fsPath;
    if (this.ws && this.wsFor === fsPath) return this.ws;
    try {
      this.ws = Workspace.open(fsPath);
      this.wsFor = fsPath;
      this.pending.clear();
    } catch {
      this.ws = undefined;
    }
    return this.ws;
  }

  views(): ChangeView[] {
    return this.pending.views();
  }

  /** Tells the list's listeners that the agent proposed something. */
  notify(): void {
    this.gen++;
    this.changed.fire();
  }

  /** Serves the two sides of a diff from memory: `/before/<rel>` and `/after/<rel>`. */
  provideTextDocumentContent(uri: vscode.Uri): string {
    const m = /^\/(before|after)\/(.+)$/.exec(uri.path);
    const f = m ? this.pending.files.get(m[2]) : undefined;
    if (!m || !f) return "";
    return m[1] === "before" ? f.before : f.after;
  }

  /** The pending path a review URI points at, or undefined. */
  relOf(uri: vscode.Uri | undefined): string | undefined {
    if (!uri || uri.scheme !== REVIEW_SCHEME) return undefined;
    const m = /^\/(?:before|after)\/(.+)$/.exec(uri.path);
    return m && this.pending.files.has(m[1]) ? m[1] : undefined;
  }

  /** Opens VS Code's diff editor for one pending change. Returns false when there is no such change. */
  async open(rel: string): Promise<boolean> {
    // SECURITY: only paths in the pending list are opened (CWE-22: the view can't name any other file).
    const f = this.pending.files.get(rel);
    if (!f) return false;
    const q = `v=${this.gen}`;
    const left = vscode.Uri.from({ scheme: REVIEW_SCHEME, path: `/before/${rel}`, query: q });
    const right = vscode.Uri.from({ scheme: REVIEW_SCHEME, path: `/after/${rel}`, query: q });
    this.docChanged.fire(left);
    this.docChanged.fire(right);
    const title = `${rel} (${f.baseVersion === undefined ? "new file" : "proposed change"}, not saved)`;
    await vscode.commands.executeCommand("vscode.diff", left, right, title, { preview: true });
    return true;
  }

  /**
   * Writes one change. Returns undefined on success or the message to show.
   * SECURITY: the version check refuses a file that changed on disk since the agent read it (CWE-367).
   */
  accept(rel: string): string | undefined {
    const ws = this.folder();
    if (!ws) return "the folder is not open any more";
    try {
      this.pending.accept(ws, rel);
    } catch (e) {
      if (e instanceof WsError && e.kind === "conflict") return "the file changed on disk after the assistant read it; reject this change and ask again";
      return e instanceof WsError ? e.message : "the change could not be saved";
    } finally {
      this.changed.fire();
    }
    void this.closeDiffs(rel);
    return undefined;
  }

  reject(rel: string): boolean {
    const ok = this.pending.reject(rel);
    this.changed.fire();
    if (ok) void this.closeDiffs(rel);
    return ok;
  }

  /** Closes open diff tabs for a change that was accepted or rejected (best effort). */
  private async closeDiffs(rel: string): Promise<void> {
    const tabs = vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => {
      const input = t.input;
      return input instanceof vscode.TabInputTextDiff && input.modified.scheme === REVIEW_SCHEME && input.modified.path === `/after/${rel}`;
    });
    if (tabs.length > 0) await vscode.window.tabGroups.close(tabs, true);
  }
}
