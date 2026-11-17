/**
 * Explorer.tsx -- the workspace file tree (VS Code's Explorer view). Folders load when expanded.
 *
 * Data comes only from the Rust core's list_dir; names are rendered as text by React (escaped, CWE-79). Symlinks are
 * shown with their own icon; opening one still goes through the core's confinement check.
 */
import { useCallback, useEffect, useState } from "react";
import { ChevronDown, ChevronRight, File, Folder, FolderOpen, Link2, RefreshCw } from "lucide-react";
import { listDir, type Entry, type Listing } from "../ipc.ts";
import { sizeLabel } from "../lib/paths.ts";

interface Props {
  workspaceName: string;
  activeRel: string | null;
  onOpenFile: (rel: string) => void;
  onError: (message: string) => void;
}

/** The tree for one workspace. `listings` caches each loaded folder by its relative path. */
export function Explorer({ workspaceName, activeRel, onOpenFile, onError }: Props) {
  const [listings, setListings] = useState<Map<string, Listing>>(new Map());
  const [expanded, setExpanded] = useState<Set<string>>(new Set([""]));

  const load = useCallback(async (rel: string) => {
    try {
      const l = await listDir(rel);
      setListings((m) => new Map(m).set(rel, l));
    } catch (e) {
      onError((e as Error).message);
    }
  }, [onError]);

  // Load the root when the workspace changes.
  useEffect(() => {
    setListings(new Map());
    setExpanded(new Set([""]));
    void load("");
  }, [workspaceName, load]);

  const toggle = (e: Entry) => {
    setExpanded((s) => {
      const next = new Set(s);
      if (next.has(e.rel)) next.delete(e.rel);
      else {
        next.add(e.rel);
        if (!listings.has(e.rel)) void load(e.rel);
      }
      return next;
    });
  };

  /** Reloads every expanded folder (after files change outside the IDE). */
  const refresh = () => { for (const rel of expanded) void load(rel); };

  const renderDir = (rel: string, depth: number) => {
    const l = listings.get(rel);
    if (!l) return <div className="px-4 py-1 text-ide-dim" style={{ paddingLeft: 8 + depth * 12 }}>Loading…</div>;
    return (
      <>
        {l.entries.map((e) => {
          const open = expanded.has(e.rel);
          const active = e.rel === activeRel;
          return (
            <div key={e.rel}>
              <button
                type="button"
                title={e.kind === "file" ? `${e.rel} (${sizeLabel(e.size)})` : e.rel}
                onClick={() => (e.kind === "dir" ? toggle(e) : onOpenFile(e.rel))}
                className={`flex w-full items-center gap-1 py-[3px] pr-2 text-left hover:bg-ide-hover ${active ? "bg-ide-sel text-white" : ""}`}
                style={{ paddingLeft: 8 + depth * 12 }}
                aria-expanded={e.kind === "dir" ? open : undefined}
              >
                {e.kind === "dir" ? (open ? <ChevronDown size={14} aria-hidden /> : <ChevronRight size={14} aria-hidden />) : <span className="w-[14px]" />}
                {e.kind === "dir" ? (open ? <FolderOpen size={15} className="text-[#dcb67a]" aria-hidden /> : <Folder size={15} className="text-[#dcb67a]" aria-hidden />)
                  : e.kind === "symlink" ? <Link2 size={15} className="text-ide-dim" aria-hidden />
                  : <File size={15} className="text-[#75beff]" aria-hidden />}
                <span className="truncate">{e.name}</span>
              </button>
              {e.kind === "dir" && open ? renderDir(e.rel, depth + 1) : null}
            </div>
          );
        })}
        {l.truncated ? <div className="px-4 py-1 text-xs text-ide-dim" style={{ paddingLeft: 8 + depth * 12 }}>Only the first 5000 entries are shown.</div> : null}
      </>
    );
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between px-3 py-1">
        <span className="truncate text-[11px] font-semibold uppercase tracking-wide text-ide-bright">{workspaceName}</span>
        <button type="button" onClick={refresh} aria-label="Refresh explorer" className="rounded p-1 hover:bg-ide-hover"><RefreshCw size={14} aria-hidden /></button>
      </div>
      <nav aria-label="Files" className="min-h-0 flex-1 overflow-auto pb-4">{renderDir("", 0)}</nav>
    </div>
  );
}
