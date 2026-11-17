/**
 * DiffReview.tsx -- side-by-side review of one change the assistant proposed (phase I3, docs/features/agent-tools.md).
 *
 * Purpose: shows the file as it is on disk (left) and as the assistant proposes it (right) in Monaco's diff editor,
 * over the editor area, with Accept and Reject. Accept writes the file through the core with the version check;
 * nothing is written before the person presses it. Phase I5 reuses it read-only (no Accept or Reject) for the Source
 * Control view's diff of a changed file against the last commit, with its own labels.
 * Trust boundary: both texts are untrusted (file content and model output). Monaco renders them as text in
 * read-only models; links are off; nothing in them is run (CWE-79, OWASP LLM01/LLM06).
 * Dependencies: monaco-editor (already loaded by monaco.ts).
 */
import { useEffect, useRef } from "react";
import { Check, X } from "lucide-react";
import { monaco } from "../monaco.ts";
import { languageFor } from "../lib/paths.ts";
import type { ChangeText } from "../ipc.ts";

interface Props {
  change: ChangeText;
  /** Without onAccept and onReject the diff is view-only (Source Control). */
  onAccept?: () => void;
  onReject?: () => void;
  onClose: () => void;
  /** Title prefix and the left/right legend; defaults describe a proposed change. */
  title?: string;
  legend?: string;
}

export function DiffReview({ change, onAccept, onReject, onClose, title, legend }: Props) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!host.current) return;
    const lang = languageFor(change.rel);
    // Private in-memory URIs: these models are not the editor's file models and are disposed on close.
    const original = monaco.editor.createModel(change.before, lang, monaco.Uri.from({ scheme: "p2preview", path: `/before/${change.rel}` }));
    const modified = monaco.editor.createModel(change.after, lang, monaco.Uri.from({ scheme: "p2preview", path: `/after/${change.rel}` }));
    const diff = monaco.editor.createDiffEditor(host.current, {
      theme: "p2p-dark",
      automaticLayout: true,
      readOnly: true,
      originalEditable: false,
      renderSideBySide: true,
      // SECURITY: no link opening from reviewed text.
      links: false,
      fontSize: 13,
    });
    diff.setModel({ original, modified });
    return () => {
      diff.dispose();
      original.dispose();
      modified.dispose();
    };
  }, [change]);

  return (
    <div role="dialog" aria-label={`Review change to ${change.rel}`} className="absolute inset-0 z-10 flex flex-col bg-ide-bg">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-ide-line bg-ide-side px-3 py-1.5">
        <span className="truncate text-ide-bright" title={change.rel}>{title ?? (change.created ? "New file: " : "Change: ")}{change.rel}</span>
        <span className="text-xs text-ide-dim">{legend ?? "left: on disk · right: proposed (not written yet)"}</span>
        <span className="flex-1" />
        {onAccept ? <button type="button" onClick={onAccept} className="flex items-center gap-1 rounded bg-ide-accent px-3 py-1 text-white"><Check size={14} aria-hidden />Accept and save</button> : null}
        {onReject ? <button type="button" onClick={onReject} className="flex items-center gap-1 rounded border border-ide-line px-3 py-1"><X size={14} aria-hidden />Reject</button> : null}
        <button type="button" onClick={onClose} className="rounded px-2 py-1 hover:bg-ide-hover">Close</button>
      </div>
      <div ref={host} aria-label="Diff" className="min-h-0 flex-1" />
    </div>
  );
}
