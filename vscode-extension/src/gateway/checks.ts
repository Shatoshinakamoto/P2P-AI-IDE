/**
 * checks.ts -- input checks and answer parsing for the extension's gateway client (pure, no network, no vscode).
 *
 * Purpose: the P2P-AI VS Code / VSCodium extension (ADR-029 side track X) talks to the same gateways as the IDE.
 * VS Code extensions run in VS Code's Node, so this is a TypeScript port of the IDE's Rust client checks
 * (core/src/client.rs, core/src/gateways.rs). Both run the shared cases in core/tests/vectors/gateway_client.json,
 * so they accept and refuse the same things.
 *
 * Inputs / trust boundary: gateway URLs, keys and model ids typed by the person; gateway answers (untrusted, from
 * the network). Output: checked values or fixed error kinds.
 *
 * Security controls:
 *  - URLs: https only, except plain http on loopback (INSECURE — DEV ONLY); no path, query, user info or percent
 *    encoding, so the key can only go to the host the person typed (CWE-319, CWE-918, CWE-601).
 *  - Keys: 8-512 visible ASCII characters, so a key cannot break the Authorization header (CWE-113).
 *  - Answers: parsed into fixed shapes; text stays text and is never run or rendered as HTML (CWE-79, CWE-20).
 *  - Errors are fixed messages by kind with at most an HTTP status, never a URL, key or answer text (CWE-209).
 * Dependencies: none (TypeScript standard library only).
 */

/** Fixed error kinds; `message()` text matches the IDE's Rust client. */
export type NetErrorKind =
  | "no_key"
  | "bad_request"
  | "connect"
  | "tls"
  | "timeout"
  | "auth"
  | "rate_limited"
  | "not_offered"
  | "status"
  | "upstream"
  | "bad_answer"
  | "too_large"
  | "stopped";

/** An error safe to show: its message never holds a URL, key or answer text (CWE-209). */
export class NetError extends Error {
  readonly kind: NetErrorKind;
  readonly detail: string | number;
  constructor(kind: NetErrorKind, detail: string | number = "") {
    super(netErrorMessage(kind, detail));
    this.name = "NetError";
    this.kind = kind;
    this.detail = detail;
  }
}

/** The person-facing text for an error kind (same wording as core/src/client.rs `NetError::message`). */
export function netErrorMessage(kind: NetErrorKind, detail: string | number = ""): string {
  switch (kind) {
    case "no_key":
      return "this gateway has no key yet; add one with 'P2P-AI: Set Gateway Key'";
    case "bad_request":
      return String(detail);
    case "connect":
      return "could not reach the gateway";
    case "tls":
      return "the gateway's certificate is not trusted (check its CA file)";
    case "timeout":
      return "the gateway took too long to answer";
    case "auth":
      return "the gateway refused the key";
    case "rate_limited":
      return "the gateway is limiting requests; try again shortly";
    case "not_offered":
      return "this gateway does not offer that";
    case "status":
      return `the gateway answered with an error (HTTP ${Number(detail) || 0})`;
    case "upstream":
      return "the gateway reported an error while answering";
    case "bad_answer":
      return "the gateway's answer could not be read";
    case "too_large":
      return "the gateway's answer was too large";
    case "stopped":
      return "stopped";
  }
}

/** Largest request body sent (bytes). */
export const MAX_REQUEST_BYTES = 2 << 20;
/** Most messages in one request. */
export const MAX_MESSAGES = 200;
/** Largest single message (bytes). */
export const MAX_MESSAGE_BYTES = 256 << 10;
/** Largest streamed answer read (bytes). */
export const MAX_STREAM_BYTES = 8 << 20;
/** Longest single event line (bytes). */
export const MAX_LINE_BYTES = 1 << 20;
/** Largest model list read (bytes), and most ids kept. */
export const MAX_MODELS_BYTES = 1 << 20;
export const MAX_MODELS = 1000;

/** One chat message. Roles are an allow-list. */
export interface Msg {
  role: "system" | "user" | "assistant";
  content: string;
}

const utf8 = new TextEncoder();
const byteLen = (s: string): number => utf8.encode(s).length;

/** True for 1-200 visible ASCII characters (gateways use names like `llama3.1:8b`). */
export function modelOk(m: string): boolean {
  return typeof m === "string" && m.length > 0 && m.length <= 200 && /^[\x21-\x7e]+$/.test(m);
}

/** Throws `bad_request` unless `m` is a valid model id. */
export function checkModel(m: string): void {
  if (!modelOk(m)) throw new NetError("bad_request", "pick a model from the list");
}

/** Checks a conversation before it is sent (CWE-20, CWE-400). */
export function checkMessages(msgs: Msg[]): void {
  if (!Array.isArray(msgs) || msgs.length === 0 || msgs.length > MAX_MESSAGES) {
    throw new NetError("bad_request", "a chat can hold 1 to 200 messages; start a new chat");
  }
  let total = 0;
  for (const m of msgs) {
    if (m.role !== "system" && m.role !== "user" && m.role !== "assistant") {
      throw new NetError("bad_request", "a message has an unknown role");
    }
    if (typeof m.content !== "string") throw new NetError("bad_request", "a message has no text");
    const n = byteLen(m.content);
    if (n > MAX_MESSAGE_BYTES) throw new NetError("bad_request", "a message is larger than 256 KiB");
    total += n;
  }
  if (total > MAX_REQUEST_BYTES - 4096) {
    throw new NetError("bad_request", "the chat is larger than 2 MiB; start a new chat");
  }
}

/** Maps an HTTP status to ok or an error kind (redirects are errors: they are never followed). */
export function checkStatus(code: number): void {
  if (code >= 200 && code <= 299) return;
  if (code === 401 || code === 403) throw new NetError("auth");
  if (code === 404) throw new NetError("not_offered");
  if (code === 429) throw new NetError("rate_limited");
  throw new NetError("status", code);
}

/** One streamed piece of a tool call (kept for later phases; X1 only shows text). */
export interface ToolPiece {
  index: number;
  id: string;
  name: string;
  args: string;
}

/** One parsed server-sent event line. */
export type SseItem =
  | { kind: "delta"; text: string }
  | { kind: "finish"; text: string }
  | { kind: "tools"; pieces: ToolPiece[] }
  | { kind: "done" }
  | { kind: "error" }
  | { kind: "skip" };

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: unknown): string => (typeof v === "string" ? v : "");

/**
 * Parses one SSE line from `/v1/chat/completions`.
 * SECURITY: unknown shapes are skipped or refused, never run; text stays text (CWE-20, CWE-79).
 * @throws NetError `bad_answer` when a `data:` line is not JSON.
 */
export function parseSseLine(raw: string): SseItem {
  const line = raw.replace(/[\r\n]+$/, "");
  if (!line.startsWith("data:")) return { kind: "skip" }; // blank lines, `: keep-alive`, `event:` lines
  const data = line.slice(5).trimStart();
  if (data === "[DONE]") return { kind: "done" };
  let v: unknown;
  try {
    v = JSON.parse(data);
  } catch {
    throw new NetError("bad_answer");
  }
  if (!isObj(v)) return { kind: "skip" };
  if ("error" in v && v.error !== undefined && v.error !== null) return { kind: "error" };
  const choices = v.choices;
  if (!Array.isArray(choices) || choices.length === 0 || !isObj(choices[0])) return { kind: "skip" };
  const choice = choices[0];
  const delta = isObj(choice.delta) ? choice.delta : undefined;
  const tc = delta?.tool_calls;
  if (Array.isArray(tc) && tc.length > 0) {
    const pieces: ToolPiece[] = tc.map((c, pos) => {
      const o = isObj(c) ? c : {};
      const f = isObj(o.function) ? o.function : {};
      const a = f.arguments;
      // Ollama sends arguments as an object; OpenAI as a JSON string. Both end up as JSON text.
      const args = typeof a === "string" ? a : isObj(a) ? JSON.stringify(a) : "";
      const idx = typeof o.index === "number" && Number.isInteger(o.index) && o.index >= 0 ? o.index : pos;
      return { index: idx, id: str(o.id), name: str(f.name), args };
    });
    return { kind: "tools", pieces };
  }
  const content = delta?.content;
  if (typeof content === "string" && content.length > 0) return { kind: "delta", text: content };
  if (typeof choice.finish_reason === "string") return { kind: "finish", text: [...choice.finish_reason].slice(0, 32).join("") };
  return { kind: "skip" };
}

/** Parses `{data: [{id}]}` into sorted, unique, checked ids. @throws NetError `bad_answer`. */
export function parseModels(body: string): string[] {
  let v: unknown;
  try {
    v = JSON.parse(body);
  } catch {
    throw new NetError("bad_answer");
  }
  if (!isObj(v) || !Array.isArray(v.data)) throw new NetError("bad_answer");
  const ids = v.data
    .map((m) => (isObj(m) && typeof m.id === "string" ? m.id : ""))
    .filter(modelOk)
    .slice(0, MAX_MODELS);
  // Byte-order sort, like Rust's `sort()` on String.
  return [...new Set(ids)].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
}

/**
 * Checks and normalises a gateway URL (same rules as core/src/gateways.rs `check_url`).
 * Returns `scheme://host[:port]` lower-cased, or null when refused.
 * SECURITY: plain http only to loopback (INSECURE — DEV ONLY); nothing that could send the key elsewhere.
 */
export function checkUrl(url: string): string | null {
  const u = url.trim();
  if (u.length > 300) return null;
  let scheme: "https" | "http";
  let rest: string;
  if (u.startsWith("https://")) [scheme, rest] = ["https", u.slice(8)];
  else if (u.startsWith("http://")) [scheme, rest] = ["http", u.slice(7)];
  else return null;
  const hostport = rest.endsWith("/") ? rest.slice(0, -1) : rest;
  if (hostport === "" || /[/?#@\\% ]/.test(hostport)) return null;
  let host: string;
  let port: string | undefined;
  if (hostport.startsWith("[")) {
    const end = hostport.indexOf("]");
    if (end < 0) return null;
    host = hostport.slice(0, end + 1);
    const inner = host.slice(1, -1);
    if (inner === "" || !/^[0-9A-Fa-f:.]+$/.test(inner)) return null;
    const after = hostport.slice(end + 1);
    if (after !== "") {
      if (!after.startsWith(":")) return null;
      port = after.slice(1);
    }
  } else {
    const i = hostport.lastIndexOf(":");
    if (i >= 0) [host, port] = [hostport.slice(0, i), hostport.slice(i + 1)];
    else host = hostport;
    if (host === "" || host.length > 253 || !/^[A-Za-z0-9.-]+$/.test(host) || /^[-.]/.test(host) || host.endsWith("-")) {
      return null;
    }
  }
  if (port !== undefined) {
    if (!/^[0-9]{1,5}$/.test(port)) return null;
    const n = Number(port);
    if (n < 1 || n > 65535) return null;
    port = String(n);
  }
  const h = host.toLowerCase();
  const loopback = h === "127.0.0.1" || h === "localhost" || h === "[::1]";
  if (scheme === "http" && !loopback) return null;
  return port === undefined ? `${scheme}://${h}` : `${scheme}://${h}:${port}`;
}

/** Checks a key: 8-512 visible ASCII characters after trimming. Returns the trimmed key or null (CWE-113). */
export function checkKey(key: string): string | null {
  const k = key.trim();
  return k.length >= 8 && k.length <= 512 && /^[\x21-\x7e]+$/.test(k) ? k : null;
}

/** Checks a gateway display name: 1-64 characters on one line. */
export function checkName(name: string): string | null {
  const n = name.trim();
  return n.length >= 1 && [...n].length <= 64 && !/[\x00-\x1f\x7f-\x9f]/.test(n) ? n : null;
}
