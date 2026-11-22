/**
 * ExtensionsView.tsx -- the Extensions view (phase I6a, docs/features/extensions.md): search Open VSX, install an
 * extension with one click, turn installed ones on or off, remove them, and pick a colour theme.
 *
 * Phase I6a installs extensions that only contribute files (themes, icon themes, grammars, snippets, language
 * settings). Phase I6b adds web extensions, whose code runs in the editor's web worker extension host (sandboxed
 * iframe, no network, no file system, no Node). Phase I6c adds Node extensions: they install here and run only in
 * the local Node extension host of a folder the person trusts (NodeHostPanel.tsx).
 *
 * Trust boundary: search results, names and descriptions come from Open VSX and are rendered by React as text only
 * (CWE-79). Nothing is installed without the person's click on Install; the core checks the download's SHA-256 and
 * Open VSX signature before anything is written.
 * Dependencies: React, lucide-react icons.
 */
import { useEffect, useState } from "react";
import { BadgeCheck, Download, Trash2 } from "lucide-react";
import { extInstall, extList, extManifest, extSearch, extSetEnabled, extSetNetwork, extUninstall, type ExtInstalled, type ExtSummary } from "../ipc.ts";
import { syncExtensions } from "../extensions.ts";
import { DEFAULT_THEME, setColorTheme } from "../monaco.ts";
import { saveTheme, savedTheme } from "../lib/theme.ts";
import { NodeHostPanel } from "./NodeHostPanel.tsx";

interface Props {
  onNotice: (m: string) => void;
}

/** A theme in the picker; `ext` is the installed extension that contributes it (none for built-in themes). */
interface ThemeChoice { id: string; label: string; ext?: string }

/** Themes an installed extension contributes (id, or label when there is no id). */
function themesOf(manifest: Record<string, unknown>): ThemeChoice[] {
  const themes = ((manifest.contributes ?? {}) as { themes?: unknown }).themes;
  if (!Array.isArray(themes)) return [];
  return themes
    .map((t) => t as { id?: unknown; label?: unknown })
    .map((t) => ({ id: String(t.id ?? t.label ?? ""), label: String(t.label ?? t.id ?? "") }))
    .filter((t) => t.id && !t.label.startsWith("%"))
    .slice(0, 50);
}

export function ExtensionsView({ onNotice }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ExtSummary[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [installed, setInstalled] = useState<ExtInstalled[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [themes, setThemes] = useState<ThemeChoice[]>([]);
  const [theme, setTheme] = useState(savedTheme() ?? DEFAULT_THEME);

  const refresh = async () => {
    try {
      const list = await extList();
      setInstalled(list);
      const choices: ThemeChoice[] = [{ id: DEFAULT_THEME, label: "Dark Modern (built in)" }, { id: "Light Modern", label: "Light Modern (built in)" }];
      for (const e of list.filter((x) => x.enabled)) {
        try {
          for (const t of themesOf(await extManifest(e.id))) choices.push({ id: t.id, label: `${t.label} (${e.display_name || e.name})`, ext: e.id });
        } catch {
          /* listed without themes */
        }
      }
      setThemes(choices);
    } catch (e) {
      onNotice((e as Error).message);
    }
  };
  useEffect(() => { void refresh(); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  // The consent card (extNet.ts) changes grants; show them at once.
  useEffect(() => {
    const on = () => void refresh();
    window.addEventListener("ide-extensions-changed", on);
    return () => window.removeEventListener("ide-extensions-changed", on);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const runSearch = async () => {
    if (!query.trim()) return;
    setSearching(true);
    try {
      setResults(await extSearch(query.trim()));
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setSearching(false);
    }
  };

  /** Runs one change, re-registers extensions with the editor, refreshes the list. */
  const act = async (key: string, fn: () => Promise<unknown>, done: string) => {
    setBusy(key);
    try {
      await fn();
      await syncExtensions();
      await refresh();
      onNotice(done);
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const pickTheme = async (id: string) => {
    setTheme(id);
    saveTheme(id);
    await setColorTheme(id);
  };

  /**
   * Before an extension is turned off or removed, moves the editor off a theme it contributes. Otherwise the theme
   * service falls back on its own and tries to rewrite the settings file, which the standalone editor cannot do.
   */
  const leaveThemeOf = async (extId: string) => {
    if (themes.some((t) => t.id === theme && t.ext === extId)) await pickTheme(DEFAULT_THEME);
  };

  const isInstalled = (r: ExtSummary) => installed.some((e) => e.id === `${r.namespace}.${r.name}`.toLowerCase());

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto px-3 pb-4">
      <form className="flex gap-1" onSubmit={(e) => { e.preventDefault(); void runSearch(); }}>
        <label htmlFor="ext-search" className="sr-only">Search Open VSX</label>
        <input id="ext-search" value={query} maxLength={100} onChange={(e) => setQuery(e.target.value)} placeholder="Search extensions in Open VSX" className="min-w-0 flex-1 rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright" />
        <button type="submit" disabled={searching || !query.trim()} className="rounded bg-ide-accent px-2 py-1 text-white disabled:opacity-50">{searching ? "…" : "Search"}</button>
      </form>
      <p className="text-xs text-ide-dim">
        Installs themes, icon themes, language grammars, snippets and web extensions. Each download is checked against
        its SHA-256 and Open VSX signature first. Web extensions run in a sandbox inside the editor: they see the files
        open in the editor and have no other access to your computer. One that wants the internet asks you first. Node
        extensions run only in a folder you trust, in the Node extension host below.
      </p>

      {results && (
        <section aria-label="Search results" className="flex flex-col gap-2">
          {results.length === 0 && <p className="text-ide-dim">No extensions found.</p>}
          {results.map((r) => {
            const key = `${r.namespace}.${r.name}`;
            return (
              <div key={key} className="rounded border border-ide-line p-2">
                <div className="flex items-start gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-ide-bright">{r.display_name || r.name}</div>
                    <div className="truncate text-xs text-ide-dim">{r.namespace} · v{r.version} · {r.downloads.toLocaleString()} downloads</div>
                  </div>
                  <button
                    type="button"
                    aria-label={`Install ${r.display_name || r.name}`}
                    disabled={busy !== null || isInstalled(r)}
                    onClick={() => void act(key, () => extInstall(r.namespace, r.name), `Installed ${r.display_name || r.name}`)}
                    className="flex shrink-0 items-center gap-1 rounded bg-ide-accent px-2 py-0.5 text-xs text-white disabled:opacity-50"
                  >
                    <Download size={12} aria-hidden />{isInstalled(r) ? "Installed" : busy === key ? "Installing…" : "Install"}
                  </button>
                </div>
                {r.description && <p className="mt-1 line-clamp-2 text-xs text-ide-text">{r.description}</p>}
              </div>
            );
          })}
        </section>
      )}

      <NodeHostPanel nodeCount={installed.filter((e) => e.kind === "node" && e.enabled).length} onNotice={onNotice} />

      <section aria-label="Installed extensions" className="flex flex-col gap-2">
        <h2 className="text-xs uppercase tracking-wide text-ide-dim">Installed</h2>
        {installed.length === 0 && <p className="text-ide-dim">No extensions installed.</p>}
        {installed.map((e) => (
          <div key={e.id} className="rounded border border-ide-line p-2">
            <div className="flex items-center gap-2">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1 truncate text-ide-bright">
                  {e.display_name || e.name}
                  {e.verified_publisher && <BadgeCheck size={13} aria-label="Verified publisher on Open VSX" />}
                </div>
                <div className="truncate text-xs text-ide-dim">
                  {e.namespace} · v{e.version}{e.kind === "web" ? (e.network ? " · web extension (runs in the editor sandbox, internet allowed)" : " · web extension (runs in the editor sandbox, no internet)") : e.kind === "node" ? " · Node extension (runs in a trusted folder's Node extension host)" : ""}
                </div>
                {e.kind === "web" && !e.network && (
                  <button type="button" disabled={busy !== null || !e.enabled} onClick={() => void act(e.id, () => extSetNetwork(e.id, true), `Allowed internet for ${e.name}`)} className="mt-1 rounded border border-ide-line px-1.5 text-xs hover:bg-ide-hover disabled:opacity-50">
                    Allow internet<span className="sr-only"> for {e.display_name || e.name}</span>
                  </button>
                )}
                {e.kind === "web" && e.network && (
                  <button type="button" disabled={busy !== null} onClick={() => void act(e.id, () => extSetNetwork(e.id, false), `Stopped internet for ${e.name}`)} className="mt-1 rounded border border-ide-line px-1.5 text-xs hover:bg-ide-hover disabled:opacity-50">
                    Stop internet<span className="sr-only"> for {e.display_name || e.name}</span>
                  </button>
                )}
              </div>
              <label className="flex items-center gap-1 text-xs">
                <input type="checkbox" checked={e.enabled} disabled={busy !== null} onChange={(ev) => { const on = ev.target.checked; void act(e.id, async () => { if (!on) await leaveThemeOf(e.id); return extSetEnabled(e.id, on); }, on ? `Turned on ${e.name}` : `Turned off ${e.name}`); }} />
                On
              </label>
              <button type="button" aria-label={`Remove ${e.display_name || e.name}`} disabled={busy !== null} onClick={() => void act(e.id, async () => { await leaveThemeOf(e.id); return extUninstall(e.id); }, `Removed ${e.name}`)} className="rounded p-1 hover:bg-ide-hover disabled:opacity-50">
                <Trash2 size={14} aria-hidden />
              </button>
            </div>
          </div>
        ))}
      </section>

      <section aria-label="Colour theme" className="flex flex-col gap-1">
        <label htmlFor="ext-theme" className="text-xs uppercase tracking-wide text-ide-dim">Colour theme</label>
        <select id="ext-theme" value={theme} onChange={(e) => void pickTheme(e.target.value)} className="rounded border border-ide-line bg-ide-bg px-1 py-1 text-ide-bright">
          {themes.map((t) => <option key={t.id} value={t.id}>{t.label}</option>)}
        </select>
      </section>
    </div>
  );
}
