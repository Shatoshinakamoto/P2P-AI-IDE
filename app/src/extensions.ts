/**
 * extensions.ts -- gives installed, enabled Open VSX extensions to the editor services (phase I6a).
 *
 * Purpose: for each enabled extension, reads its package.json and the files its contributions name (themes, icon
 * themes, grammars, snippets, language settings, translations) through the Rust core, and registers them with
 * @codingame/monaco-vscode-api exactly as the built-in extensions are registered. Removing or turning an extension off
 * unregisters it.
 *
 * Trust boundaries: the manifest and files are third-party data that the core verified at install time (Open VSX
 * hash and signature) and reads only from inside the extension's own folder. No extension code runs in I6a: the core
 * refuses extensions with a `main` or `browser` entry, and this file checks again before registering (defence in
 * depth, CWE-94).
 * Security: files become blob: URLs owned by this page (CSP connect-src blob:), revoked when the extension is
 * removed; nothing is fetched from the network here.
 */
import { registerExtension, type IExtensionManifest, type RegisterLocalExtensionResult } from "@codingame/monaco-vscode-api/extensions";
import { extFile, extList, extManifest } from "./ipc.ts";

type Registered = { dispose(): Promise<void>; urls: string[] };
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

const mimeFor = (p: string) => (p.endsWith(".json") || p.endsWith(".tmLanguage.json") ? "application/json" : p.endsWith(".svg") ? "image/svg+xml" : "application/octet-stream");

/** Registers one installed extension; files that are missing are skipped (the editor reports them). */
async function registerOne(id: string): Promise<void> {
  const manifest = await extManifest(id);
  // SECURITY: I6a runs no extension code; refuse a manifest with a code entry even if one slipped past install.
  if (manifest.main || manifest.browser) return;
  // No extension host kind: declarative only, the same call the built-in extensions make; it still serves files.
  const reg = registerExtension(manifest as unknown as IExtensionManifest, undefined, { system: false }) as RegisterLocalExtensionResult;
  const urls: string[] = [];
  for (const path of contributedFiles(manifest)) {
    try {
      const bytes = await extFile(id, path);
      const url = URL.createObjectURL(new Blob([bytes], { type: mimeFor(path) }));
      urls.push(url);
      reg.registerFileUrl(path, url, { mimeType: mimeFor(path), size: bytes.byteLength });
    } catch {
      /* not in the package (for example no package.nls.json) */
    }
  }
  active.set(id, { dispose: () => reg.dispose(), urls });
  await reg.whenReady();
}

async function unregisterOne(id: string): Promise<void> {
  const r = active.get(id);
  if (!r) return;
  active.delete(id);
  await r.dispose();
  for (const u of r.urls) URL.revokeObjectURL(u);
}

/** Makes the editor match the installed list: registers enabled extensions, unregisters the rest. */
export async function syncExtensions(): Promise<void> {
  const list = await extList();
  const want = new Set(list.filter((e) => e.enabled && e.kind === "declarative").map((e) => e.id));
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
