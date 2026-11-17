/**
 * PublicGateways.tsx -- the "Public gateways" part of the Gateways side bar (phase I2b,
 * docs/features/public-gateways.md).
 *
 * Purpose: the person lists directories (https addresses), presses Refresh, and sees the gateways that publish
 * themselves for users: region, measured latency, models, how to get access (key or sign-up), and whether the
 * operator says prompts are logged. Add saves one as a gateway; its key is added afterwards like any other.
 * Trust boundary: every value shown came from a signed advert that the Rust core verified, but it is still
 * untrusted text: rendered as React text only, links are not clickable (they can be copied), nothing is run
 * (CWE-79, CWE-601). Adding sends only the advert id; the core takes the address from its own verified copy.
 */
import { useEffect, useState } from "react";
import { Copy, Globe, Plus, RefreshCw, X } from "lucide-react";
import { directoriesList, directoriesSet, gatewayAddPublic, publicList, type PublicList } from "../ipc.ts";

interface Props {
  /** URLs of the saved gateways, to mark entries that are already added. */
  savedUrls: string[];
  onAdded: () => void;
  onNotice: (m: string) => void;
}

const input = "w-full rounded border border-ide-line bg-ide-bg px-2 py-1 text-ide-bright placeholder:text-ide-dim";
const btn = "rounded px-2 py-1 hover:bg-ide-hover";

/** The origin the core saves for an api_url (lower case, no trailing slash), to match saved gateways. */
const origin = (u: string) => u.replace(/\/+$/, "").toLowerCase();

/** Copies a URL so the person can open it in their own browser (links in adverts are never clickable). */
function CopyUrl({ url, label, onNotice }: { url: string; label: string; onNotice: (m: string) => void }) {
  return (
    <span className="flex min-w-0 items-center gap-1">
      <span className="text-ide-dim">{label}</span>
      <span className="truncate" title={url}>{url}</span>
      <button
        type="button"
        aria-label={`Copy ${label.toLowerCase()} address`}
        className="rounded p-0.5 hover:bg-ide-hover"
        onClick={() => { navigator.clipboard.writeText(url).then(() => onNotice("Address copied"), () => onNotice("Could not copy")); }}
      ><Copy size={11} aria-hidden /></button>
    </span>
  );
}

export function PublicGateways({ savedUrls, onAdded, onNotice }: Props) {
  const [dirs, setDirs] = useState<string[]>([]);
  const [newDir, setNewDir] = useState("");
  const [list, setList] = useState<PublicList | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => { directoriesList().then(setDirs).catch((e) => onNotice((e as Error).message)); }, [onNotice]);

  const saveDirs = async (next: string[]) => {
    try { setDirs(await directoriesSet(next)); return true; } catch (e) { onNotice((e as Error).message); return false; }
  };

  const refresh = async () => {
    setLoading(true);
    try { setList(await publicList()); } catch (e) { onNotice((e as Error).message); } finally { setLoading(false); }
  };

  const add = async (id: string, signup: string) => {
    try {
      await gatewayAddPublic(id, null);
      onNotice(signup ? "Gateway added. Get a key from its sign-up page, then press Add key." : "Gateway added. Ask its operator for a key, then press Add key.");
      onAdded();
    } catch (e) {
      onNotice((e as Error).message);
    }
  };

  const skipped = list ? Object.entries(list.rejected) : [];
  const skippedCount = skipped.reduce((n, [, c]) => n + c, 0);

  return (
    <section aria-label="Public gateways" className="flex flex-col gap-2 border-t border-ide-line pt-2">
      <h3 className="flex items-center gap-1 text-xs font-semibold uppercase text-ide-dim"><Globe size={12} aria-hidden />Public gateways</h3>
      <ul aria-label="Directories" className="flex flex-col gap-1 text-xs">
        {dirs.map((d) => (
          <li key={d} className="flex items-center gap-1">
            <span className="truncate text-ide-bright" title={d}>{d}</span>
            <button type="button" aria-label={`Remove directory ${d}`} className="ml-auto rounded p-0.5 hover:bg-ide-hover" onClick={() => void saveDirs(dirs.filter((x) => x !== d))}><X size={12} aria-hidden /></button>
          </li>
        ))}
      </ul>
      <form className="flex gap-1" onSubmit={(e) => { e.preventDefault(); if (!newDir.trim()) return; void saveDirs([...dirs, newDir]).then((ok) => { if (ok) setNewDir(""); }); }}>
        <input aria-label="Directory address" value={newDir} onChange={(e) => setNewDir(e.target.value)} className={input} placeholder="https://directory.example.org" />
        <button type="submit" aria-label="Add directory" className={btn}><Plus size={14} aria-hidden /></button>
      </form>
      {dirs.length === 0 ? <p className="text-xs text-ide-dim">Add a directory you trust to list the gateways it knows. The IDE ships with none and fetches nothing until you add one.</p> : (
        <button type="button" disabled={loading} onClick={() => void refresh()} className="flex items-center gap-1 self-start rounded border border-ide-line px-2 py-1 text-xs disabled:opacity-50">
          <RefreshCw size={12} aria-hidden className={loading ? "animate-spin" : ""} />{loading ? "Checking..." : "Refresh list"}
        </button>
      )}
      {list ? (
        <>
          {list.gateways.length === 0 ? <p className="text-xs text-ide-dim">No public gateway for users was found.</p> : null}
          <ul aria-label="Public gateway list" className="flex flex-col gap-2">
            {list.gateways.map((g) => {
              const added = savedUrls.includes(origin(g.access.api_url));
              return (
                <li key={g.gateway_id} className="rounded border border-ide-line p-2 text-xs">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-ide-bright" title={g.gateway_id}>{g.gateway_id.slice(0, 8)}</span>
                    {g.region ? <span className="text-ide-dim">{g.region}</span> : null}
                    <span className="ml-auto text-ide-dim">{g.latency_ms === null ? "no probe" : `${g.latency_ms} ms`}</span>
                  </div>
                  <div className="truncate text-ide-dim" title={g.models.join(", ")}>{g.models.length ? g.models.join(", ") : "no models listed"}</div>
                  <div className="text-ide-dim">{g.access.mode === "signup" ? "Sign-up gives a key" : "Key from the operator"} · {g.access.features.join(", ")}</div>
                  <div className={g.access.logs_prompts ? "text-[#cca700]" : "text-ide-dim"}>
                    {g.access.logs_prompts ? `Logs prompts, kept ${g.access.retention_days} days` : "Says it does not log prompts"}
                  </div>
                  <div className="flex flex-col gap-0.5 pt-1">
                    <CopyUrl url={g.access.api_url} label="API" onNotice={onNotice} />
                    {g.access.signup_url ? <CopyUrl url={g.access.signup_url} label="Sign-up" onNotice={onNotice} /> : null}
                    {g.access.terms_url ? <CopyUrl url={g.access.terms_url} label="Terms" onNotice={onNotice} /> : null}
                  </div>
                  <div className="pt-1">
                    {added ? <span className="text-ide-dim">Added</span> : (
                      <button type="button" aria-label={`Add public gateway ${g.gateway_id.slice(0, 8)}`} onClick={() => void add(g.gateway_id, g.access.signup_url)} className="rounded bg-ide-accent px-2 py-0.5 text-white">Add</button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
          {skippedCount > 0 ? (
            <p className="text-xs text-ide-dim" title={skipped.map(([k, c]) => `${k}: ${c}`).join(", ")}>
              {skippedCount} {skippedCount === 1 ? "entry" : "entries"} skipped ({skipped.map(([k, c]) => `${k.replace(/_/g, " ")} ${c}`).join(", ")})
            </p>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
