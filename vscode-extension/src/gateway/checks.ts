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
  | "stopped"
  /** A known gateway refusal (X3 missions); `detail` is the fixed message chosen by `codeMessage`. */
  | "gateway";

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
    case "gateway":
      return String(detail);
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

/** One streamed piece of a tool call (`delta.tool_calls[]`). */
export interface ToolPiece {
  index: number;
  id: string;
  name: string;
  args: string;
}

/** A complete tool call the model asked for (phase X2). Untrusted: the agent checks the name and arguments. */
export interface ToolCall {
  id: string;
  name: string;
  /** The arguments as the JSON text the model wrote. */
  arguments: string;
}

/** Most tool calls accepted in one answer, and the largest arguments text of one call (same as the Rust client). */
export const MAX_TOOL_CALLS = 8;
export const MAX_TOOL_ARGS_BYTES = 1 << 20;

/**
 * Adds streamed tool-call pieces to the calls collected so far, by index (same as core/src/client.rs `add_pieces`).
 * SECURITY: at most 8 calls, ids and names cut to 64 characters, arguments capped at 1 MiB while they stream in
 * (CWE-400). @throws NetError `too_large`.
 */
export function addPieces(calls: ToolCall[], pieces: ToolPiece[]): void {
  for (const p of pieces) {
    if (p.index >= MAX_TOOL_CALLS) throw new NetError("too_large");
    while (calls.length <= p.index) calls.push({ id: "", name: "", arguments: "" });
    const c = calls[p.index];
    if (c.id === "" && p.id !== "") c.id = [...p.id].slice(0, 64).join("");
    if (c.name === "" && p.name !== "") c.name = [...p.name].slice(0, 64).join("");
    c.arguments += p.args;
    if (byteLen(c.arguments) > MAX_TOOL_ARGS_BYTES) throw new NetError("too_large");
  }
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

/**
 * Maps a gateway error `code` (from `{"error": {"code": ...}}`) to a fixed message (X3; same table as the IDE's
 * core/src/gwapi.rs `code_message`, checked by the shared cases). Unknown codes give undefined.
 * SECURITY: the gateway's own message text is never shown (CWE-209, CWE-117).
 */
export function codeMessage(code: string): string | undefined {
  return CODE_MESSAGES[code];
}

const CODE_MESSAGES: Record<string, string> = Object.assign(Object.create(null), {
  version_conflict: "the gateway project changed meanwhile; try again",
  project_toggle_off: "the project owner has not switched this on for the project",
  experimental_disabled: "the gateway's administrator has switched missions off",
  not_available: "this gateway does not offer that feature",
  exec_not_allowed: "this gateway does not run code for your key, so Autopilot is not available",
  mission_exists: "this project already has a mission of this kind; stop it first",
  mission_limit: "the most missions allowed are already running; try later",
  invalid_state: "the mission cannot do that in its current state",
  attachment_not_ready: "a file is still being scanned on the gateway; try again shortly",
  quota_exceeded: "the project owner's storage quota on the gateway is used up",
  unsupported_media_type: "the gateway does not accept this file type",
  too_large: "too large for the gateway",
  sync_in_progress: "a copy of the project to a node is still running",
  delta_in_progress: "the node's changes are already being collected",
  no_copy: "no node holds a copy of this project yet; send the folder first",
  copy_stale: "the node that held the copy left; send the folder again",
  delta_unsupported: "the node that holds the copy cannot send changes back",
  no_capacity: "no node can take a copy of this project right now",
  busy: "the gateway is busy; try again shortly",
  service_unavailable: "this gateway has that feature switched off",
  approval_not_pending: "that approval was already decided or has expired",
  arguments_mismatch: "the approval changed; open it again",
  auto_approve_not_allowed: "this gateway does not allow auto-approve",
  assist_model_not_allowed: "this gateway does not allow that assist model",
  limit_reached: "your key has reached the gateway's project limit",
  forbidden: "your role in this project does not allow that",
  not_found: "not found on the gateway (it may have been deleted)",
  invalid_request: "the gateway refused the request as invalid",
});

/**
 * Turns a non-2xx answer into an error: a known code gives its fixed message (except 401, which stays "refused the
 * key"), else the status mapping (same as the IDE's `answer_error`).
 */
export function answerError(status: number, body: string): NetError {
  let code: unknown;
  try {
    code = (JSON.parse(body) as { error?: { code?: unknown } })?.error?.code;
  } catch {
    code = undefined;
  }
  const m = typeof code === "string" ? codeMessage(code) : undefined;
  if (m !== undefined && status !== 401) return new NetError("gateway", m);
  try {
    checkStatus(status);
  } catch (e) {
    return e as NetError;
  }
  return new NetError("status", status);
}

/** True only for `http://127.0.0.1:<port>` where `<port>` is canonical digits in `1024..=65535` (CWE-918). */
export function isNexusLoopbackUrl(url: string): boolean {
  const rest = url.startsWith("http://127.0.0.1:") ? url.slice("http://127.0.0.1:".length) : "";
  if (!/^[1-9]\d{3,4}$/.test(rest)) return false;
  const port = Number(rest);
  return port >= 1024 && port <= 65535;
}
