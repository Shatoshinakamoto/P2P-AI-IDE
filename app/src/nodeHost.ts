/**
 * nodeHost.ts -- connects the editor to the local Node extension host (phase I6c-2,
 * docs/features/node-extensions-design.md).
 *
 * Purpose: Node extensions run in the VSCodium server that the Rust core starts for a trusted folder
 * (core/src/node_host.rs). The editor library can only be given that server when its services start, so this module
 * asks the shell, before the services start, whether a server runs for the open folder, and builds the workbench
 * options for it: the remote authority (`127.0.0.1:<port>`), the connection token and the folder as a
 * `vscode-remote://` workspace. Turning Node extensions on or off reloads the window (`reloadForNodeHost`); the open
 * folder and the server live in the shell and survive the reload.
 *
 * Trust boundaries: the port, token and folder come from the shell. The token is kept in this module's memory only,
 * never in web storage or the URL bar (CWE-522, CWE-598); VS Code sends it to the server on 127.0.0.1 only.
 * Security: the server listens on 127.0.0.1 only; the CSP allows the page (not the extension host iframe, see
 * vite.config.ts) to open `ws://127.0.0.1:*` and `http://127.0.0.1:*` for it (CWE-1385 style origin limits).
 */
import product from "@codingame/monaco-vscode-api/vscode/vs/platform/product/common/product";
import { nodeHostStart, nodeHostStatus, type NodeHostConnection } from "./ipc.ts";
import { sameMinor } from "./lib/vscodeVersion.ts";

/** The VS Code version the editor library is built on. */
export const editorVsCode: string = product.version ?? "";

/**
 * True when a server built from VS Code `server` speaks this editor's protocol (same minor version). A mismatched
 * pair connects but never starts an extension, so the editor neither offers nor connects one (lib/vscodeVersion.ts).
 */
export const hostMatchesEditor = (server: string) => sameMinor(editorVsCode, server);

let current: NodeHostConnection | null = null;

/** The running server for the open folder, or null. Called once, before the editor services start. */
export async function resolveNodeHost(): Promise<NodeHostConnection | null> {
  try {
    const st = await nodeHostStatus();
    // Reuse only a server that already runs (the person started it before the reload): never start one here.
    if (st.port === null || !st.trusted) return null;
    // SECURITY: fail closed on a server that can't speak this editor's protocol, instead of a silent half-working
    // connection (CWE-1188 style safe default).
    if (!hostMatchesEditor(st.vscode)) return null;
    current = await nodeHostStart(false);
    if (!hostMatchesEditor(current.vscode)) current = null;
  } catch {
    current = null;
  }
  return current;
}

/** The connection the editor was started with, or null. */
export const nodeHost = () => current;

/** `127.0.0.1:<port>`, the remote authority the workbench uses. */
export const authorityOf = (c: NodeHostConnection) => `127.0.0.1:${c.port}`;

/**
 * A folder path as the server's URI path: `/home/me/proj` stays, `C:\\me\\proj` becomes `/C:/me/proj`
 * (VS Code's own rule for Windows paths).
 */
export function remotePath(folder: string): string {
  const p = folder.replace(/\\/g, "/");
  return p.startsWith("/") ? p : `/${p}`;
}

/** Reloads the window so the editor services start again with (or without) the Node host. */
export function reloadForNodeHost(): void {
  window.location.reload();
}
