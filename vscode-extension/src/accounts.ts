/**
 * accounts.ts -- "Sign in with a P2P-AI account" for the VS Code / VSCodium extension (universal accounts, P2P-AI
 * ADR-030 phase A3; IDE docs/features/account-sign-in.md).
 *
 * Purpose: a TypeScript port of the IDE core's `core/src/accounts.rs`, so the extension signs in the same way and a
 * saved gateway can be linked to an account (the account service address, the "home", plus the gateway's account id)
 * instead of a pasted key. Before a gateway call the extension asks the home for a 5-minute token for that gateway.
 * The extension uses the IDE's built-in client id `p2p-ai-ide` (it is the IDE family's own client).
 *
 * Flow (OAuth 2.0 for native apps, RFC 8252, with PKCE, RFC 7636):
 *   1. A one-shot listener on `http://127.0.0.1:<random port>/cb` (the IP literal, never `localhost`, RFC 8252 8.3).
 *   2. A PKCE verifier + S256 challenge and a `state`; the home's sign-in page opens in the system browser
 *      (`vscode.env.openExternal`, never a webview, RFC 8252 8.12), so the extension never sees the password.
 *   3. The home sends the browser back to `/cb?code&state&iss`; `state` (constant time) and `iss` (RFC 9207) are
 *      checked, repeated parameters refused.
 *   4. `POST /token` trades code + verifier for a refresh token (VS Code SecretStorage only) and an account token
 *      (used once for `/v1/me`).
 *   5. `tokenFor` refreshes for `resource=<gateway id>`; the home rotates the refresh token on every use, so the new
 *      one is stored first and every refresh runs in one queue (a reused old token ends the session, RFC 9700).
 *
 * Inputs / trust boundary: addresses, gateway ids and passwords typed into VS Code input boxes; the browser redirect
 * to the loopback listener (any local process can reach it: path, `state` and `iss` are checked, connections capped);
 * the home's answers (untrusted JSON, size-capped, parsed into fixed fields); the stored home list (re-checked).
 *
 * Security controls:
 *  - Refresh tokens only in SecretStorage (the OS keychain where VS Code has one); access tokens only in memory.
 *    Neither goes to a webview, the state file, logs or error text (CWE-312, CWE-522, CWE-532, NIST IA-5).
 *  - Homes: https (plain http only for loopback, INSECURE — DEV ONLY), no user, query or fragment, allow-listed path;
 *    a fresh agent per request, no redirects followed, 64 KiB answers, 10 s connect / 20 s total (NIST SC-8, CWE-295,
 *    CWE-601, CWE-918, CWE-400).
 *  - Gateway tokens are scoped to one gateway id and `chat projects:read projects:write missions` (NIST AC-6); a
 *    refresh answer for another audience is refused.
 *  - Fixed error messages, the same words as the IDE; the home's own text is never shown (CWE-209).
 * SECURITY-WARNING: as for gateway calls (gateway/client.ts), VS Code may route these requests through its configured
 * proxy; TLS stays end to end.
 * Dependencies: Node's built-in `node:http`, `node:https`, `node:crypto`; ./gateway/checks.ts. No vscode import, so the
 * unit tests run in plain Node.
 */
import * as http from "node:http";
import * as https from "node:https";
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import type { AddressInfo } from "node:net";
import { checkUrl } from "./gateway/checks.ts";

/** The IDE family's built-in client at every P2P-AI account service. */
export const CLIENT_ID = "p2p-ai-ide";
/** Asked at sign-in: everything a gateway token may need, plus the home's own account API. */
export const SIGN_IN_SCOPE = "chat projects:read projects:write missions account";
/** Asked for a gateway token (least privilege: no `account`). */
export const GATEWAY_SCOPE = "chat projects:read projects:write missions";
export const SIGN_IN_TIMEOUT_MS = 5 * 60_000;
export const MAX_HOMES = 16;
const CB_PATH = "/cb";
const MAX_ANSWER = 64 << 10;
const MAX_CB_CONNECTIONS = 32;
const HOMES_KEY = "p2pAi.accounts";
const secretName = (id: string): string => `p2pAi.accountRefresh.${id}`;

/** Fixed error kinds; the messages are the IDE's (core/src/accounts.rs `AcctError::message`). */
export type AcctErrorKind =
  | "bad_home" | "bad_audience" | "not_signed_in" | "signed_out" | "password_change" | "not_for_gateway" | "no_scope"
  | "cancelled" | "denied" | "timed_out" | "listener" | "browser" | "connect" | "tls" | "busy" | "bad_answer"
  | "status" | "bad_password" | "too_many" | "store";

/** An error safe to show: never holds a token, password or the home's own text (CWE-209). */
export class AcctError extends Error {
  readonly kind: AcctErrorKind;
  constructor(kind: AcctErrorKind, status = 0) {
    super(acctMessage(kind, status));
    this.name = "AcctError";
    this.kind = kind;
  }
}

export function acctMessage(kind: AcctErrorKind, status = 0): string {
  switch (kind) {
    case "bad_home": return "the account service address must be https://host[:port][/path] (or http://127.0.0.1:port for a dev service)";
    case "bad_audience": return "the gateway's account id must be 3 to 128 characters: a-z, 0-9, '.', '_', ':' or '-'";
    case "not_signed_in": return "sign in to this account service first";
    case "signed_out": return "the sign-in has ended; sign in again";
    case "password_change": return "this account must choose its own password first ('P2P-AI: Change Account Password')";
    case "not_for_gateway": return "the account service does not issue tokens for this gateway (check the gateway's account id)";
    case "no_scope": return "the sign-in was not granted what this gateway needs; sign in again";
    case "cancelled": return "sign-in cancelled";
    case "denied": return "the sign-in was refused or cancelled in the browser";
    case "timed_out": return "the sign-in took longer than 5 minutes; try again";
    case "listener": return "could not open the local sign-in return address";
    case "browser": return "could not open the system browser";
    case "connect": return "could not reach the account service";
    case "tls": return "the account service's certificate is not trusted";
    case "busy": return "the account service is limiting requests; try again shortly";
    case "bad_answer": return "the account service's answer could not be read";
    case "status": return `the account service answered with an error (HTTP ${status})`;
    case "bad_password": return "the password was not accepted (check the current one; the new one needs 15+ characters and must not be a common password)";
    case "too_many": return "at most 16 account services can be remembered";
    case "store": return "the saved sign-ins could not be read or written";
  }
}

// ---------------------------------------------------------------------------------------------------------------
// Checks (same rules as the IDE core)
// ---------------------------------------------------------------------------------------------------------------

/**
 * Checks and normalises an account service address: `https://host[:port]` plus an optional path of
 * `[A-Za-z0-9._~-]` segments; no trailing slash, query, fragment, user info or `..`. Returns null when refused.
 * SECURITY: the origin goes through the gateway URL allow-list (CWE-20, CWE-918); plain http only for loopback.
 */
export function checkHome(raw: string): string | null {
  if (typeof raw !== "string") return null;
  const u = raw.trim().replace(/\/+$/, "");
  if (u.length > 300) return null;
  const sep = u.indexOf("://");
  if (sep < 0) return null;
  const slash = u.indexOf("/", sep + 3);
  const origin = slash < 0 ? u : u.slice(0, slash);
  const path = slash < 0 ? "" : u.slice(slash);
  const o = checkUrl(origin);
  if (o === null) return null;
  if (path !== "") {
    const ok = path.split("/").slice(1).every((s) => s !== "" && s !== "." && s !== ".." && /^[A-Za-z0-9._~-]+$/.test(s));
    if (!ok) return null;
  }
  return o + path;
}

/** Checks a gateway's account id (its `GATEWAY_ID`), the Go `ValidAudience` rule. Returns null when refused. */
export function checkAudience(raw: string): string | null {
  if (typeof raw !== "string") return null;
  const a = raw.trim();
  return /^[a-z0-9][a-z0-9._:-]{2,127}$/.test(a) ? a : null;
}

/** The SecretStorage id of a home: the first 16 hex of SHA-256 of its normalised address (the IDE's `home_id`). */
export function homeId(issuer: string): string {
  return createHash("sha256").update(issuer).digest("hex").slice(0, 16);
}

/** Which account a gateway uses; saved with the gateway (no secret in it). */
export interface AccountLink {
  home: string;
  audience: string;
}

/** Both fields checked and normalised, or the error to show. */
export function makeLink(home: string, audience: string): AccountLink | AcctError {
  const h = checkHome(home);
  if (h === null) return new AcctError("bad_home");
  const a = checkAudience(audience);
  if (a === null) return new AcctError("bad_audience");
  return { home: h, audience: a };
}

/** True for a stored link whose fields are already normalised (stored values are untrusted too, CWE-20). */
export function linkValid(l: unknown): l is AccountLink {
  if (typeof l !== "object" || l === null) return false;
  const { home, audience } = l as Record<string, unknown>;
  return typeof home === "string" && typeof audience === "string" && checkHome(home) === home && checkAudience(audience) === audience;
}

// ---------------------------------------------------------------------------------------------------------------
// PKCE, the sign-in link and the callback
// ---------------------------------------------------------------------------------------------------------------

const b64url = (b: Buffer): string => b.toString("base64url");

/** One attempt's secrets: a 32-byte verifier (43 characters), its S256 challenge, a 32-byte `state` (CWE-338). */
export function newPkce(): { verifier: string; challenge: string; state: string } {
  const verifier = b64url(randomBytes(32));
  return { verifier, challenge: b64url(createHash("sha256").update(verifier).digest()), state: b64url(randomBytes(32)) };
}

/** Percent-encodes a query value (RFC 3986 unreserved characters kept), as the IDE does. */
function enc(s: string): string {
  return encodeURIComponent(s).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
}

/** The home's sign-in link for one attempt. */
export function authorizeUrl(issuer: string, redirect: string, p: { challenge: string; state: string }): string {
  return `${issuer}/authorize?response_type=code&client_id=${enc(CLIENT_ID)}&redirect_uri=${enc(redirect)}` +
    `&code_challenge=${enc(p.challenge)}&code_challenge_method=S256&state=${enc(p.state)}&scope=${enc(SIGN_IN_SCOPE)}`;
}

export type Callback = { kind: "code"; code: string } | { kind: "error" } | { kind: "ignore" };

/** Constant-time string comparison (CWE-208). */
function ctEq(a: string, b: string): boolean {
  const x = Buffer.from(a);
  const y = Buffer.from(b);
  return x.length === y.length && timingSafeEqual(x, y);
}

/**
 * Parses one request to the listener. Pure, so it is tested without sockets.
 * SECURITY: only `GET /cb`; a repeated parameter is refused (CWE-235); `state` compared in constant time (CWE-352);
 * `iss` must equal the home (RFC 9207 mix-up defence).
 */
export function parseCallback(method: string, target: string, state: string, issuer: string): Callback {
  if (method !== "GET") return { kind: "ignore" };
  const q = target.indexOf("?");
  const path = q < 0 ? target : target.slice(0, q);
  if (path !== CB_PATH) return { kind: "ignore" };
  const m = new Map<string, string>();
  for (const pair of (q < 0 ? "" : target.slice(q + 1)).split("&").filter((p) => p !== "")) {
    const i = pair.indexOf("=");
    const k = i < 0 ? pair : pair.slice(0, i);
    let v: string;
    try {
      v = decodeURIComponent((i < 0 ? "" : pair.slice(i + 1)).replace(/\+/g, " "));
    } catch {
      return { kind: "ignore" };
    }
    if (m.has(k)) return { kind: "ignore" };
    m.set(k, v);
  }
  const st = m.get("state");
  if (st === undefined || !ctEq(st, state)) return { kind: "ignore" };
  if (m.has("error")) return { kind: "error" };
  if (m.get("iss") !== issuer) return { kind: "ignore" };
  const code = m.get("code") ?? "";
  return code.length >= 16 && code.length <= 512 && /^[\x21-\x7e]+$/.test(code) ? { kind: "code", code } : { kind: "ignore" };
}

/** A fixed answer page. SECURITY: fixed text, no script may run on it (CWE-79). */
function page(res: http.ServerResponse, status: number, title: string, text: string): void {
  const body = `<!doctype html><html><head><meta charset="utf-8"><title>${title}</title></head>` +
    `<body style="font-family:sans-serif;max-width:32rem;margin:4rem auto"><h1>${title}</h1><p>${text}</p></body></html>`;
  res.writeHead(status, {
    "Content-Type": "text/html; charset=utf-8",
    "Cache-Control": "no-store",
    "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'",
    "Referrer-Policy": "no-referrer",
    "X-Content-Type-Options": "nosniff",
    Connection: "close",
  });
  res.end(body);
}

/** The one-shot local return address. `wait` resolves with the code, or rejects with an AcctError. */
export class Loopback {
  readonly redirect: string;
  private readonly server: http.Server;
  /** Ends a running `wait` (set while one runs). */
  private stop: ((err: AcctError) => void) | undefined;
  private constructor(server: http.Server, port: number) {
    this.server = server;
    this.redirect = `http://127.0.0.1:${port}${CB_PATH}`;
  }

  /** Binds `127.0.0.1:0`. SECURITY: the IP literal, never `localhost` (RFC 8252 8.3). */
  static async bind(): Promise<Loopback> {
    // SECURITY: small heads, short timeouts (CWE-400).
    const server = http.createServer({ maxHeaderSize: 8 << 10, requestTimeout: 5_000, headersTimeout: 5_000 });
    await new Promise<void>((resolve, reject) => {
      server.once("error", () => reject(new AcctError("listener")));
      server.listen(0, "127.0.0.1", () => resolve());
    });
    return new Loopback(server, (server.address() as AddressInfo).port);
  }

  /**
   * Waits for the browser, until `timeoutMs` or `signal`. Closes the listener in every case. Call it before the
   * browser is opened: a request that arrives before the handler is attached would never be answered.
   */
  wait(state: string, issuer: string, timeoutMs: number, signal?: AbortSignal): Promise<string> {
    return new Promise<string>((resolve, reject) => {
      let served = 0;
      let done = false;
      const finish = (err: AcctError | null, code?: string): void => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        signal?.removeEventListener("abort", onAbort);
        this.server.closeAllConnections();
        this.server.close();
        if (err) reject(err);
        else resolve(code as string);
      };
      const timer = setTimeout(() => finish(new AcctError("timed_out")), timeoutMs);
      this.stop = (err) => finish(err);
      const onAbort = (): void => finish(new AcctError("cancelled"));
      if (signal?.aborted) return onAbort();
      signal?.addEventListener("abort", onAbort);
      this.server.on("request", (req, res) => {
        served++;
        const cb = parseCallback(req.method ?? "", req.url ?? "", state, issuer);
        if (cb.kind === "code") {
          page(res, 200, "Signed in", "You are signed in to the P2P-AI extension. You can close this tab.");
          finish(null, cb.code);
        } else if (cb.kind === "error") {
          page(res, 200, "Sign-in not finished", "The sign-in was cancelled. You can close this tab.");
          finish(new AcctError("denied"));
        } else {
          page(res, 404, "Not found", "Nothing here.");
          // SECURITY: a flood of wrong requests ends the wait (CWE-400).
          if (served >= MAX_CB_CONNECTIONS) finish(new AcctError("listener"));
        }
      });
    });
  }

  /** Ends a running wait with `err` (or just closes the listener). */
  close(err: AcctError = new AcctError("cancelled")): void {
    if (this.stop) return this.stop(err);
    this.server.closeAllConnections();
    this.server.close();
  }
}

// ---------------------------------------------------------------------------------------------------------------
// The home's API
// ---------------------------------------------------------------------------------------------------------------

/** One request to the home: fresh agent, no redirects, capped answer (CWE-400, CWE-601). */
function call(issuer: string, method: string, path: string, headers: Record<string, string>, body?: string): Promise<{ status: number; json: Record<string, unknown> }> {
  const url = new URL(issuer + path);
  const secure = url.protocol === "https:";
  const mod = secure ? https : http;
  return new Promise((resolve, reject) => {
    const req = mod.request(url, {
      method,
      headers: { Accept: "application/json", "User-Agent": "p2p-ai-vscode/0.4", ...headers, ...(body !== undefined ? { "Content-Length": String(Buffer.byteLength(body)) } : {}) },
      agent: secure ? new https.Agent({ keepAlive: false }) : new http.Agent({ keepAlive: false }),
      timeout: 10_000,
    }, (res) => {
      const chunks: Buffer[] = [];
      let n = 0;
      res.on("data", (c: Buffer) => {
        n += c.length;
        if (n > MAX_ANSWER) {
          req.destroy();
          reject(new AcctError("bad_answer"));
        } else chunks.push(c);
      });
      res.on("end", () => {
        let json: Record<string, unknown> = {};
        try {
          const v = JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
          if (typeof v === "object" && v !== null && !Array.isArray(v)) json = v;
        } catch {
          /* not JSON: fixed shapes below */
        }
        resolve({ status: res.statusCode ?? 0, json });
      });
      res.on("error", () => reject(new AcctError("connect")));
    });
    const total = setTimeout(() => req.destroy(new Error("timeout")), 20_000);
    req.on("close", () => clearTimeout(total));
    req.on("timeout", () => req.destroy(new Error("timeout")));
    req.on("error", (e: NodeJS.ErrnoException) => {
      const tls = /CERT|SSL|TLS|SELF_SIGNED/i.test(e.code ?? "") || /certificate/i.test(e.message ?? "");
      reject(new AcctError(tls ? "tls" : "connect"));
    });
    if (body !== undefined) req.write(body);
    req.end();
  });
}

function form(issuer: string, path: string, fields: Record<string, string>) {
  const body = Object.entries(fields).map(([k, v]) => `${enc(k)}=${enc(v)}`).join("&");
  return call(issuer, "POST", path, { "Content-Type": "application/x-www-form-urlencoded" }, body);
}

/** Our tokens are short visible ASCII (`p2prt_...` or a JWT). SECURITY: also keeps them out of headers' way (CWE-113). */
const tokenShape = (t: unknown): t is string => typeof t === "string" && t.length >= 16 && t.length <= 4096 && /^[\x21-\x7e]+$/.test(t);

/** Maps a token-endpoint refusal to a fixed error (the home's text is never shown, CWE-209). */
export function tokenError(status: number, j: Record<string, unknown>): AcctError {
  const code = typeof j.error === "string" ? j.error : "";
  if (code === "invalid_grant") return new AcctError("signed_out");
  if (code === "password_change_required") return new AcctError("password_change");
  if (code === "invalid_target") return new AcctError("not_for_gateway");
  if (code === "invalid_scope") return new AcctError("no_scope");
  if (status === 429) return new AcctError("busy");
  if (status === 401) return new AcctError("signed_out");
  return new AcctError("status", status);
}

interface Refreshed { refresh: string; access: string; expiresIn: number; scope: string }

async function exchange(issuer: string, code: string, redirect: string, verifier: string) {
  const r = await form(issuer, "/token", { grant_type: "authorization_code", client_id: CLIENT_ID, code, redirect_uri: redirect, code_verifier: verifier, resource: issuer });
  if (r.status !== 200) throw tokenError(r.status, r.json);
  if (!tokenShape(r.json.refresh_token)) throw new AcctError("bad_answer");
  return {
    refresh: r.json.refresh_token,
    account: tokenShape(r.json.access_token) ? r.json.access_token : undefined,
    mustChange: r.json.password_change_required === true,
  };
}

async function refresh(issuer: string, rt: string, resource: string, scope: string): Promise<Refreshed> {
  const r = await form(issuer, "/token", { grant_type: "refresh_token", client_id: CLIENT_ID, refresh_token: rt, resource, scope });
  if (r.status !== 200) throw tokenError(r.status, r.json);
  const { refresh_token, access_token, audience, expires_in } = r.json;
  if (!tokenShape(refresh_token) || !tokenShape(access_token)) throw new AcctError("bad_answer");
  // A confused home must not hand us another gateway's token.
  if (typeof audience === "string" && audience !== resource) throw new AcctError("bad_answer");
  const exp = typeof expires_in === "number" && Number.isFinite(expires_in) ? Math.min(600, Math.max(10, Math.floor(expires_in))) : 60;
  return { refresh: refresh_token, access: access_token, expiresIn: exp, scope: typeof r.json.scope === "string" ? r.json.scope.slice(0, 200) : "" };
}

const clean = (v: unknown, n: number): string => (typeof v === "string" ? v.replace(/[\x00-\x1f\x7f]/g, "").slice(0, n) : "");

async function me(issuer: string, accountToken: string): Promise<string> {
  const r = await call(issuer, "GET", "/v1/me", { Authorization: `Bearer ${accountToken}` });
  if (r.status !== 200) throw tokenError(r.status, r.json);
  const email = clean(r.json.email, 254);
  if (!email) throw new AcctError("bad_answer");
  return email;
}

// ---------------------------------------------------------------------------------------------------------------
// Remembered homes and the token cache
// ---------------------------------------------------------------------------------------------------------------

/** VS Code's SecretStorage, or a fake in tests. */
export interface Secrets {
  get(key: string): Thenable<string | undefined>;
  store(key: string, value: string): Thenable<void>;
  delete(key: string): Thenable<void>;
}
/** The extension's state file (fileState.ts), or a fake in tests. Holds no secret. */
export interface StateBox {
  get<T>(key: string): T | undefined;
  update(key: string, value: unknown): Thenable<void>;
}

interface HomeRecord { id: string; issuer: string; email: string; mustChange: boolean }

/** What the commands show of a home. */
export interface HomeView { id: string; issuer: string; email: string; dev: boolean; signedIn: boolean; mustChange: boolean }

export class Accounts {
  private readonly secrets: Secrets;
  private readonly state: StateBox;
  /** `${home id} ${audience}` -> token and expiry. SECURITY: memory only. */
  private readonly cache = new Map<string, { token: string; until: number }>();
  /** Every use of a refresh token runs in this queue (rotation must never race). */
  private queue: Promise<unknown> = Promise.resolve();

  constructor(secrets: Secrets, state: StateBox) {
    this.secrets = secrets;
    this.state = state;
  }

  private locked<T>(fn: () => Promise<T>): Promise<T> {
    const run = this.queue.then(fn, fn);
    this.queue = run.catch(() => undefined);
    return run;
  }

  /** The remembered homes, re-checked (an edited state file cannot point a secret elsewhere, CWE-20). */
  private homes(): HomeRecord[] {
    const raw = this.state.get<unknown>(HOMES_KEY);
    if (!Array.isArray(raw)) return [];
    const out: HomeRecord[] = [];
    for (const h of raw.slice(0, MAX_HOMES)) {
      if (typeof h !== "object" || h === null) continue;
      const { id, issuer, email, mustChange } = h as Record<string, unknown>;
      if (typeof issuer !== "string" || checkHome(issuer) !== issuer || id !== homeId(issuer)) continue;
      out.push({ id: homeId(issuer), issuer, email: clean(email, 254), mustChange: mustChange === true });
    }
    return out;
  }

  private async upsert(rec: HomeRecord): Promise<void> {
    const all = this.homes();
    const i = all.findIndex((h) => h.id === rec.id);
    if (i >= 0) all[i] = rec;
    else if (all.length >= MAX_HOMES) throw new AcctError("too_many");
    else all.push(rec);
    await this.state.update(HOMES_KEY, all);
  }

  private clearCache(id: string): void {
    for (const k of [...this.cache.keys()]) if (k.startsWith(`${id} `)) this.cache.delete(k);
  }

  async views(): Promise<HomeView[]> {
    const out: HomeView[] = [];
    for (const h of this.homes()) {
      out.push({ id: h.id, issuer: h.issuer, email: h.email, dev: h.issuer.startsWith("http://"), signedIn: (await this.secrets.get(secretName(h.id))) !== undefined, mustChange: h.mustChange });
    }
    return out;
  }

  /**
   * The whole browser sign-in at `home`. `open` opens a URL in the system browser and says whether it could.
   * Rejects with an AcctError; `signal` cancels. Returns the home's view.
   */
  async signIn(home: string, open: (url: string) => Promise<boolean>, signal?: AbortSignal): Promise<HomeView> {
    const issuer = checkHome(home);
    if (issuer === null) throw new AcctError("bad_home");
    const id = homeId(issuer);
    const known = this.homes();
    if (!known.some((h) => h.id === id) && known.length >= MAX_HOMES) throw new AcctError("too_many");
    const lb = await Loopback.bind();
    const p = newPkce();
    const url = authorizeUrl(issuer, lb.redirect, p);
    // The handler is attached before the browser opens, so a fast browser is never left unanswered.
    const waiting = lb.wait(p.state, issuer, SIGN_IN_TIMEOUT_MS, signal);
    waiting.catch(() => undefined);
    // SECURITY: only the checked home's own sign-in page is ever opened (CWE-601).
    if (!url.startsWith(`${issuer}/authorize?`) || !(await open(url).catch(() => false))) {
      lb.close(new AcctError("browser"));
      await waiting.catch(() => undefined);
      throw new AcctError("browser");
    }
    const code = await waiting;
    return this.locked(async () => {
      const s = await exchange(issuer, code, lb.redirect, p.verifier);
      // SECURITY: the refresh token goes to SecretStorage only (CWE-522).
      await this.secrets.store(secretName(id), s.refresh);
      let email = "";
      if (s.account) email = await me(issuer, s.account).catch(() => "");
      await this.upsert({ id, issuer, email, mustChange: s.mustChange });
      this.clearCache(id);
      const v = (await this.views()).find((h) => h.id === id);
      if (!v) throw new AcctError("bad_answer");
      return v;
    });
  }

  /** One refresh inside the queue; the rotated token is stored first; a dead session drops the stored token. */
  private async refreshLocked(rec: HomeRecord, resource: string, scope: string): Promise<Refreshed> {
    const rt = await this.secrets.get(secretName(rec.id));
    if (rt === undefined) throw new AcctError("not_signed_in");
    try {
      const r = await refresh(rec.issuer, rt, resource, scope);
      // SECURITY: the old token is now retired at the home; keep only the new one (RFC 9700 4.14.2).
      await this.secrets.store(secretName(rec.id), r.refresh);
      return r;
    } catch (e) {
      if (e instanceof AcctError && e.kind === "signed_out") {
        await this.secrets.delete(secretName(rec.id));
        this.clearCache(rec.id);
      } else if (e instanceof AcctError && e.kind === "password_change") {
        await this.upsert({ ...rec, mustChange: true }).catch(() => undefined);
      }
      throw e;
    }
  }

  /** A gateway token for `link`, cached in memory until a minute before it runs out. */
  async tokenFor(link: AccountLink): Promise<string> {
    if (!linkValid(link)) throw new AcctError("bad_home");
    const id = homeId(link.home);
    const key = `${id} ${link.audience}`;
    const hit = this.cache.get(key);
    if (hit && Date.now() < hit.until) return hit.token;
    return this.locked(async () => {
      // Another call may have refreshed while this one waited in the queue.
      const again = this.cache.get(key);
      if (again && Date.now() < again.until) return again.token;
      const rec = this.homes().find((h) => h.id === id);
      if (!rec) throw new AcctError("not_signed_in");
      const r = await this.refreshLocked(rec, link.audience, GATEWAY_SCOPE);
      if (!r.scope.split(" ").includes("chat")) throw new AcctError("no_scope");
      this.cache.set(key, { token: r.access, until: Date.now() + Math.max(5, r.expiresIn - 60) * 1000 });
      return r.access;
    });
  }

  /** Changes the password at the home (needed first for an operator-made one-time password). */
  async changePassword(id: string, current: string, next: string): Promise<void> {
    // SECURITY: checked before anything is sent (NIST SP 800-63B minimum length here is the home's 15).
    if (current.length === 0 || [...next].length < 15 || next.length > 256 || current.length > 256) throw new AcctError("bad_password");
    return this.locked(async () => {
      const rec = this.homes().find((h) => h.id === id);
      if (!rec) throw new AcctError("not_signed_in");
      const r = await this.refreshLocked(rec, rec.issuer, "account");
      const res = await call(rec.issuer, "POST", "/v1/me/password", { Authorization: `Bearer ${r.access}`, "Content-Type": "application/json" }, JSON.stringify({ current, new: next }));
      if (res.status === 400 || res.status === 403 || res.status === 422) throw new AcctError("bad_password");
      if (res.status !== 200 && res.status !== 204) throw tokenError(res.status, res.json);
      const email = await me(rec.issuer, r.access).catch(() => rec.email);
      await this.upsert({ ...rec, email, mustChange: false });
      this.clearCache(id);
    });
  }

  /** Signs out of a home: revokes the session there (best effort), forgets the refresh token and cached tokens. */
  async signOut(id: string): Promise<void> {
    return this.locked(async () => {
      const rec = this.homes().find((h) => h.id === id);
      if (!rec) throw new AcctError("not_signed_in");
      const rt = await this.secrets.get(secretName(id));
      if (rt !== undefined) await form(rec.issuer, "/revoke", { token: rt, client_id: CLIENT_ID }).catch(() => undefined);
      await this.secrets.delete(secretName(id));
      this.clearCache(id);
      await this.state.update(HOMES_KEY, this.homes().filter((h) => h.id !== id));
    });
  }
}
