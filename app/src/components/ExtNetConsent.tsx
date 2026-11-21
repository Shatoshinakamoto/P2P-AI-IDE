/**
 * ExtNetConsent.tsx -- the card that asks "Allow internet for this extension?" (phase I6b, extNet.ts).
 *
 * Purpose: shown the first time a web extension tries to reach an https site. Allow saves the grant in the Rust
 * core; Not now refuses until the IDE restarts. The Extensions view can take the grant away later.
 * Trust boundary: the extension name and host come from extension data and the extension's request; React renders
 * them as text (CWE-79). Nothing is allowed without this click (NIST AC-3).
 * Dependencies: React, lucide-react.
 */
import { useEffect, useState } from "react";
import { Globe } from "lucide-react";
import { onNetConsent, type NetConsent } from "../extNet.ts";

export function ExtNetConsent() {
  const [q, setQ] = useState<NetConsent | null>(null);
  useEffect(() => onNetConsent(setQ), []);
  if (!q) return null;
  return (
    <div role="alertdialog" aria-labelledby="extnet-title" aria-describedby="extnet-text" className="fixed bottom-10 right-4 z-50 w-96 max-w-[calc(100vw-2rem)] rounded border border-ide-line bg-ide-panel p-3 text-sm shadow-lg">
      <h2 id="extnet-title" className="flex items-center gap-2 font-semibold text-ide-bright">
        <Globe size={15} aria-hidden />Allow internet for {q.name}?
      </h2>
      <p id="extnet-text" className="mt-1 text-ide-dim">
        {q.name} wants to reach <span className="font-mono text-ide-bright">{q.host || "a website"}</span>. If you allow it, it can reach
        public https sites while it is on, and can send them what it sees in the editor. It still can't reach this
        computer's network, your files or your keys. You can stop it in Extensions.
      </p>
      <div className="mt-2 flex justify-end gap-2">
        <button type="button" className="rounded px-2 py-1 hover:bg-ide-hover" onClick={() => q.answer(false)}>Not now</button>
        <button type="button" className="rounded bg-ide-accent px-2 py-1 text-white" onClick={() => q.answer(true)}>Allow internet</button>
      </div>
    </div>
  );
}
