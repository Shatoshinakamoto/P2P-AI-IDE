/**
 * client.ts -- the extension's HTTPS client for one gateway: model list and streamed chat (side track X1).
 *
 * Purpose: same two calls as the IDE's Rust client (core/src/client.rs):
 *   - `models()`      GET  /v1/models            -> {data: [{id}]}
 *   - `chatStream()`  POST /v1/chat/completions  with stream: true -> server-sent events
 * It runs in VS Code's extension host (Node). The chat view's webview never talks to a gateway: it posts messages
 * to the extension, which makes the request here with the key from VS Code's SecretStorage.
 *
 * Inputs / trust boundary: the gateway URL (checked by `checkUrl`), the key (from SecretStorage), the model and
 * conversation (from the webview: untrusted, checked here). Every gateway answer is untrusted: size-capped and
 * parsed into fixed shapes by checks.ts, and handed back as plain text.
 *
 * Security controls:
 *  - TLS for every non-loopback gateway; a pinned CA (PEM text), when saved, is passed as the only `ca` (CWE-295).
 *  - Redirects are never followed (Node's https doesn't, and 3xx is an error), so a gateway cannot bounce the key
 *    to another host (CWE-601, CWE-522). The request goes only to the checked URL's host.
 *  - The key is sent only as `Authorization: Bearer`, never logged or put in error text (CWE-532, CWE-209).
 *  - Caps: request 2 MiB, 200 messages, answer stream 8 MiB, one event line 1 MiB, model list 1 MiB / 1000 ids;
 *    15 s for the model list and 10 min for a chat turn; Stop aborts the request (CWE-400, CWE-770).
 * SECURITY-WARNING: VS Code may route extension requests through its configured proxy (`http.proxySupport`) and
 * may add OS certificates to the trust list. TLS stays end to end through a proxy, but a pinned CA may then not be
 * the only root. TODO(security): verify the peer chain against the pinned CA in `checkServerIdentity`.
 * Dependencies: Node's built-in `node:https` / `node:http` only.
 */
import * as http from "node:http";
import * as https from "node:https";
import {
  MAX_LINE_BYTES,
  MAX_MODELS_BYTES,
  MAX_REQUEST_BYTES,
  MAX_STREAM_BYTES,
  NetError,
  checkMessages,
  checkModel,
  checkStatus,
  checkUrl,
  parseModels,
  addPieces,
  parseSseLine,
  type Msg,
  type ToolCall,
} from "./checks.ts";

/** How a streamed turn ended. */
export interface StreamEnd {
  /** `stop`, `length`, ... as the gateway said, or null. */
  finishReason: string | null;
  /** Bytes of answer text received. */
  chars: number;
}

/** What the client needs about a gateway. */
export interface GatewayTarget {
  url: string;
  key: string | undefined;
  /** PEM text of a pinned CA, or undefined for the normal trust roots. */
  caPem?: string;
}

const MODELS_TIMEOUT_MS = 15_000;
const CHAT_TIMEOUT_MS = 600_000;
const CONNECT_TIMEOUT_MS = 10_000;

/** Maps a Node request error to a fixed kind (never its message, which can hold the host). */
function mapError(e: unknown): NetError {
  if (e instanceof NetError) return e;
  const code = (e as { code?: string })?.code ?? "";
  if (code === "ETIMEDOUT" || code === "ESOCKETTIMEDOUT") return new NetError("timeout");
  if (code.startsWith("ERR_TLS") || code.startsWith("CERT_") || code.includes("SELF_SIGNED") || code === "UNABLE_TO_VERIFY_LEAF_SIGNATURE" || code === "ERR_SSL_WRONG_VERSION_NUMBER") {
    return new NetError("tls");
  }
  return new NetError("connect");
}

export class GatewayClient {
  private readonly base: URL;
  private readonly key: string;
  private readonly caPem: string | undefined;

  /** @throws NetError `no_key` without a key, `bad_request` for a URL that fails `checkUrl`. */
  constructor(t: GatewayTarget) {
    if (!t.key) throw new NetError("no_key");
    const url = checkUrl(t.url);
    // SECURITY: the URL is re-checked here, so a stored value edited by hand can't reach another scheme (CWE-20).
    if (url === null) throw new NetError("bad_request", "the gateway address is not valid");
    this.base = new URL(url);
    this.key = t.key;
    this.caPem = t.caPem;
  }

  /** Lists the gateway's model ids and the round-trip time in milliseconds. */
  async models(signal?: AbortSignal): Promise<{ ids: string[]; ms: number }> {
    const t0 = Date.now();
    const res = await this.request("GET", "/v1/models", undefined, "application/json", MODELS_TIMEOUT_MS, signal);
    const ms = Date.now() - t0;
    const body = await readCapped(res, MAX_MODELS_BYTES);
    return { ids: parseModels(body.toString("utf8")), ms };
  }

  /**
   * Sends a conversation and calls `onDelta` with each piece of answer text as it arrives.
   * Abort `signal` to stop; the turn then ends with NetError `stopped`.
   */
  async chatStream(model: string, messages: Msg[], onDelta: (t: string) => void, signal?: AbortSignal): Promise<StreamEnd> {
    checkModel(model);
    checkMessages(messages);
    // Only the checked fields are sent, whatever else the objects carry.
    const body = JSON.stringify({ model, messages: messages.map((m) => ({ role: m.role, content: m.content })), stream: true });
    if (Buffer.byteLength(body) > MAX_REQUEST_BYTES) {
      throw new NetError("bad_request", "the chat is larger than 2 MiB; start a new chat");
    }
    const res = await this.request("POST", "/v1/chat/completions", body, "text/event-stream", CHAT_TIMEOUT_MS, signal);
    return readStream(res, onDelta, signal);
  }

  /**
   * One step of an agent turn (phase X2, same as core/src/client.rs `agent_step`): sends `messages` (built and
   * checked by the agent) with the tool definitions, streams answer text to `onDelta`, and returns the end and any
   * tool calls.
   * SECURITY: same transport controls as `chatStream`; the body is capped at 2 MiB; tool calls come back as untrusted
   * text for the agent to check (CWE-20, CWE-400).
   */
  async agentStep(
    model: string,
    messages: unknown[],
    tools: unknown,
    onDelta: (t: string) => void,
    signal?: AbortSignal,
  ): Promise<{ end: StreamEnd; calls: ToolCall[] }> {
    checkModel(model);
    const body = JSON.stringify({ model, messages, tools, stream: true });
    if (Buffer.byteLength(body) > MAX_REQUEST_BYTES) {
      throw new NetError("bad_request", "the chat and the files read are larger than 2 MiB; start a new chat");
    }
    const res = await this.request("POST", "/v1/chat/completions", body, "text/event-stream", CHAT_TIMEOUT_MS, signal);
    const calls: ToolCall[] = [];
    const end = await readStream(res, onDelta, signal, calls);
    return { end, calls };
  }

  /** Makes one request to a fixed path on the gateway and checks its status. */
  private request(
    method: "GET" | "POST",
    path: "/v1/models" | "/v1/chat/completions",
    body: string | undefined,
    accept: string,
    totalMs: number,
    signal?: AbortSignal,
  ): Promise<http.IncomingMessage> {
    return new Promise((resolve, reject) => {
      if (signal?.aborted) return reject(new NetError("stopped"));
      const secure = this.base.protocol === "https:";
      const headers: Record<string, string | number> = {
        // SECURITY: the key goes only in this header, to this host (CWE-522).
        Authorization: `Bearer ${this.key}`,
        Accept: accept,
        "User-Agent": "p2p-ai-vscode/0.1",
      };
      if (body !== undefined) {
        headers["Content-Type"] = "application/json";
        headers["Content-Length"] = Buffer.byteLength(body);
      }
      const opts: https.RequestOptions = {
        method,
        protocol: this.base.protocol,
        hostname: this.base.hostname.replace(/^\[(.*)\]$/, "$1"),
        port: this.base.port || (secure ? 443 : 80),
        path,
        headers,
        // SECURITY: a fresh agent (no shared global agent), and the pinned CA as the only root when one is saved.
        agent: secure ? new https.Agent({ keepAlive: false, ca: this.caPem }) : new http.Agent({ keepAlive: false }),
        timeout: CONNECT_TIMEOUT_MS,
      };
      const req = (secure ? https : http).request(opts, (res) => {
        clearTimeout(total);
        try {
          checkStatus(res.statusCode ?? 0);
        } catch (e) {
          res.resume();
          return reject(e);
        }
        resolve(res);
      });
      // A turn may stream for up to totalMs; the socket timeout covers connecting and silence.
      const total = setTimeout(() => req.destroy(Object.assign(new Error("timeout"), { code: "ETIMEDOUT" })), totalMs);
      req.on("timeout", () => req.destroy(Object.assign(new Error("timeout"), { code: "ETIMEDOUT" })));
      req.on("error", (e) => {
        clearTimeout(total);
        reject(signal?.aborted ? new NetError("stopped") : mapError(e));
      });
      signal?.addEventListener("abort", () => req.destroy(new NetError("stopped")), { once: true });
      if (body !== undefined) req.end(body);
      else req.end();
    });
  }
}

/** Reads a whole answer up to `max` bytes. @throws NetError `too_large`. */
export async function readCapped(res: NodeJS.ReadableStream, max: number): Promise<Buffer> {
  const parts: Buffer[] = [];
  let n = 0;
  for await (const chunk of res) {
    const b = typeof chunk === "string" ? Buffer.from(chunk) : (chunk as Buffer);
    n += b.length;
    // SECURITY: stop reading as soon as the cap is passed (CWE-400).
    if (n > max) {
      (res as http.IncomingMessage).destroy?.();
      throw new NetError("too_large");
    }
    parts.push(b);
  }
  return Buffer.concat(parts);
}

/**
 * Reads an SSE body line by line (split out so tests can feed it bytes). Answers without `[DONE]` are kept,
 * as some gateways close early. With `calls`, streamed tool-call pieces are collected into it (phase X2);
 * without it they are ignored (plain chat).
 */
export async function readStream(
  res: NodeJS.ReadableStream,
  onDelta: (t: string) => void,
  signal?: AbortSignal,
  calls?: ToolCall[],
): Promise<StreamEnd> {
  const end: StreamEnd = { finishReason: null, chars: 0 };
  let total = 0;
  let pending = Buffer.alloc(0);
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const handle = (lineBytes: Buffer): "done" | undefined => {
    let text: string;
    try {
      text = decoder.decode(lineBytes);
    } catch {
      throw new NetError("bad_answer");
    }
    const item = parseSseLine(text);
    switch (item.kind) {
      case "delta":
        end.chars += Buffer.byteLength(item.text);
        onDelta(item.text);
        return undefined;
      case "finish":
        end.finishReason = item.text;
        return undefined;
      case "done":
        return "done";
      case "error":
        throw new NetError("upstream");
      case "tools":
        if (calls) addPieces(calls, item.pieces);
        return undefined; // ignored in a plain chat
      default:
        return undefined;
    }
  };
  try {
    for await (const chunk of res) {
      if (signal?.aborted) throw new NetError("stopped");
      const b = typeof chunk === "string" ? Buffer.from(chunk) : (chunk as Buffer);
      total += b.length;
      // SECURITY: the whole stream and each line are capped while they arrive (CWE-400).
      if (total > MAX_STREAM_BYTES) throw new NetError("too_large");
      pending = Buffer.concat([pending, b]);
      let nl: number;
      while ((nl = pending.indexOf(0x0a)) >= 0) {
        const line = pending.subarray(0, nl + 1);
        pending = pending.subarray(nl + 1);
        if (line.length > MAX_LINE_BYTES + 1) throw new NetError("too_large");
        if (handle(line) === "done") {
          (res as http.IncomingMessage).destroy?.();
          return end;
        }
      }
      if (pending.length > MAX_LINE_BYTES) throw new NetError("too_large");
    }
  } catch (e) {
    (res as http.IncomingMessage).destroy?.();
    if (signal?.aborted) throw new NetError("stopped");
    if (e instanceof NetError) throw e;
    throw mapError(e);
  }
  if (pending.length > 0) handle(pending);
  return end;
}
