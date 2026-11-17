/**
 * Editor.tsx -- the editor area: tabs, breadcrumbs and one Monaco editor that swaps models per open file.
 *
 * Purpose: multi-file editing (one Monaco model per tab keeps undo history and cursor per file), multi-cursor and
 * multi-line editing (Monaco's own: Alt+Click, Ctrl+Alt+Up/Down, column selection), find and replace, Ctrl+S to save.
 * Trust boundary: file text is untrusted content; Monaco renders it as text in its own DOM, never as HTML. Saves go
 * through the Rust core with the version the file was loaded at, so an outside change is never overwritten silently
 * (CWE-367); on a conflict the person chooses to reload or keep editing.
 */
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import { Circle, X } from "lucide-react";
import { monaco } from "../monaco.ts";
import { readFile, writeFile } from "../ipc.ts";
import { baseName, crumbs, languageFor } from "../lib/paths.ts";

/** One open file. `savedAltId` is the model's alternative version id at the last load or save (for dirty tracking). */
interface Tab {
  rel: string;
  version: string;
  dirty: boolean;
}

export interface EditorHandle {
  /** Opens a file in a tab (or focuses its tab) and optionally reveals a line. */
  open: (rel: string, line?: number) => Promise<void>;
  /** Closes every tab and drops every model (when another workspace opens). */
  reset: () => void;
  /** Tells the editor a file was written outside it (an accepted assistant change): a clean tab reloads, a tab with
   * unsaved edits shows the conflict bar so nothing is lost. */
  fileWritten: (rel: string) => void;
}

interface Props {
  ref?: Ref<EditorHandle>;
  onActiveChange: (rel: string | null) => void;
  onCursor: (line: number, col: number, language: string) => void;
  onNotice: (message: string) => void;
}

/** A Monaco URI per relative path, under a private scheme so Monaco never treats it as a real file URL. */
const uriFor = (rel: string) => monaco.Uri.from({ scheme: "p2pws", path: `/${rel}` });

export function Editor({ ref, onActiveChange, onCursor, onNotice }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const savedAlt = useRef<Map<string, number>>(new Map());
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [conflict, setConflict] = useState<string | null>(null);
  // Refs mirror state for Monaco callbacks, which are created once.
  const tabsRef = useRef<Tab[]>([]);
  const activeRef = useRef<string | null>(null);
  tabsRef.current = tabs;
  activeRef.current = active;

  // Create the one editor instance.
  useEffect(() => {
    if (!host.current) return;
    const ed = monaco.editor.create(host.current, {
      model: null,
      theme: "p2p-dark",
      automaticLayout: true,
      fontSize: 14,
      minimap: { enabled: true },
      multiCursorModifier: "alt",
      renderWhitespace: "selection",
      // SECURITY: links in file text are not opened from the editor (no navigation out of the app).
      links: false,
    });
    editor.current = ed;
    ed.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => { void save(); });
    ed.onDidChangeCursorPosition((e) => {
      const rel = activeRef.current;
      onCursor(e.position.lineNumber, e.position.column, rel ? languageFor(rel) : "plaintext");
    });
    return () => {
      ed.dispose();
      for (const m of monaco.editor.getModels()) m.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Marks a tab dirty or clean from its model's version. */
  const trackDirty = (rel: string, model: monaco.editor.ITextModel) => {
    model.onDidChangeContent(() => {
      const dirty = model.getAlternativeVersionId() !== savedAlt.current.get(rel);
      setTabs((ts) => ts.map((t) => (t.rel === rel && t.dirty !== dirty ? { ...t, dirty } : t)));
    });
  };

  const show = (rel: string, line?: number) => {
    const model = monaco.editor.getModel(uriFor(rel));
    const ed = editor.current;
    if (!model || !ed) return;
    ed.setModel(model);
    setActive(rel);
    onActiveChange(rel);
    // Report this file's language now: the cursor listener reads activeRef, which still names the previous tab
    // until React re-renders, so without this the status bar kept the old language after a tab switch.
    const pos = ed.getPosition();
    onCursor(pos?.lineNumber ?? 1, pos?.column ?? 1, languageFor(rel));
    if (line) {
      ed.revealLineInCenter(line);
      ed.setPosition({ lineNumber: line, column: 1 });
    }
    ed.focus();
  };

  const open = async (rel: string, line?: number) => {
    if (!monaco.editor.getModel(uriFor(rel))) {
      try {
        const f = await readFile(rel);
        const model = monaco.editor.createModel(f.text, languageFor(rel), uriFor(rel));
        savedAlt.current.set(rel, model.getAlternativeVersionId());
        trackDirty(rel, model);
        setTabs((ts) => [...ts, { rel, version: f.version, dirty: false }]);
      } catch (e) {
        onNotice((e as Error).message);
        return;
      }
    }
    show(rel, line);
  };

  const save = async () => {
    const rel = activeRef.current;
    const tab = tabsRef.current.find((t) => t.rel === rel);
    const model = rel ? monaco.editor.getModel(uriFor(rel)) : null;
    if (!rel || !tab || !model) return;
    try {
      const version = await writeFile(rel, model.getValue(), tab.version);
      savedAlt.current.set(rel, model.getAlternativeVersionId());
      setTabs((ts) => ts.map((t) => (t.rel === rel ? { ...t, version, dirty: false } : t)));
      setConflict(null);
      onNotice(`Saved ${baseName(rel)}`);
    } catch (e) {
      const m = (e as Error).message;
      if (m.includes("changed on disk")) setConflict(rel);
      onNotice(m);
    }
  };

  /** Replaces the editor's copy with the file on disk (after a conflict). Local edits are dropped. */
  const reload = async (rel: string) => {
    const model = monaco.editor.getModel(uriFor(rel));
    if (!model) return;
    try {
      const f = await readFile(rel);
      model.setValue(f.text);
      savedAlt.current.set(rel, model.getAlternativeVersionId());
      setTabs((ts) => ts.map((t) => (t.rel === rel ? { ...t, version: f.version, dirty: false } : t)));
      setConflict(null);
      onNotice(`Reloaded ${baseName(rel)} from disk`);
    } catch (e) {
      onNotice((e as Error).message);
    }
  };

  const close = (rel: string) => {
    const tab = tabsRef.current.find((t) => t.rel === rel);
    // A dirty tab needs a second click: the first one only warns, so work is never lost by one stray click.
    if (tab?.dirty && !closeArmed.current.has(rel)) {
      closeArmed.current.add(rel);
      onNotice(`${baseName(rel)} has unsaved changes. Click close again to discard them.`);
      return;
    }
    closeArmed.current.delete(rel);
    monaco.editor.getModel(uriFor(rel))?.dispose();
    savedAlt.current.delete(rel);
    const rest = tabsRef.current.filter((t) => t.rel !== rel);
    setTabs(rest);
    if (activeRef.current === rel) {
      const next = rest.at(-1)?.rel ?? null;
      if (next) show(next);
      else {
        editor.current?.setModel(null);
        setActive(null);
        onActiveChange(null);
      }
    }
  };
  const closeArmed = useRef<Set<string>>(new Set());

  const reset = () => {
    editor.current?.setModel(null);
    for (const m of monaco.editor.getModels()) m.dispose();
    savedAlt.current.clear();
    setTabs([]);
    setActive(null);
    setConflict(null);
    onActiveChange(null);
  };

  const fileWritten = (rel: string) => {
    const tab = tabsRef.current.find((t) => t.rel === rel);
    if (!tab) return;
    if (tab.dirty) setConflict(rel);
    else void reload(rel);
  };

  useImperativeHandle(ref, () => ({ open, reset, fileWritten }));

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-ide-bg">
      <div role="tablist" aria-label="Open files" className="flex h-[35px] shrink-0 overflow-x-auto bg-ide-side">
        {tabs.map((t) => (
          <div key={t.rel} className={`group flex items-center border-r border-ide-line ${t.rel === active ? "border-t border-t-ide-focus bg-ide-bg text-white" : "text-ide-dim"}`}>
            <button type="button" role="tab" aria-selected={t.rel === active} title={t.rel} onClick={() => show(t.rel)} className="h-full pl-3 pr-1">
              {baseName(t.rel)}
            </button>
            <button type="button" aria-label={t.dirty ? `Close ${baseName(t.rel)} (unsaved)` : `Close ${baseName(t.rel)}`} onClick={() => close(t.rel)} className="mr-1 rounded p-1 hover:bg-ide-hover">
              {t.dirty ? <Circle size={10} fill="currentColor" aria-hidden /> : <X size={14} aria-hidden />}
            </button>
          </div>
        ))}
      </div>
      {active ? (
        <div className="shrink-0 border-b border-ide-line px-4 py-1 text-xs text-ide-dim">{crumbs(active).join(" › ")}</div>
      ) : null}
      {conflict ? (
        <div role="alert" className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[#3c3c3c] bg-[#3a2d1a] px-4 py-2 text-ide-bright">
          <span>{baseName(conflict)} changed on disk since you opened it. Your save was not written.</span>
          <span className="flex-1" />
          <button type="button" onClick={() => void reload(conflict)} className="rounded bg-ide-accent px-3 py-1 text-white">Reload from disk (drop my edits)</button>
          <button type="button" onClick={() => setConflict(null)} className="rounded border border-[#3c3c3c] px-3 py-1">Keep editing</button>
        </div>
      ) : null}
      <div className="relative min-h-0 flex-1">
        <div ref={host} className="absolute inset-0" />
        {!active ? (
          <div className="absolute inset-0 flex items-center justify-center text-ide-dim">Open a file from the Explorer. Ctrl+S saves.</div>
        ) : null}
      </div>
    </div>
  );
}
