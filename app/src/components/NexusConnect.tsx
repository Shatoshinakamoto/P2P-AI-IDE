/**
 * NexusConnect.tsx -- "NEXUS Engine on this computer" in the Gateways side bar (docs/features/nexus-engine.md).
 *
 * Purpose: detects local NEXUS Proxy (Context Packer + Data Airlock + Agent tools, default 11435) and NEXUS
 * Runtime (native .nexus model inference, default 11436) on `127.0.0.1` via Rust `nexus_status`, and connects
 * either or both to the IDE's gateway list without an API key.
 *
 * Security: all URL discovery, `~/.nexus-studio/studio-settings.json` reading, and HTTP health probes run inside
 * Rust on `127.0.0.1` only (NIST SC-7, CWE-918). Custom port input accepts only integer ports `1024..=65535`,
 * verified again by Rust. Untrusted telemetry strings are rendered strictly as React text (CWE-79).
 */
import { useCallback, useEffect, useState } from "react";
import { Cpu, Link2, RefreshCw } from "lucide-react";
import { nexusConnect, nexusStatus, type NexusRole, type NexusStatusView } from "../ipc.ts";

interface Props {
  onConnected: () => void;
  onNotice: (text: string) => void;
}

const btn = "rounded border border-ide-line px-2 py-0.5 text-[11px] hover:bg-ide-hover disabled:opacity-50";
const input = "w-full rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright focus:border-ide-focus focus:outline-none";

function portFromUrl(url: string): string {
  const idx = url.lastIndexOf(":");
  return idx >= 0 ? url.slice(idx + 1) : "";
}

export function NexusConnect({ onConnected, onNotice }: Props) {
  const [status, setStatus] = useState<NexusStatusView | null>(null);
  const [busyRole, setBusyRole] = useState<NexusRole | null>(null);
  const [customPort, setCustomPort] = useState("");
  const [showCustom, setShowCustom] = useState(false);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    setError("");
    try {
      setStatus(await nexusStatus());
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const connect = async (role: NexusRole, explicitPort: number | null = null) => {
    setBusyRole(role);
    setError("");
    try {
      const r = await nexusConnect(role, explicitPort);
      const label = role === "proxy" ? "NEXUS Proxy" : "NEXUS Runtime";
      onNotice(`Connected to ${label}: ${r.models.length} ${r.models.length === 1 ? "model" : "models"} on this computer`);
      setCustomPort("");
      setStatus(await nexusStatus().catch(() => status));
      onConnected();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusyRole(null);
    }
  };

  const connectCustom = async (role: NexusRole) => {
    const trimmed = customPort.trim();
    const parsed = Number(trimmed);
    if (!/^\d+$/.test(trimmed) || !Number.isInteger(parsed) || parsed < 1024 || parsed > 65535) {
      setError("pick an unprivileged port between 1024 and 65535");
      return;
    }
    await connect(role, parsed);
  };

  const proxy = status?.proxy ?? null;
  const runtime = status?.runtime ?? null;

  return (
    <section aria-label="NEXUS Engine on this computer" className="rounded border border-ide-line p-2 text-xs" data-testid="nexus-connect">
      <div className="mb-1 flex items-center gap-1.5">
        <Cpu size={13} className="shrink-0 text-ide-dim" aria-hidden />
        <span className="font-semibold text-ide-bright">NEXUS Engine on this computer</span>
        <button type="button" onClick={() => void refresh()} disabled={busyRole !== null} className={`ml-auto ${btn}`} aria-label="Refresh NEXUS Engine">
          <RefreshCw size={11} aria-hidden />
        </button>
      </div>

      {status === null ? (
        <p className="text-ide-dim">Checking local NEXUS Proxy and Runtime...</p>
      ) : (
        <div className="space-y-2">
          <div className="rounded border border-ide-line/60 px-2 py-1.5">
            <div className="flex items-center gap-1.5">
              <span className="font-medium text-ide-bright">NEXUS Proxy</span>
              <span className="text-[11px] text-ide-dim">:{portFromUrl(proxy!.url)}</span>
              <span className={`ml-auto text-[11px] ${proxy!.running ? "text-[#4ec9b0]" : "text-ide-dim"}`}>
                {proxy!.running ? "running" : "not detected"}
              </span>
            </div>
            <p className="mt-0.5 text-[11px] text-ide-dim">
              {proxy!.running ? (
                <>
                  Agent tools, Context Packer &amp; Data Airlock
                  {proxy!.airlock_threshold !== null ? ` (airlock ${proxy!.airlock_threshold} tok` : ""}
                  {proxy!.artifacts !== null ? `, ${proxy!.artifacts} artifacts` : ""}
                  {proxy!.airlock_threshold !== null ? ")" : ""}
                  {proxy!.mock_upstream ? " · mock mode" : ""}
                </>
              ) : (
                "Context Packer, Data Airlock & tool forwarding (default :11435)."
              )}
            </p>
            <div className="mt-1.5 flex items-center gap-1.5">
              <button
                type="button"
                disabled={!proxy!.running || busyRole !== null}
                onClick={() => void connect("proxy")}
                className="flex items-center gap-1 rounded bg-ide-accent px-2 py-1 text-white disabled:opacity-50"
              >
                <Link2 size={12} aria-hidden />
                {busyRole === "proxy" ? "Connecting..." : proxy!.id ? "Reconnect Proxy" : "Connect Proxy"}
              </button>
            </div>
          </div>

          <div className="rounded border border-ide-line/60 px-2 py-1.5">
            <div className="flex items-center gap-1.5">
              <span className="font-medium text-ide-bright">NEXUS Runtime</span>
              <span className="text-[11px] text-ide-dim">:{portFromUrl(runtime!.url)}</span>
              <span className={`ml-auto text-[11px] ${runtime!.running ? "text-[#4ec9b0]" : "text-ide-dim"}`}>
                {runtime!.running ? "running" : "not detected"}
              </span>
            </div>
            <p className="mt-0.5 text-[11px] text-ide-dim">
              {runtime!.running ? (
                <>
                  Native .nexus models (chat only): {runtime!.models_loaded ?? 0}/{runtime!.models_available ?? 0} loaded
                  {runtime!.simd ? ` · ${runtime!.simd}` : ""}
                </>
              ) : (
                "Native ternary / .nexus inference for chat (default :11436)."
              )}
            </p>
            <div className="mt-1.5 flex items-center gap-1.5">
              <button
                type="button"
                disabled={!runtime!.running || busyRole !== null}
                onClick={() => void connect("runtime")}
                className="flex items-center gap-1 rounded bg-ide-accent px-2 py-1 text-white disabled:opacity-50"
              >
                <Link2 size={12} aria-hidden />
                {busyRole === "runtime" ? "Connecting..." : runtime!.id ? "Reconnect Runtime" : "Connect Runtime"}
              </button>
            </div>
          </div>

          <div>
            <button type="button" onClick={() => setShowCustom((v) => !v)} className={btn}>
              {showCustom ? "Hide custom port" : "Custom local port"}
            </button>
            {showCustom ? (
              <div className="mt-1.5 space-y-1.5 rounded border border-ide-line/60 p-1.5">
                <label className="block text-ide-dim">
                  Local 127.0.0.1 port (1024–65535)
                  <input
                    aria-label="NEXUS Engine local port"
                    type="text"
                    inputMode="numeric"
                    value={customPort}
                    onChange={(e) => setCustomPort(e.target.value)}
                    placeholder="11435"
                    className={input}
                  />
                </label>
                <div className="flex gap-1.5">
                  <button type="button" disabled={busyRole !== null} onClick={() => void connectCustom("proxy")} className={btn}>
                    Connect as Proxy
                  </button>
                  <button type="button" disabled={busyRole !== null} onClick={() => void connectCustom("runtime")} className={btn}>
                    Connect as Runtime
                  </button>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      )}

      {error ? <p role="alert" className="mt-1.5 text-[#f48771]">{error}</p> : null}
    </section>
  );
}
