/**
 * extensions.ts -- gives installed, enabled Open VSX extensions to the editor services (phases I6a-I6b).
 *
 * Purpose: for each enabled extension, reads its package.json and the files its contributions name (themes, icon
 * themes, grammars, snippets, language settings, translations) through the Rust core, and registers them with
 * @codingame/monaco-vscode-api exactly as the built-in extensions are registered. Removing or turning an extension off
 * unregisters it.
 *
 * Trust boundaries: the manifest and files are third-party data that the core verified at install time (Open VSX
 * hash and signature) and reads only from inside the extension's own folder. Declarative extensions run no code.
 * Web extensions (I6b, a `browser` entry) are registered with the web worker extension host only, so their code runs
 * in a Web Worker inside the editor's sandboxed hidden iframe: no Node, no file system, no Tauri IPC, and no network
 * under the app's CSP. An extension with a Node `main` and no `browser` entry is never registered here (the core
 * refuses it at install; this file checks again, defence in depth, CWE-94).
 * Security: files become blob: URLs owned by this page (CSP connect-src blob:), revoked when the extension is
 * removed; nothing is fetched from the network here.
 */
import { ExtensionHostKind, registerExtension, type IExtensionManifest, type RegisterLocalExtensionResult } from "@codingame/monaco-vscode-api/extensions";
import { getService, IExtensionService } from "@codingame/monaco-vscode-api/services";
import { extFile, extList, extManifest } from "./ipc.ts";
import { armExtensionNet } from "./extNet.ts";

type Registered = { dispose(): Promise<void>; urls: string[]; web: boolean };
const active = new Map<string, Registered>();

/** The file paths an extension's declarative contributions name, without a leading "./". */
export function contributedFiles(manifest: Record<string, unknown>): string[] {
  const c = (manifest.contributes ?? {}) as Record<string, unknown>;
  const paths = new Set<string>();
  const add = (p: unknown) => {
    if (typeof p === "string" && p.length > 0 && p.length < 512) paths.add(p.replace(/^\.\//, ""));
  };
  for (const key of ["themes", "iconThemes", "productIconThemes", "grammars", "snippets"]) {
    const list = c[key];
    if (Array.isArray(list)) for (const item of list) add((item as { path?: unknown })?.path);
  }
  if (Array.isArray(c.languages)) for (const l of c.languages) add((l as { configuration?: unknown })?.configuration);
  paths.add("package.nls.json");
  return [...paths];
}

const mimeFor = (p: string) =>
  p.endsWith(".json") ? "application/json" : p.endsWith(".svg") ? "image/svg+xml" : p.endsWith(".js") || p.endsWith(".cjs") || p.endsWith(".mjs") ? "text/javascript" : "application/octet-stream";

/**
 * The file a web extension's `browser` entry names, as stored in the package. VS Code adds ".js" when the entry has no
 * extension. Returns null for anything that is not a plain relative path (the core also refuses those when reading).
 */
export function browserEntry(manifest: Record<string, unknown>): string | null {
  const b = manifest.browser;
  if (typeof b !== "string" || b.length === 0 || b.length > 256) return null;
  const p = b.replace(/^\.\//, "");
  // SECURITY: relative path parts only (CWE-22); the core enforces the same when it reads the file.
  if (p.startsWith("/") || p.split("/").some((part) => part === ".." || part === "")) return null;
  return /\.[cm]?js$/.test(p) ? p : `${p}.js`;
}

/** A data: URL for a file's bytes (base64, done by the browser). */
function dataUrl(bytes: ArrayBuffer, mime: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(new Error("could not read the extension file"));
    r.readAsDataURL(new Blob([bytes], { type: mime }));
  });
}

/** Registers one installed extension; files that are missing are skipped (the editor reports them). */
async function registerOne(id: string): Promise<void> {
  const manifest = await extManifest(id);
  const entry = browserEntry(manifest);
  // SECURITY: never register Node code (a `main` with no `browser` entry), or a `browser` entry that isn't a plain
  // relative path; the core refuses both at install, this is defence in depth (CWE-94).
  if (manifest.browser !== undefined && !entry) return;
  if (!entry && manifest.main) return;
  // Web extensions go to the web worker host only (never LocalProcess); declarative ones get no host, as the
  // built-in extensions do, and still serve their files.
  const reg = (entry
    ? registerExtension(manifest as unknown as IExtensionManifest, ExtensionHostKind.LocalWebWorker, { system: false })
    : registerExtension(manifest as unknown as IExtensionManifest, undefined, { system: false })) as RegisterLocalExtensionResult;
  const urls: string[] = [];
  const files = contributedFiles(manifest);
  if (entry) files.unshift(entry);
  for (const path of files) {
    try {
      const bytes = await extFile(id, path);
      // The worker host fetches a web extension's files from inside the extension host iframe, whose own CSP allows
      // data: but not blob: (connect-src), so those files are data: URLs; declarative files stay page-owned blobs.
      const url = entry ? await dataUrl(bytes, mimeFor(path)) : URL.createObjectURL(new Blob([bytes], { type: mimeFor(path) }));
      if (!entry) urls.push(url);
      reg.registerFileUrl(path, url, { mimeType: mimeFor(path), size: bytes.byteLength });
    } catch {
      /* not in the package (for example no package.nls.json) */
    }
  }
  active.set(id, { dispose: () => reg.dispose(), urls, web: entry !== null });
  await reg.whenReady();
}

async function unregisterOne(id: string): Promise<void> {
  const r = active.get(id);
  if (!r) return;
  active.delete(id);
  // SECURITY: an activated web extension keeps running in the worker, and VS Code won't drop an activated extension
  // while its host runs. So removing or turning one off stops the extension hosts, unregisters it, and starts them
  // again without it: its code, providers and timers go away with the old worker.
  // Stopping clears the "activated" mark, which is what lets the registration below actually remove it.
  const svc = r.web ? await getService(IExtensionService) : null;
  const stopped = svc ? await svc.stopExtensionHosts("an extension was removed or turned off", true) : false;
  await r.dispose();
  if (svc && stopped) {
    // The new worker announces a new private channel for internet requests; accept it (extNet.ts).
    armExtensionNet();
    await svc.startExtensionHosts();
  }
  for (const u of r.urls) URL.revokeObjectURL(u);
}

/** Makes the editor match the installed list: registers enabled extensions, unregisters the rest. */
export async function syncExtensions(): Promise<void> {
  const list = await extList();
  const want = new Set(list.filter((e) => e.enabled && (e.kind === "declarative" || e.kind === "web")).map((e) => e.id));
  for (const id of [...active.keys()]) if (!want.has(id)) await unregisterOne(id);
  for (const id of want) {
    if (active.has(id)) continue;
    try {
      await registerOne(id);
    } catch {
      /* a broken extension must not stop the others */
    }
  }
}
