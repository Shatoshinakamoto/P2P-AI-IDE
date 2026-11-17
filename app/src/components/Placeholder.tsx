/**
 * Placeholder.tsx -- side bar views that arrive in later phases (Chat, Missions, Gateways, Extensions). They explain
 * what is coming so the layout is complete from the start; they do nothing and call nothing.
 */
export function Placeholder({ title, phase, children }: { title: string; phase: string; children: string }) {
  return (
    <div className="flex flex-col gap-2 px-3 text-ide-text">
      <div className="text-ide-bright">{title}</div>
      <p className="leading-relaxed text-ide-dim">{children}</p>
      <span className="self-start rounded bg-ide-sel px-2 py-0.5 text-xs text-ide-bright">Arrives in {phase}</span>
    </div>
  );
}
