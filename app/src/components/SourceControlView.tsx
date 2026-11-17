/**
 * SourceControlView.tsx -- the Source Control view (phase I5, docs/features/source-control.md): the branch, changed
 * files split into Staged and Changes, a diff of each against the last commit, stage and unstage, a commit message
 * box with Commit, and the recent history. Like VS Code's Source Control view, without any network step (no pull,
 * push or fetch).
 *
 * Trust boundary: file names, commit subjects and author names come from git and the repository (untrusted); they
 * are rendered by React as text, never as HTML or links (CWE-79). Every action goes through ipc.ts to the Rust core,
 * which checks paths and runs git without hooks or repository programs.
 * Dependencies: React, lucide-react icons.
 */
import { useCallback, useEffect, useState } from "react";
import { Check, GitBranch, Minus, Plus, RefreshCw } from "lucide-react";
import { scmCommit, scmDiff, scmLog, scmStage, scmStatus, scmUnstage, type ScmCommit, type ScmDiff, type ScmFile, type ScmState } from "../ipc.ts";

interface Props {
  /** Bumped when another view wrote files, so the list reloads. */
  refreshTick: number;
  onNotice: (m: string) => void;
  onDiff: (d: ScmDiff | null) => void;
}

/** A short word for git's status letter. */
function word(c: string): string {
  switch (c) {
    case "M": return "modified";
    case "A": return "added";
    case "D": return "deleted";
    case "R": return "renamed";
    case "C": return "copied";
    case "T": return "type changed";
    case "U": return "conflict";
    case "?": return "new";
    default: return "";
  }
}

export function SourceControlView({ refreshTick, onNotice, onDiff }: Props) {
  const [state, setState] = useState<ScmState | null>(null);
  const [log, setLog] = useState<ScmCommit[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      const s = await scmStatus();
      setState(s);
      setLog(s.available ? await scmLog() : []);
    } catch (e) {
      onNotice((e as Error).message);
    }
  }, [onNotice]);

  useEffect(() => { void reload(); }, [reload, refreshTick]);

  const act = async (f: () => Promise<unknown>, done?: string) => {
    setBusy(true);
    try {
      await f();
      if (done) onNotice(done);
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setBusy(false);
      void reload();
    }
  };

  const showDiff = async (f: ScmFile) => {
    try { onDiff(await scmDiff(f.rel)); } catch (e) { onNotice((e as Error).message); }
  };

  const commit = () => act(async () => {
    const id = await scmCommit(message);
    setMessage("");
    onNotice(`Committed ${id}`);
  });

  if (!state) return <p className="px-3 text-ide-dim">Reading the repository…</p>;
  if (!state.available || !state.status) {
    return (
      <div className="flex flex-col gap-2 px-3">
        <p role="status" className="text-ide-dim">{state.reason}</p>
        <button type="button" onClick={() => void reload()} className="self-start rounded border border-ide-line px-2 py-1 hover:bg-ide-hover">Check again</button>
      </div>
    );
  }
  const st = state.status;
  const staged = st.files.filter((f) => !f.untracked && f.index !== "." && !f.conflict);
  const changes = st.files.filter((f) => f.untracked || f.work !== "." || f.conflict);

  const row = (f: ScmFile, side: "staged" | "changes") => {
    const letter = side === "staged" ? f.index : f.untracked ? "?" : f.conflict ? "U" : f.work;
    return (
      <li key={`${side}:${f.rel}`} className="group flex items-center gap-1 rounded px-1 hover:bg-ide-hover">
        <button type="button" disabled={!f.actionable} onClick={() => void showDiff(f)} title={f.rel}
          className="flex min-w-0 flex-1 items-center gap-2 py-0.5 text-left disabled:opacity-60">
          <span className="truncate text-ide-bright">{f.rel}</span>
          <span className="shrink-0 text-xs text-ide-dim">{word(letter)}</span>
        </button>
        {f.actionable ? (
          side === "staged" ? (
            <button type="button" aria-label={`Unstage ${f.rel}`} title="Unstage" disabled={busy}
              onClick={() => void act(() => scmUnstage(f.rel))} className="rounded p-0.5 hover:bg-ide-line"><Minus size={14} aria-hidden /></button>
          ) : (
            <button type="button" aria-label={`Stage ${f.rel}`} title="Stage" disabled={busy}
              onClick={() => void act(() => scmStage(f.rel))} className="rounded p-0.5 hover:bg-ide-line"><Plus size={14} aria-hidden /></button>
          )
        ) : null}
      </li>
    );
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto px-3 pb-4">
      <div className="flex items-center gap-2">
        <GitBranch size={14} aria-hidden />
        <span className="truncate text-ide-bright">{st.branch || "detached"}</span>
        {st.ahead || st.behind ? <span className="text-xs text-ide-dim">↑{st.ahead} ↓{st.behind}</span> : null}
        <span className="flex-1" />
        <button type="button" aria-label="Refresh" title="Refresh" onClick={() => void reload()} className="rounded p-1 hover:bg-ide-hover"><RefreshCw size={14} aria-hidden /></button>
      </div>
      <label htmlFor="scm-msg" className="text-xs text-ide-dim">Commit message</label>
      <textarea id="scm-msg" value={message} maxLength={5000} rows={3} onChange={(e) => setMessage(e.target.value)}
        placeholder={st.initial ? "First commit" : "Describe the change"}
        className="rounded border border-[#3c3c3c] bg-[#313131] px-2 py-1 text-ide-bright outline-none focus:border-ide-focus" />
      <button type="button" disabled={busy || message.trim() === "" || staged.length === 0} onClick={() => void commit()}
        className="flex items-center justify-center gap-1 rounded bg-ide-accent px-3 py-1 text-white disabled:opacity-50">
        <Check size={14} aria-hidden />Commit{staged.length ? ` ${staged.length} staged` : ""}
      </button>
      <p className="text-xs text-ide-dim">Commits stay on this computer. Hooks in the repository are not run.</p>

      <section aria-label="Staged changes">
        <div className="flex items-center text-[11px] uppercase tracking-wide text-ide-dim">
          <span className="flex-1">Staged ({staged.length})</span>
          {staged.length ? <button type="button" disabled={busy} onClick={() => void act(() => scmUnstage(null))} className="rounded px-1 normal-case hover:bg-ide-hover">Unstage all</button> : null}
        </div>
        <ul>{staged.map((f) => row(f, "staged"))}</ul>
      </section>
      <section aria-label="Changes">
        <div className="flex items-center text-[11px] uppercase tracking-wide text-ide-dim">
          <span className="flex-1">Changes ({changes.length})</span>
          {changes.length ? <button type="button" disabled={busy} onClick={() => void act(() => scmStage(null))} className="rounded px-1 normal-case hover:bg-ide-hover">Stage all</button> : null}
        </div>
        <ul>{changes.map((f) => row(f, "changes"))}</ul>
        {st.truncated ? <p className="text-xs text-ide-dim">Only the first 5000 changed files are listed.</p> : null}
        {st.files.length === 0 ? <p className="text-xs text-ide-dim">No changes.</p> : null}
      </section>
      <section aria-label="History">
        <div className="text-[11px] uppercase tracking-wide text-ide-dim">History</div>
        {log.length === 0 ? <p className="text-xs text-ide-dim">No commits yet.</p> : null}
        <ul>
          {log.map((c) => (
            <li key={c.id} className="flex gap-2 py-0.5 text-xs">
              <span className="shrink-0 font-mono text-ide-dim">{c.id}</span>
              <span className="truncate text-ide-bright" title={c.subject}>{c.subject}</span>
              <span className="ml-auto shrink-0 text-ide-dim">{c.author}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
