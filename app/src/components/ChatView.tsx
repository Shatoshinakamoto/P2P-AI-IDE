/**
 * ChatView.tsx -- the Agent chat side bar (phase I2: chat without tools, docs/features/gateways-and-chat.md).
 *
 * Purpose: chat with a model on the active gateway. Answers stream in; every chat is saved in this folder's history
 * (on this computer) and can be reopened, searched and deleted, like Hermes or OpenClaw. The agent's tools and diff
 * review arrive in phase I3, so in I2 the model cannot read or change files.
 * Trust boundary: model answers are untrusted text. They are rendered with React text nodes only (code fences in a
 * <pre>), never as HTML or Markdown-to-HTML, so an answer cannot inject markup or script (CWE-79). Links in answers
 * are not clickable. Nothing an answer says is run.
 */
import { useEffect, useRef, useState } from "react";
import { History, Plus, Search, Send, Square, Trash2 } from "lucide-react";
import {
  chatDelete, chatLoad, chatSave, chatSend, chatsList, chatsSearch, chatStop, gatewayCheck,
  type Chat, type ChatHit, type ChatMessage, type ChatSummary, type GatewayView, type Msg,
} from "../ipc.ts";

interface Props {
  /** The active gateway, or null when none is set. */
  gateway: GatewayView | null;
  /** Changes when the open folder changes (each folder has its own history). */
  workspaceKey: string;
  onNotice: (m: string) => void;
  onOpenGateways: () => void;
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

export function ChatView({ gateway, workspaceKey, onNotice, onOpenGateways }: Props) {
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

  // A different folder has a different history: start fresh.
  useEffect(() => {
    setChat(empty("", ""));
    setHits(null);
    setShowHistory(false);
  }, [workspaceKey]);

  useEffect(() => { end.current?.scrollIntoView({ block: "end" }); }, [chat.messages]);

  const loadHistory = async () => {
    try { setHistory(await chatsList()); } catch (e) { onNotice((e as Error).message); }
  };

  const send = async () => {
    const text = draft.trim();
    if (!text || busy || !gateway || !model) return;
    setDraft("");
    const userMsg: ChatMessage = { role: "user", content: text, at: now() };
    const base: Chat = { ...chat, gateway: gateway.name, model, messages: [...chat.messages, userMsg] };
    // Only user and assistant turns go to the model; error lines stay local.
    const wire: Msg[] = base.messages.filter((m) => m.role !== "error").map((m) => ({ role: m.role as Msg["role"], content: m.content }));
    let answer = "";
    setChat({ ...base, messages: [...base.messages, { role: "assistant", content: "", at: now() }] });
    setBusy(true);
    let final: Chat;
    try {
      await chatSend(model, wire, (t) => {
        answer += t;
        setChat((c) => ({ ...c, messages: [...c.messages.slice(0, -1), { role: "assistant", content: answer, at: now() }] }));
      });
      final = { ...base, messages: [...base.messages, { role: "assistant", content: answer, at: now() }] };
    } catch (e) {
      const m = (e as Error).message;
      const msgs: ChatMessage[] = [...base.messages];
      if (answer) msgs.push({ role: "assistant", content: answer, at: now() });
      msgs.push({ role: "error", content: m === "stopped" ? "Stopped." : m, at: now() });
      final = { ...base, messages: msgs };
    } finally {
      setBusy(false);
    }
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
          {chat.messages.length === 0 ? <p className="text-ide-dim">Ask about your code. In this phase the model sees only what you paste here; reading and editing files comes next.</p> : null}
          {chat.messages.map((m, i) => (
            <div key={i} className={m.role === "user" ? "self-end rounded bg-[#264f78] px-2 py-1 text-white" : m.role === "error" ? "text-xs text-[#f48771]" : "text-ide-bright"}>
              {m.role === "assistant" && m.content === "" && busy ? <span className="text-ide-dim">...</span> : <Text text={m.content} />}
            </div>
          ))}
          <div ref={end} />
        </div>
      )}
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
