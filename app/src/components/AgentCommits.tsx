/**
 * AgentCommits.tsx -- "From the agent" in the Source Control view (docs/features/agent-commits.md): commits an agent
 * asked for with git_commit mode "local". Each shows its message and diff; the person commits it in the open folder
 * (with their own git identity) or rejects it. Nothing happens to the repository without the Commit click.
 *
 * Trust boundary: the message and the diff come from the gateway (untrusted). They are rendered by React as text in
 * <pre> blocks, never as HTML or links (CWE-79). The Rust side checks the patch again right before applying it.
 * Dependencies: React, lucide-react icons.
 */
import { useCallback, useEffect, useState } from "react";
import { Bot, Check, X } from "lucide-react";
import { agentCommitApply, agentCommitDiff, agentCommitReject, agentCommitsPending, type AgentCheckpoint, type AgentCommitDiff } from "../ipc.ts";

interface Props {
  /** Bumped when the view reloads. */
  refreshTick: number;
  onNotice: (m: string) => void;
  /** Called after a commit, so the history and status reload. */
  onCommitted: () => void;
}

/** How often the list is read again while the view is open (the agent may commit at any time). */
const POLL_MS = 30_000;

export function AgentCommits({ refreshTick, onNotice, onCommitted }: Props) {
  const [items, setItems] = useState<AgentCheckpoint[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [diff, setDiff] = useState<AgentCommitDiff | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try { setItems(await agentCommitsPending()); } catch { setItems([]); /* not linked or gateway away: show nothing */ }
  }, []);

  useEffect(() => {
    void reload();
    const t = setInterval(() => void reload(), POLL_MS);
    return () => clearInterval(t);
  }, [reload, refreshTick]);

  const toggle = async (c: AgentCheckpoint) => {
    if (open === c.id) { setOpen(null); setDiff(null); return; }
    setOpen(c.id);
    setDiff(null);
    try { setDiff(await agentCommitDiff(c.id)); } catch (e) { onNotice((e as Error).message); }
  };

  const decide = async (c: AgentCheckpoint, yes: boolean) => {
    setBusy(true);
    try {
      if (yes) {
        const id = await agentCommitApply(c.id);
        onNotice(`Committed the agent's change as ${id.slice(0, 12)}`);
        onCommitted();
      } else {
        await agentCommitReject(c.id);
        onNotice("Rejected the agent's commit; the agent is told");
      }
      setOpen(null);
      setDiff(null);
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setBusy(false);
      void reload();
    }
  };

  if (items.length === 0) return null;
  return (
    <section aria-label="Commits from the agent">
      <div className="flex items-center gap-1 text-[11px] uppercase tracking-wide text-ide-dim">
        <Bot size={12} aria-hidden /><span>From the agent ({items.length})</span>
      </div>
      <ul>
        {items.map((c) => (
          <li key={c.id} className="rounded border border-ide-line px-2 py-1">
            <button type="button" onClick={() => void toggle(c)} aria-expanded={open === c.id}
              className="flex w-full items-center gap-2 text-left">
              <span className="truncate text-ide-bright" title={c.subject}>{c.subject || "(no message)"}</span>
              <span className="ml-auto shrink-0 text-xs text-ide-dim">version {c.version}</span>
            </button>
            {open === c.id ? (
              <div className="mt-1 flex flex-col gap-1">
                <pre className="max-h-40 overflow-auto whitespace-pre-wrap text-xs text-ide-bright">{c.message}</pre>
                {diff ? (
                  <>
                    <p className="text-xs text-ide-dim">{diff.files.length} file(s): {diff.files.join(", ")}</p>
                    <pre aria-label="Diff" className="max-h-72 overflow-auto rounded bg-[#1e1e1e] p-1 text-xs">{diff.diff}</pre>
                  </>
                ) : <p className="text-xs text-ide-dim">Reading the change…</p>}
                <div className="flex gap-2">
                  <button type="button" disabled={busy || !diff} onClick={() => void decide(c, true)}
                    className="flex items-center gap-1 rounded bg-ide-accent px-2 py-1 text-white disabled:opacity-50">
                    <Check size={14} aria-hidden />Commit
                  </button>
                  <button type="button" disabled={busy} onClick={() => void decide(c, false)}
                    className="flex items-center gap-1 rounded border border-ide-line px-2 py-1 hover:bg-ide-hover disabled:opacity-50">
                    <X size={14} aria-hidden />Reject
                  </button>
                </div>
                <p className="text-xs text-ide-dim">Commit applies the change to this folder and commits it with your git name. Your own staged changes must be committed or unstaged first.</p>
              </div>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
