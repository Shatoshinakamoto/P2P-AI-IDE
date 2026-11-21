/**
 * TandemForgeConnect.tsx -- "TandemForge on this computer" in the Gateways side bar (phase I7,
 * docs/features/tandemforge-engine.md).
 *
 * Purpose: shows whether TandemForge is running here with an app connection open, and pairs the IDE with it using
 * the code TandemForge shows in Settings > Connected apps. Once paired, TandemForge appears in the saved list and is
 * used like a gateway: chat and the agent talk to TandemForge's local models.
 * Trust boundary: the code field is a password input; its value goes once to the Rust core, which checks it against
 * TandemForge and stores it in the OS keychain, then the field is cleared. The port is never typed or shown here; the
 * core reads it from TandemForge's own file. Messages are rendered as React text (CWE-79).
 */
import { useEffect, useState } from "react";
import { Cpu, Link2 } from "lucide-react";
import { tandemforgeConnect, tandemforgeStatus, type TandemForgeStatus } from "../ipc.ts";

interface Props {
  onPaired: () => void;
  onNotice: (m: string) => void;
}

const input = "w-full rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright placeholder:text-ide-dim";

export function TandemForgeConnect({ onPaired, onNotice }: Props) {
  const [status, setStatus] = useState<TandemForgeStatus | null>(null);
  const [open, setOpen] = useState(false);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);

  const load = async () => {
    try {
      setStatus(await tandemforgeStatus());
    } catch (e) {
      onNotice((e as Error).message);
    }
  };
  useEffect(() => { void load(); }, []);

  const pair = async () => {
    const c = code;
    // SECURITY: the code leaves the form at once; from here it lives only in the OS keychain.
    setCode("");
    setBusy(true);
    try {
      const r = await tandemforgeConnect(c);
      onNotice(`Paired with TandemForge: ${r.models.length} ${r.models.length === 1 ? "model" : "models"} on this computer`);
      setOpen(false);
      onPaired();
    } catch (e) {
      onNotice((e as Error).message);
    } finally {
      setBusy(false);
      await load();
    }
  };

  return (
    <section aria-label="TandemForge on this computer" className="rounded border border-ide-line p-2 text-xs" data-testid="tandemforge-connect">
      <div className="flex items-center gap-2">
        <Cpu size={14} aria-hidden className="text-ide-dim" />
        <span className="font-semibold text-ide-bright">TandemForge on this computer</span>
        <button type="button" onClick={() => void load()} className="ml-auto rounded px-1 hover:bg-ide-hover">Refresh</button>
      </div>
      <p className="pt-1 text-ide-dim" role="status">
        {status === null
          ? "Looking for TandemForge..."
          : status.running
            ? status.id
              ? "Running and paired. Pick it in the list above to chat with its local models."
              : "Running. Pair the IDE to use its local models."
            : status.reason}
      </p>
      {open ? (
        <form className="flex flex-col gap-1 pt-1" onSubmit={(e) => { e.preventDefault(); void pair(); }}>
          <label className="text-ide-dim">
            Pairing code from TandemForge (Settings, Connected apps)
            <input aria-label="TandemForge pairing code" type="password" autoComplete="off" value={code} onChange={(e) => setCode(e.target.value)} className={input} placeholder="tfa_..." />
          </label>
          <div className="flex gap-2">
            <button type="submit" disabled={busy || code.trim() === ""} className="rounded bg-ide-accent px-3 py-1 text-white disabled:opacity-60">{busy ? "Checking..." : "Pair"}</button>
            <button type="button" onClick={() => { setOpen(false); setCode(""); }} className="rounded px-2 py-1 hover:bg-ide-hover">Cancel</button>
          </div>
        </form>
      ) : (
        <button type="button" onClick={() => setOpen(true)} className="mt-1 flex items-center gap-1 rounded border border-ide-line px-2 py-1 hover:bg-ide-hover">
          <Link2 size={12} aria-hidden />{status?.id ? "Pair again" : "Pair with TandemForge"}
        </button>
      )}
    </section>
  );
}
