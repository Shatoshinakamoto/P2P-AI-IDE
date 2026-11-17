/**
 * StatusBar.tsx -- the bottom bar: gateway, tool location, Autopilot and Auto Improve switches, cursor and language.
 *
 * In phase I1 there is no network code at all, so the gateway, tools and mission switches are shown but disabled,
 * each with a hint saying which phase turns it on. Nothing here sends anything anywhere.
 */
import { Plug, Rocket, Sparkles, Wrench } from "lucide-react";
import { Tip } from "./Tip.tsx";

interface Props {
  line: number;
  col: number;
  language: string;
  notice: string;
}

const item = "flex h-[22px] items-center gap-1 rounded-sm px-2";

export function StatusBar({ line, col, language, notice }: Props) {
  return (
    <footer className="flex min-h-[24px] shrink-0 flex-wrap items-center gap-1 bg-ide-accent px-1 text-xs text-white">
      <Tip text="Connect a local, public or manual gateway. Arrives in phase I2.">
        <button type="button" disabled aria-disabled className={`${item} opacity-90`}><Plug size={13} aria-hidden />No gateway</button>
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
