/**
 * ChatView.tsx -- the Agent chat side bar (phase I2: chat without tools, docs/features/gateways-and-chat.md).
 *
 * Purpose: chat with a model on the active gateway. Answers stream in; every chat is saved in this folder's history
 * (on this computer) and can be reopened, searched and deleted, like Hermes or OpenClaw. The agent's tools and diff
 * review arrived in phase I3 (docs/features/agent-tools.md): with **Tools** on and a folder open, the model can list,
 * read and search the folder and propose edits; proposed edits wait in **Changes** until the person reviews the diff
 * and accepts or rejects each file. Every tool step is shown in the chat.
 * Trust boundary: model answers are untrusted text. They are rendered with React text nodes only (code fences in a
 * <pre>), never as HTML or Markdown-to-HTML, so an answer cannot inject markup or script (CWE-79). Links in answers
 * are not clickable. Nothing an answer says is run. Tool lines show only the tool name and its target as text.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Eye, FileDiff, History, Plus, Search, Send, Square, Trash2, Wrench, X } from "lucide-react";
import {
  agentSend, changeAccept, changeGet, changeReject, changesList, chatDelete, chatLoad, chatSave, chatSend, chatsList,
  chatsSearch, chatStop, gatewayCheck,
  type ChangeText, type ChangeView, type Chat, type ChatHit, type ChatMessage, type ChatSummary, type GatewayView, type Msg,
} from "../ipc.ts";

/** A change open in the diff view, with what Accept and Reject do. */
export interface Review { change: ChangeText; accept: () => void; reject: () => void }

interface Props {
  /** The active gateway, or null when none is set. */
  gateway: GatewayView | null;
  /** Changes when the open folder changes (each folder has its own history). */
  workspaceKey: string;
  onNotice: (m: string) => void;
  onOpenGateways: () => void;
  /** A folder is open (the tools work on it). */
  hasFolder: boolean;
  /** Opens (or closes, with null) the diff view over the editor area. */
  onReview: (r: Review | null) => void;
  /** An accepted change was written to this file. */
  onFileWritten: (rel: string) => void;
}

const empty = (gateway: string, model: string): Chat => ({ id: "", title: "", gateway, model, created: 0, updated: 0, messages: [] });
const now = () => Math.floor(Date.now() / 1000);

/** Renders untrusted answer text: plain text, with ``` fenced blocks shown as code. React escapes everything. */
function Text({ text }: { text: string }) {
  const parts = text.split("```");
  return (
    <>
      {parts.map((p, i) =>
        i % 2 === 1 ? (
          <pre key={i} className="my-1 overflow-x-auto rounded bg-ide-bg p-2 font-mono text-xs">{p.replace(/^[\w+-]*\n/, "")}</pre>
        ) : (
          <span key={i} className="whitespace-pre-wrap break-words">{p}</span>
        ),
      )}
    </>
  );
}

export function ChatView({ gateway, workspaceKey, onNotice, onOpenGateways, hasFolder, onReview, onFileWritten }: Props) {
  const [tools, setTools] = useState(true);
  const [changes, setChanges] = useState<ChangeView[]>([]);
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState("");
  const [chat, setChat] = useState<Chat>(empty("", ""));
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [history, setHistory] = useState<ChatSummary[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ChatHit[] | null>(null);
  const [deleteArmed, setDeleteArmed] = useState<string | null>(null);
  const end = useRef<HTMLDivElement>(null);

  // Load the active gateway's models (and keep the last choice when it is still offered).
  useEffect(() => {
    setModels([]);
    if (!gateway?.has_key) return;
    gatewayCheck(gateway.id)
      .then((r) => {
        setModels(r.models);
        setModel((m) => (r.models.includes(m) ? m : (r.models[0] ?? "")));
      })
      .catch((e) => onNotice((e as Error).message));
  }, [gateway?.id, gateway?.has_key, onNotice]);

  const refreshChanges = useCallback(async () => {
    try { setChanges(await changesList()); } catch { setChanges([]); }
  }, []);

  // A different folder has a different history and no pending changes: start fresh.
  useEffect(() => {
    setChat(empty("", ""));
    setHits(null);
    setShowHistory(false);
    void refreshChanges();
  }, [workspaceKey, refreshChanges]);

  const accept = async (rel: string) => {
    try {
      await changeAccept(rel);
      onFileWritten(rel);
      onReview(null);
      onNotice(`Saved the change to ${rel}`);
    } catch (e) {
      onNotice((e as Error).message);
    }
    await refreshChanges();
  };
  const reject = async (rel: string) => {
    try { await changeReject(rel); onReview(null); onNotice(`Rejected the change to ${rel}`); } catch (e) { onNotice((e as Error).message); }
    await refreshChanges();
  };
  const review = async (rel: string) => {
    try {
      const change = await changeGet(rel);
      onReview({ change, accept: () => void accept(rel), reject: () => void reject(rel) });
    } catch (e) {
      onNotice((e as Error).message);
      await refreshChanges();
    }
  };

  useEffect(() => { end.current?.scrollIntoView({ block: "end" }); }, [chat.messages]);

  const loadHistory = async () => {
    try { setHistory(await chatsList()); } catch (e) { onNotice((e as Error).message); }
  };

  const send = async () => {
    const text = draft.trim();
    if (!text || busy || !gateway || !model) return;
    setDraft("");
    const useTools = tools && hasFolder;
    const userMsg: ChatMessage = { role: "user", content: text, at: now() };
    const base: Chat = { ...chat, gateway: gateway.name, model, messages: [...chat.messages, userMsg] };
    // Only user and assistant text goes to the model; error and tool lines stay local.
    const wire: Msg[] = base.messages
      .filter((m) => (m.role === "user" || m.role === "assistant") && m.content !== "")
      .map((m) => ({ role: m.role as Msg["role"], content: m.content }));
    // The turn's messages after the user's: assistant text pieces and tool lines, in order.
    const turn: ChatMessage[] = [{ role: "assistant", content: "", at: now() }];
    const show = () => setChat({ ...base, messages: [...base.messages, ...turn] });
    show();
    setBusy(true);
    let failed: string | null = null;
    try {
      if (useTools) {
        await agentSend(model, wire, (e) => {
          if (e.kind === "delta") {
            const last = turn[turn.length - 1];
            if (last.role === "assistant") turn[turn.length - 1] = { ...last, content: last.content + e.text, at: now() };
            else turn.push({ role: "assistant", content: e.text, at: now() });
          } else {
            // SECURITY: only the tool name and target are shown, as text.
            turn.push({ role: "tool", content: `${e.ok ? "" : "failed: "}${e.name} ${e.target}`.trim(), at: now() });
          }
          show();
        });
      } else {
        await chatSend(model, wire, (t) => {
          turn[0] = { ...turn[0], content: turn[0].content + t, at: now() };
          show();
        });
      }
    } catch (e) {
      failed = (e as Error).message;
    } finally {
      setBusy(false);
    }
    const kept = turn.filter((m) => !(m.role === "assistant" && m.content === ""));
    if (failed) kept.push({ role: "error", content: failed === "stopped" ? "Stopped." : failed, at: now() });
    const final: Chat = { ...base, messages: [...base.messages, ...kept] };
    if (useTools) await refreshChanges();
    try {
      const saved = await chatSave(final);
      setChat(saved);
    } catch (e) {
      setChat(final);
      onNotice((e as Error).message);
    }
  };

  const open = async (id: string) => {
    try {
      const c = await chatLoad(id);
      setChat(c);
      if (models.includes(c.model)) setModel(c.model);
      setShowHistory(false);
      setHits(null);
    } catch (e) {
      onNotice((e as Error).message);
    }
  };

  const remove = async (id: string) => {
    if (deleteArmed !== id) { setDeleteArmed(id); onNotice("Click delete again to remove that chat"); return; }
    setDeleteArmed(null);
    try {
      await chatDelete(id);
      if (chat.id === id) setChat(empty("", ""));
      setHits((h) => h?.filter((x) => x.id !== id) ?? null);
      onNotice("Chat deleted");
    } catch (e) {
      onNotice((e as Error).message);
    }
    await loadHistory();
  };

  if (!gateway) {
    return (
      <div className="flex flex-col gap-2 px-3">
        <p className="text-ide-dim">Chat needs a gateway. Add one, then pick a model here.</p>
        <button type="button" onClick={onOpenGateways} className="self-start rounded bg-ide-accent px-3 py-1 text-white">Open Gateways</button>
      </div>
    );
  }

  const list = hits ?? history.map((h) => ({ id: h.id, title: h.title, preview: `${h.count} messages` }));

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-1 px-3 pb-2">
        <select aria-label="Model" value={model} onChange={(e) => setModel(e.target.value)} className="min-w-0 flex-1 rounded border border-ide-line bg-ide-bg px-1 py-1 text-ide-bright">
          {models.length === 0 ? <option value="">{gateway.has_key ? "no models yet" : "add a key first"}</option> : null}
          {models.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        <button
          type="button"
          aria-label="Tools"
          aria-pressed={tools && hasFolder}
          disabled={!hasFolder}
          title={hasFolder ? (tools ? "Tools on: the assistant can read and search this folder and propose edits for your review" : "Tools off: plain chat") : "Open a folder to use tools"}
          onClick={() => setTools(!tools)}
          className={`flex items-center gap-1 rounded px-1.5 py-1 text-xs disabled:opacity-40 ${tools && hasFolder ? "bg-[#264f78] text-white" : "hover:bg-ide-hover"}`}
        ><Wrench size={13} aria-hidden />Tools</button>
        <button type="button" aria-label="New chat" title="New chat" onClick={() => setChat(empty("", ""))} className="rounded p-1 hover:bg-ide-hover"><Plus size={16} aria-hidden /></button>
        <button type="button" aria-label="Chat history" aria-pressed={showHistory} title="History" onClick={() => { setShowHistory(!showHistory); void loadHistory(); }} className="rounded p-1 hover:bg-ide-hover"><History size={16} aria-hidden /></button>
      </div>
      {showHistory ? (
        <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-3">
          <form className="flex gap-1" onSubmit={(e) => { e.preventDefault(); if (!query.trim()) { setHits(null); return; } chatsSearch(query).then(setHits).catch((er) => onNotice((er as Error).message)); }}>
            <input aria-label="Search chats" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search chats" className="min-w-0 flex-1 rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright" />
            <button type="submit" aria-label="Search" className="rounded p-1 hover:bg-ide-hover"><Search size={14} aria-hidden /></button>
          </form>
          {list.length === 0 ? <p className="text-ide-dim">{hits ? "No chats match." : "No chats yet in this folder."}</p> : null}
          <ul aria-label="Saved chats" className="flex flex-col">
            {list.map((h) => (
              <li key={h.id} className="group flex items-start gap-1 rounded hover:bg-ide-hover">
                <button type="button" onClick={() => void open(h.id)} className="min-w-0 flex-1 px-2 py-1 text-left">
                  <div className="truncate text-ide-bright">{h.title}</div>
                  <div className="truncate text-xs text-ide-dim">{h.preview}</div>
                </button>
                <button type="button" aria-label={deleteArmed === h.id ? `Confirm delete ${h.title}` : `Delete ${h.title}`} onClick={() => void remove(h.id)} className={`p-1 ${deleteArmed === h.id ? "text-[#f48771]" : "text-ide-dim"}`}><Trash2 size={14} aria-hidden /></button>
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <div aria-label="Messages" role="log" className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-3">
          {chat.messages.length === 0 ? (
            <p className="text-ide-dim">
              {tools && hasFolder
                ? "Ask about your code. With Tools on, the assistant can read and search this folder and propose edits; nothing is saved until you review and accept it."
                : "Ask about your code. With Tools off (or no folder open) the model sees only what you paste here."}
            </p>
          ) : null}
          {chat.messages.map((m, i) => (
            <div key={i} className={m.role === "user" ? "self-end rounded bg-[#264f78] px-2 py-1 text-white" : m.role === "error" ? "text-xs text-[#f48771]" : m.role === "tool" ? "flex items-center gap-1 font-mono text-xs text-ide-dim" : "text-ide-bright"}>
              {m.role === "tool" ? <Wrench size={11} aria-hidden /> : null}
              {m.role === "assistant" && m.content === "" && busy ? <span className="text-ide-dim">...</span> : <Text text={m.content} />}
            </div>
          ))}
          <div ref={end} />
        </div>
      )}
      {changes.length > 0 ? (
        <section aria-label="Changes" className="flex max-h-48 shrink-0 flex-col gap-1 overflow-y-auto border-t border-ide-line px-3 py-2">
          <h3 className="flex items-center gap-1 text-xs font-semibold uppercase text-ide-dim"><FileDiff size={12} aria-hidden />Changes to review ({changes.length})</h3>
          <ul aria-label="Pending changes" className="flex flex-col">
            {changes.map((c) => (
              <li key={c.rel} className="flex items-center gap-1 text-xs">
                <span className="min-w-0 flex-1 truncate text-ide-bright" title={c.rel}>{c.rel}</span>
                <span className="text-[#89d185]">{c.created ? "new" : `+${c.added}`}</span>
                {c.created ? null : <span className="text-[#f48771]">-{c.removed}</span>}
                <button type="button" aria-label={`Review ${c.rel}`} title="Review the diff" onClick={() => void review(c.rel)} className="rounded p-1 hover:bg-ide-hover"><Eye size={13} aria-hidden /></button>
                <button type="button" aria-label={`Accept ${c.rel}`} title="Accept and save" onClick={() => void accept(c.rel)} className="rounded p-1 text-[#89d185] hover:bg-ide-hover"><Check size={13} aria-hidden /></button>
                <button type="button" aria-label={`Reject ${c.rel}`} title="Reject" onClick={() => void reject(c.rel)} className="rounded p-1 text-[#f48771] hover:bg-ide-hover"><X size={13} aria-hidden /></button>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <form className="flex flex-col gap-1 border-t border-ide-line p-2" onSubmit={(e) => { e.preventDefault(); void send(); }}>
        <textarea
          aria-label="Message"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); void send(); } }}
          rows={3}
          placeholder={model ? `Message ${model} (Ctrl+Enter to send)` : "Pick a model first"}
          className="resize-none rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright"
        />
        <div className="flex items-center gap-2 text-xs text-ide-dim">
          <span className="truncate">{gateway.name}{gateway.dev ? " (INSECURE — DEV ONLY)" : ""}</span>
          <span className="flex-1" />
          {busy ? (
            <button type="button" onClick={() => void chatStop()} className="flex items-center gap-1 rounded border border-ide-line px-2 py-1 text-ide-bright"><Square size={12} aria-hidden />Stop</button>
          ) : (
            <button type="submit" disabled={!model || !draft.trim()} className="flex items-center gap-1 rounded bg-ide-accent px-2 py-1 text-white disabled:opacity-50"><Send size={12} aria-hidden />Send</button>
          )}
        </div>
      </form>
    </div>
  );
}
