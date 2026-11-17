/**
 * StatusBar.tsx -- the bottom bar: gateway, tool location, Autopilot and Auto Improve switches, cursor and language.
 *
 * The gateway chip (phase I2) shows the active gateway's name and opens the Gateways view; a plain-http dev gateway
 * is marked INSECURE - DEV ONLY. The tools and mission switches stay disabled, each with a hint saying which phase
 * turns it on. Nothing here sends anything anywhere.
 */
import { Plug, Rocket, ShieldAlert, Sparkles, Wrench } from "lucide-react";
import type { GatewayView } from "../ipc.ts";
import { Tip } from "./Tip.tsx";

interface Props {
  line: number;
  col: number;
  language: string;
  notice: string;
  gateway: GatewayView | null;
  onGateway: () => void;
}

const item = "flex h-[22px] items-center gap-1 rounded-sm px-2";

export function StatusBar({ line, col, language, notice, gateway, onGateway }: Props) {
  return (
    <footer className="flex min-h-[24px] shrink-0 flex-wrap items-center gap-1 bg-ide-accent px-1 text-xs text-white">
      <Tip text={gateway ? `Active gateway: ${gateway.url}. Click to manage gateways.` : "Add the gateway on this computer or one by its address."}>
        <button type="button" onClick={onGateway} className={`${item} hover:bg-white/10`}>
          <Plug size={13} aria-hidden />{gateway ? gateway.name : "No gateway"}
          {gateway?.dev ? <span className="ml-1 flex items-center gap-0.5 rounded bg-[#5a1d1d] px-1"><ShieldAlert size={11} aria-hidden />INSECURE — DEV ONLY</span> : null}
        </button>
      </Tip>
      <Tip text="Where the agent's tools run: Local (this folder) or Gateway (the gateway's copy). Arrives with the agent in phase I3.">
        <button type="button" disabled aria-disabled className={`${item} opacity-90`}><Wrench size={13} aria-hidden />Tools: Local</button>
      </Tip>
      <span role="status" aria-live="polite" className="truncate px-2">{notice}</span>
      <span className="flex-1" />
      <Tip text="Autopilot works on a goal until your acceptance checks pass. Runs on the gateway; arrives in phase I4.">
        <button type="button" disabled aria-disabled aria-pressed={false} className={`${item} opacity-90`}><Rocket size={13} aria-hidden />Autopilot off</button>
      </Tip>
      <Tip text="Auto Improve suggests ideas and never changes files. Runs on the gateway; arrives in phase I4.">
        <button type="button" disabled aria-disabled aria-pressed={false} className={`${item} opacity-90`}><Sparkles size={13} aria-hidden />Auto Improve off</button>
      </Tip>
      <span className={item}>Ln {line}, Col {col}</span>
      <span className={item}>{language}</span>
    </footer>
  );
}
