/**
 * extNetShim.ts -- runs first inside the web worker extension host (phase I6b, "Plus internet, asked per extension",
 * docs/features/extensions.md#internet-for-web-extensions).
 *
 * Purpose: the worker's Content-Security-Policy allows no network, so a web extension's `fetch("https://...")` would
 * fail. This file replaces `fetch` in the worker, before any extension code loads. Requests to https sites are sent
 * to the IDE page, which makes them through the Rust core only for extensions the person allowed (asking once).
 * Everything else (`data:` and the app's own files, which VS Code uses to load extension code) goes to the real
 * `fetch`, still under the CSP.
 *
 * Trust boundaries: extension code in this worker is untrusted. The page side (extNet.ts) re-checks everything.
 * Security:
 *   * Private channel: a random channel name (Web Crypto CSPRNG) is announced once, at worker start, before any
 *     extension code exists to hear it. Later messages travel on that name, which only this closure knows, so
 *     extension code can't forge requests or read answers on it (CWE-345, CWE-200).
 *   * Attribution: the extensions whose code is on the call stack (VS Code tags each extension's code with its
 *     `extension-file://<id>/` source URL). Every one of them must be allowed; a call with none is refused
 *     (deny by default). Known limit: all web extensions share this worker, so a hostile extension could borrow
 *     another one's access (for example by passing `fetch` to it as a callback). Documented as an accepted risk.
 *   * http:// and other sites' plain URLs are refused here; the core accepts public https only (CWE-319, CWE-918).
 */

const HELLO = "p2p-ide-ext-net-hello";
type Answer = { t: "res"; rid: string; ok: boolean; error?: string; status?: number; statusText?: string; headers?: [string, string][]; url?: string; body_b64?: string };

const NativeError = Error;
const realFetch = self.fetch.bind(self);
const rand = () => Array.from(crypto.getRandomValues(new Uint8Array(24)), (b) => b.toString(16).padStart(2, "0")).join("");
const secret = rand();

// SECURITY: the hello carries the private channel name and is sent now, while no extension code is loaded.
const hello = new BroadcastChannel(HELLO);
hello.postMessage({ t: "hello", ch: secret });
hello.close();

const channel = new BroadcastChannel(secret);
const pending = new Map<string, { resolve: (a: Answer) => void }>();
channel.onmessage = (ev: MessageEvent) => {
  const a = ev.data as Answer;
  if (!a || a.t !== "res" || typeof a.rid !== "string") return;
  pending.get(a.rid)?.resolve(a);
  pending.delete(a.rid);
};

/** The extension ids whose code is on the current call stack, lowercase, innermost first. */
function callers(): string[] {
  const limit = (NativeError as unknown as { stackTraceLimit?: number }).stackTraceLimit;
  try {
    (NativeError as unknown as { stackTraceLimit?: number }).stackTraceLimit = 100;
    const stack = String(new NativeError().stack ?? "");
    const ids: string[] = [];
    for (const m of stack.matchAll(/extension-file:\/\/([A-Za-z0-9._-]{1,201})\//g)) {
      const id = m[1].toLowerCase();
      if (!ids.includes(id)) ids.push(id);
    }
    return ids;
  } finally {
    (NativeError as unknown as { stackTraceLimit?: number }).stackTraceLimit = limit;
  }
}

function toB64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function fromB64(b64: string): Uint8Array<ArrayBuffer> {
  const s = atob(b64);
  const out = new Uint8Array(new ArrayBuffer(s.length));
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

const NULL_BODY = new Set([101, 103, 204, 205, 304]);

async function viaIde(ids: string[], req: Request): Promise<Response> {
  const body = req.method === "GET" || req.method === "HEAD" ? new Uint8Array() : new Uint8Array(await req.arrayBuffer());
  const headers: [string, string][] = [];
  req.headers.forEach((v, k) => headers.push([k, v]));
  const rid = rand();
  const answer = await new Promise<Answer>((resolve) => {
    pending.set(rid, { resolve });
    channel.postMessage({ t: "req", rid, ids, method: req.method, url: req.url, headers, body_b64: toB64(body) });
  });
  if (!answer.ok) throw new TypeError(answer.error || "network request refused");
  const status = answer.status ?? 0;
  const resp = new Response(NULL_BODY.has(status) ? null : fromB64(answer.body_b64 ?? ""), {
    status: status >= 200 && status <= 599 ? status : 502,
    statusText: answer.statusText ?? "",
    headers: answer.headers ?? [],
  });
  Object.defineProperty(resp, "url", { value: answer.url ?? req.url });
  return resp;
}

/** The worker's fetch: https sites go through the IDE, the rest to the real fetch under the CSP. */
function ideFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  let url: URL;
  try {
    url = new URL(input instanceof Request ? input.url : String(input), self.location.href);
  } catch {
    return realFetch(input, init);
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return realFetch(input, init);
  if (url.origin === self.location.origin) return realFetch(input, init);
  if (url.protocol === "http:") return Promise.reject(new TypeError("extensions may reach https sites only"));
  const ids = callers();
  // SECURITY: deny by default when no extension's code made the call (CWE-862).
  if (ids.length === 0) return Promise.reject(new TypeError("network requests must come from an extension"));
  if (init?.signal?.aborted) return Promise.reject(new DOMException("The operation was aborted.", "AbortError"));
  let req: Request;
  try {
    req = new Request(input, init);
  } catch (e) {
    return Promise.reject(e);
  }
  return viaIde(ids, req);
}

self.fetch = ideFetch as typeof fetch;

export {};
