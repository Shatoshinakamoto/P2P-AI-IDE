/**
 * SearchView.tsx -- workspace text search (VS Code's Search view). The query goes to the Rust core, which treats it as
 * plain text (no regular expressions, so no ReDoS, CWE-1333) and caps results. Previews are rendered as text (CWE-79).
 */
import { useState, type FormEvent } from "react";
import { search, type SearchResult } from "../ipc.ts";

interface Props {
  onOpenAt: (rel: string, line: number) => void;
  onError: (message: string) => void;
}

export function SearchView({ onOpenAt, onError }: Props) {
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<SearchResult | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (ev: FormEvent) => {
    ev.preventDefault();
    if (query.trim() === "") return;
    setBusy(true);
    try { setResult(await search(query)); } catch (e) { onError((e as Error).message); } finally { setBusy(false); }
  };

  const files = result ? new Set(result.matches.map((m) => m.rel)).size : 0;
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 px-3">
      <form onSubmit={run} className="flex flex-col gap-1">
        <label htmlFor="ws-search" className="text-xs text-ide-dim">Search in workspace (plain text)</label>
        <input
          id="ws-search"
          value={query}
          maxLength={256}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search"
          className="h-7 rounded border border-[#3c3c3c] bg-[#313131] px-2 text-ide-bright outline-none focus:border-ide-focus"
        />
      </form>
      {busy ? <div className="text-xs text-ide-dim">Searching…</div> : null}
      {result ? (
        <div className="text-xs text-ide-dim">
          {result.matches.length} {result.matches.length === 1 ? "result" : "results"} in {files} {files === 1 ? "file" : "files"}{result.truncated ? " (stopped at the limit)" : ""}
        </div>
      ) : null}
      <ul className="min-h-0 flex-1 overflow-auto pb-4">
        {result?.matches.map((m) => (
          <li key={`${m.rel}:${m.line}`}>
            <button type="button" onClick={() => onOpenAt(m.rel, m.line)} className="flex w-full flex-col items-start rounded px-1 py-1 text-left hover:bg-ide-hover">
              <span className="text-ide-bright">{m.rel}<span className="text-ide-dim">:{m.line}</span></span>
              <span className="w-full truncate font-mono text-xs text-ide-dim">{m.preview}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
