/**
 * extNet.ts -- the IDE page's side of internet for web extensions (phase I6b, decision 2026-11-21 "Plus internet,
 * asked per extension"; docs/features/extensions.md#internet-for-web-extensions).
 *
 * Purpose: receives the https requests that extNetShim.ts forwards from the web worker extension host, asks the
 * person once per extension ("Allow internet for X?"), and makes allowed requests through the Rust core
 * (`ext_fetch`, public https only). The Extensions view shows and changes who has internet.
 *
 * Trust boundaries: messages on the channels are untrusted (extension code shares the worker); every field is
 * checked. The grant itself is stored by the core and checked again there on every request.
 * Security:
 *   * The private channel name is accepted only while armed: at start and right before the IDE restarts the
 *     extension host, i.e. before any extension code runs. The first hello disarms it (CWE-345).
 *   * Deny by default: an extension that is not an enabled web extension, or that the person did not allow, gets
 *     no request made; "Not now" is remembered until the IDE restarts so a looping extension can't nag (CWE-862).
 *   * Requests and answers are size-capped here and again in the core (CWE-400). Only the core touches the network.
 */
import { extFetch, extList, extSetNetwork, type ExtInstalled } from "./ipc.ts";

const HELLO = "p2p-ide-ext-net-hello";
const MAX_B64 = Math.ceil((4 * 1024 * 1024) / 3) * 4 + 8;

/** A question the consent card shows. */
export interface NetConsent {
  id: string;
  name: string;
  host: string;
  answer(allow: boolean): void;
}

type Req = { t: "req"; rid: string; ids: string[]; method: string; url: string; headers: [string, string][]; body_b64: string };

let armed = true;
let channel: BroadcastChannel | null = null;
const denied = new Set<string>();
const asking = new Map<string, Promise<boolean>>();
const listeners = new Set<(q: NetConsent | null) => void>();
const queue: NetConsent[] = [];

/** Lets the next hello set up a new private channel. Call right before (re)starting the extension host. */
export function armExtensionNet(): void {
  armed = true;
}

/** The consent card subscribes here; it gets the question to show, or null when none is waiting. */
export function onNetConsent(cb: (q: NetConsent | null) => void): () => void {
  listeners.add(cb);
  cb(queue[0] ?? null);
  return () => listeners.delete(cb);
}

function notify() {
  for (const cb of listeners) cb(queue[0] ?? null);
  window.dispatchEvent(new Event("ide-extensions-changed"));
}

const hostOf = (u: string) => {
  try {
    return new URL(u).hostname;
  } catch {
    return "";
  }
};

/** Asks the person once for `ext`; resolves true when allowed (and saved by the core). */
function ask(ext: ExtInstalled, host: string): Promise<boolean> {
  const running = asking.get(ext.id);
  if (running) return running;
  const p = new Promise<boolean>((resolve) => {
    const q: NetConsent = {
      id: ext.id,
      name: ext.display_name || ext.name,
      host,
      answer: (allow) => {
        queue.splice(queue.indexOf(q), 1);
        asking.delete(ext.id);
        if (!allow) {
          denied.add(ext.id);
          notify();
          resolve(false);
          return;
        }
        // SECURITY: the grant is written only here, from the person's click (NIST AC-3).
        extSetNetwork(ext.id, true).then(() => { notify(); resolve(true); }, () => { notify(); resolve(false); });
      },
    };
    queue.push(q);
    notify();
  });
  asking.set(ext.id, p);
  return p;
}

const valid = (m: Req) =>
  m && m.t === "req" && typeof m.rid === "string" && m.rid.length <= 64 && Array.isArray(m.ids) && m.ids.length > 0 && m.ids.length <= 16 &&
  m.ids.every((i) => typeof i === "string" && /^[a-z0-9._-]{1,201}$/.test(i)) && typeof m.method === "string" && m.method.length <= 10 &&
  typeof m.url === "string" && m.url.length <= 4096 && Array.isArray(m.headers) && m.headers.length <= 64 &&
  m.headers.every((h) => Array.isArray(h) && h.length === 2 && typeof h[0] === "string" && typeof h[1] === "string") &&
  typeof m.body_b64 === "string" && m.body_b64.length <= MAX_B64;

async function handle(m: Req, reply: (a: object) => void) {
  const fail = (error: string) => reply({ t: "res", rid: m.rid, ok: false, error });
  if (!valid(m)) return fail("bad request");
  const list = await extList().catch(() => [] as ExtInstalled[]);
  const host = hostOf(m.url);
  // Every extension whose code made the call must be allowed (innermost first).
  for (const id of m.ids) {
    const ext = list.find((e) => e.id === id && e.enabled && e.kind === "web");
    if (!ext) return fail("network requests must come from an installed web extension");
    if (ext.network) continue;
    if (denied.has(id) || !(await ask(ext, host))) return fail("internet is not allowed for this extension");
  }
  try {
    const a = await extFetch(m.ids[0], { method: m.method, url: m.url, headers: m.headers, body_b64: m.body_b64 });
    reply({ t: "res", rid: m.rid, ok: true, status: a.status, statusText: a.status_text, headers: a.headers, url: a.url, body_b64: a.body_b64 });
  } catch (e) {
    fail(String((e as Error)?.message ?? e));
  }
}

// The hello channel: the worker announces its private channel name once, at start.
const hello = new BroadcastChannel(HELLO);
hello.onmessage = (ev: MessageEvent) => {
  const d = ev.data as { t?: string; ch?: string };
  if (!armed || d?.t !== "hello" || typeof d.ch !== "string" || !/^[0-9a-f]{48}$/.test(d.ch)) return;
  // SECURITY: accept one hello per host start; later ones (extension code knows this public name) are ignored.
  armed = false;
  channel?.close();
  const ch = new BroadcastChannel(d.ch);
  channel = ch;
  ch.onmessage = (e: MessageEvent) => {
    const m = e.data as Req;
    if (m?.t !== "req") return; // our own answers echo to nobody, but other kinds are ignored
    void handle(m, (a) => ch.postMessage(a));
  };
};
