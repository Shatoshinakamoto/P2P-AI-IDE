/**
 * NetworkView.tsx -- the Gateways side bar (phase I2, docs/features/gateways-and-chat.md).
 *
 * Purpose: lists saved gateways, picks the active one, checks it (latency and model count), adds a gateway by URL
 * or the one set up on this computer, pins a CA file, sets or replaces a key, and removes a gateway. Below it, the
 * public gateway list (phase I2b, PublicGateways.tsx).
 * Trust boundary: everything goes through ipc.ts to the Rust core, which checks it. The key field is a password
 * input; its value is sent once to the core (which writes it to the OS keychain) and then cleared from the form.
 * Keys never come back to the webview (only `has_key`). Names and URLs are rendered as React text (CWE-79).
 * Phase A3: a gateway can instead be linked to a P2P-AI account (AccountsPanel.tsx): the card then shows the account
 * and the core asks the account service for a short token before each call. The link holds no secret.
 */
import { useEffect, useState } from "react";
import { CheckCircle2, KeyRound, Lock, Plus, RefreshCw, ShieldAlert, Trash2, UserRound } from "lucide-react";
import {
  gatewayAdd, gatewayAddLocal, gatewayCheck, gatewayClearCa, gatewayPickCa, gatewayRemove, gatewaySetActive,
  gatewaySetAccount, gatewaySetKey, gatewaysList, type AccountHome, type GatewayCheck, type GatewayList,
} from "../ipc.ts";
import { AccountsPanel } from "./AccountsPanel.tsx";
import { PublicGateways } from "./PublicGateways.tsx";
import { NexusConnect } from "./NexusConnect.tsx";
import { TandemForgeConnect } from "./TandemForgeConnect.tsx";
import { Tip } from "./Tip.tsx";

interface Props {
  /** Called after any change, so the status bar and chat can refresh. */
  onChanged: (list: GatewayList) => void;
  onNotice: (m: string) => void;
}

const input = "w-full rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright placeholder:text-ide-dim";
const btn = "rounded px-2 py-1 hover:bg-ide-hover";

export function NetworkView({ onChanged, onNotice }: Props) {
  const [list, setList] = useState<GatewayList>({ gateways: [], active: null });
  const [checks, setChecks] = useState<Record<string, GatewayCheck | string>>({});
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [key, setKey] = useState("");
  const [keyFor, setKeyFor] = useState<string | null>(null);
  const [newKey, setNewKey] = useState("");
  const [removeArmed, setRemoveArmed] = useState<string | null>(null);
  const [homes, setHomes] = useState<AccountHome[]>([]);
  const [acctFor, setAcctFor] = useState<string | null>(null);
  const [acctHome, setAcctHome] = useState("");
  const [acctAud, setAcctAud] = useState("");

  const refresh = async () => {
    try {
      const l = await gatewaysList();
      setList(l);
      onChanged(l);
    } catch (e) {
      onNotice((e as Error).message);
    }
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { void refresh(); }, []);

  /** Runs an action, shows its message, refreshes the list. */
  const act = async (f: () => Promise<unknown>, done?: string) => {
    try {
      await f();
      if (done) onNotice(done);
    } catch (e) {
      onNotice((e as Error).message);
    }
    await refresh();
  };

  const check = async (id: string) => {
    setChecks((c) => ({ ...c, [id]: "checking..." }));
    try {
      const r = await gatewayCheck(id);
      setChecks((c) => ({ ...c, [id]: r }));
    } catch (e) {
      setChecks((c) => ({ ...c, [id]: (e as Error).message }));
    }
  };

  const add = async (local: boolean) => {
    const k = key.trim() === "" ? null : key;
    try {
      const id = local ? await gatewayAddLocal(name, k) : await gatewayAdd(name, url, k);
      if (id) {
        setAdding(false);
        setName(""); setUrl("");
        onNotice("Gateway added");
      }
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      // SECURITY: the key leaves the form as soon as it was sent (it now lives in the OS keychain only).
      setKey("");
    }
    await refresh();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-3 pb-3">
      {list.gateways.length === 0 && !adding ? (
        <p className="text-ide-dim">No gateway yet. Add the gateway on this computer, one by its address, or one from a public list below.</p>
      ) : null}
      <ul aria-label="Saved gateways" className="flex flex-col gap-2">
        {list.gateways.map((g) => {
          const c = checks[g.id];
          return (
            <li key={g.id} className={`rounded border p-2 ${list.active === g.id ? "border-ide-focus" : "border-ide-line"}`}>
              <label className="flex items-center gap-2">
                <input type="radio" name="active-gateway" checked={list.active === g.id} onChange={() => void act(() => gatewaySetActive(g.id), `Using ${g.name}`)} />
                <span className="truncate font-semibold text-ide-bright">{g.name}</span>
                <span className="ml-auto text-[11px] text-ide-dim">{g.kind === "local" ? "this computer" : g.kind === "public" ? "public list" : g.kind === "tandemforge" ? "TandemForge" : g.kind === "nexus" ? "NEXUS Engine" : "manual"}</span>
              </label>
              <div className="truncate pl-5 text-xs text-ide-dim" title={g.url}>{g.url}</div>
              <div className="flex flex-wrap items-center gap-1 pl-5 pt-1 text-xs">
                {g.dev ? <span className="flex items-center gap-1 rounded bg-[#5a1d1d] px-1.5 text-white"><ShieldAlert size={12} aria-hidden />INSECURE — DEV ONLY</span> : null}
                {g.pinned ? <span className="flex items-center gap-1 text-ide-dim"><Lock size={12} aria-hidden />CA pinned</span> : null}
                {g.account ? (
                  <span className="flex items-center gap-1 text-ide-dim" title={g.account.home}><UserRound size={12} aria-hidden />
                    account {homes.find((h) => h.issuer === g.account?.home)?.email || g.account.home}
                    {homes.find((h) => h.issuer === g.account?.home && h.signed_in) ? null : <span className="text-[#cca700]">(sign in)</span>}
                  </span>
                ) : g.kind === "nexus" ? <span className="flex items-center gap-1 text-ide-dim">local engine (no key needed)</span> : g.has_key ? <span className="flex items-center gap-1 text-ide-dim"><KeyRound size={12} aria-hidden />{g.kind === "tandemforge" ? "paired (code in keychain)" : "key in keychain"}</span> : <span className="text-[#cca700]">no key</span>}
              </div>
              {c ? (
                <div className="pl-5 pt-1 text-xs" role="status">
                  {typeof c === "string" ? <span className="text-ide-dim">{c}</span> : (
                    <span className="flex items-center gap-1 text-[#89d185]"><CheckCircle2 size={12} aria-hidden />{c.latency_ms} ms, {c.models.length} {c.models.length === 1 ? "model" : "models"}</span>
                  )}
                </div>
              ) : null}
              <div className="flex flex-wrap gap-1 pl-4 pt-1 text-xs">
                <button type="button" className={btn} onClick={() => void check(g.id)}><RefreshCw size={12} className="mr-1 inline" aria-hidden />Check</button>
                {g.kind === "tandemforge" || g.kind === "nexus" ? null : (
                  <>
                    <button type="button" className={btn} onClick={() => { setKeyFor(keyFor === g.id ? null : g.id); setNewKey(""); }}>{g.has_key ? "Replace key" : "Add key"}</button>
                    {g.account ? (
                      <button type="button" className={btn} onClick={() => void act(() => gatewaySetAccount(g.id, "", ""), "This gateway uses its key again")}>Use key instead</button>
                    ) : (
                      <Tip text="Sign in to this gateway with your P2P-AI account instead of a key. The gateway's operator tells you its account id.">
                        <button type="button" className={btn} onClick={() => { setAcctFor(acctFor === g.id ? null : g.id); setAcctHome(homes.find((h) => h.signed_in)?.issuer ?? ""); setAcctAud(""); }}>Use account</button>
                      </Tip>
                    )}
                    <Tip text="Trust only this CA for this gateway (for a private or self-signed gateway).">
                      <button type="button" className={btn} onClick={() => void act(() => gatewayPickCa(g.id))}>{g.pinned ? "Change CA" : "Pin CA"}</button>
                    </Tip>
                  </>
                )}
                {g.pinned && g.kind !== "local" ? <button type="button" className={btn} onClick={() => void act(() => gatewayClearCa(g.id), "CA removed")}>Unpin</button> : null}
                <button
                  type="button"
                  className={`${btn} ${removeArmed === g.id ? "text-[#f48771]" : ""}`}
                  aria-label={removeArmed === g.id ? `Click again to remove ${g.name}` : `Remove ${g.name}`}
                  onClick={() => {
                    // Two clicks: the first only arms, so a stray click never deletes a gateway and its key.
                    if (removeArmed !== g.id) { setRemoveArmed(g.id); onNotice(`Click remove again to delete ${g.name} and its key`); return; }
                    setRemoveArmed(null);
                    void act(() => gatewayRemove(g.id), "Gateway removed");
                  }}
                ><Trash2 size={12} className="mr-1 inline" aria-hidden />{removeArmed === g.id ? "Confirm" : "Remove"}</button>
              </div>
              {keyFor === g.id ? (
                <form
                  className="flex gap-1 pl-5 pt-1"
                  onSubmit={(e) => {
                    e.preventDefault();
                    const k = newKey;
                    setNewKey("");
                    setKeyFor(null);
                    void act(() => gatewaySetKey(g.id, k), "Key saved in the system keychain");
                  }}
                >
                  <input aria-label={`Key for ${g.name}`} type="password" autoComplete="off" value={newKey} onChange={(e) => setNewKey(e.target.value)} className={input} placeholder="API key" />
                  <button type="submit" className="rounded bg-ide-accent px-2 text-white">Save</button>
                </form>
              ) : null}
              {acctFor === g.id ? (
                <form
                  className="flex flex-col gap-1 pl-5 pt-1 text-xs"
                  onSubmit={(e) => {
                    e.preventDefault();
                    setAcctFor(null);
                    void act(() => gatewaySetAccount(g.id, acctHome, acctAud), "This gateway now signs in with your account");
                  }}
                >
                  <label className="text-ide-dim">Account service
                    <input aria-label={`Account service for ${g.name}`} list="account-homes" value={acctHome} onChange={(e) => setAcctHome(e.target.value)} className={input} placeholder="https://accounts.example.org" />
                  </label>
                  <datalist id="account-homes">{homes.map((h) => <option key={h.id} value={h.issuer}>{h.email}</option>)}</datalist>
                  <label className="text-ide-dim">Gateway's account id
                    <input aria-label={`Account id of ${g.name}`} value={acctAud} onChange={(e) => setAcctAud(e.target.value)} className={input} placeholder="gw-example-01" />
                  </label>
                  <button type="submit" className="self-start rounded bg-ide-accent px-2 text-white">Link</button>
                </form>
              ) : null}
            </li>
          );
        })}
      </ul>
      <AccountsPanel onChanged={setHomes} onNotice={onNotice} />
      <TandemForgeConnect onPaired={() => void refresh()} onNotice={onNotice} />
      <NexusConnect onConnected={() => void refresh()} onNotice={onNotice} />
      {adding ? (
        <form className="flex flex-col gap-2 rounded border border-ide-line p-2" onSubmit={(e) => { e.preventDefault(); void add(false); }}>
          <label className="text-xs text-ide-dim">Name<input aria-label="Gateway name" value={name} onChange={(e) => setName(e.target.value)} className={input} placeholder="My gateway" /></label>
          <label className="text-xs text-ide-dim">Address<input aria-label="Gateway address" value={url} onChange={(e) => setUrl(e.target.value)} className={input} placeholder="https://gateway.example.org" /></label>
          <label className="text-xs text-ide-dim">Key (stored in the system keychain)<input aria-label="Gateway key" type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} className={input} placeholder="optional, can be added later" /></label>
          <div className="flex flex-wrap gap-2">
            <button type="submit" className="rounded bg-ide-accent px-3 py-1 text-white">Add by address</button>
            <Tip text="Pick the setup folder of the gateway on this computer (made by the P2P-AI desktop app). The IDE reads its port and CA, never its admin key.">
              <button type="button" onClick={() => void add(true)} className="rounded border border-ide-line px-3 py-1">This computer's gateway...</button>
            </Tip>
            <button type="button" onClick={() => { setAdding(false); setKey(""); }} className={btn}>Cancel</button>
          </div>
        </form>
      ) : (
        <button type="button" onClick={() => setAdding(true)} className="flex items-center gap-1 self-start rounded bg-ide-accent px-3 py-1 text-white"><Plus size={14} aria-hidden />Add gateway</button>
      )}
      <PublicGateways savedUrls={list.gateways.map((g) => g.url)} onAdded={() => void refresh()} onNotice={onNotice} />
    </div>
  );
}
