/**
 * gateways.ts -- the extension's saved gateways: names and addresses in the extension's own state file
 * (./fileState.ts; VS Code's global state until X3), keys in VS Code's SecretStorage (the OS keychain where VS Code
 * has one).
 *
 * Purpose: the gateway picker (side track X1). Same rules as the IDE's core/src/gateways.rs: checked name, URL and
 * key; at most 32 gateways; one active.
 * Inputs / trust boundary: values typed by the person in VS Code input boxes; the stored list (re-checked on every
 * load, because another program or a hand edit could change the state file).
 * Security controls:
 *  - Keys are kept only in SecretStorage, never in settings, the state file, global state, logs or the webview (CWE-312, CWE-522).
 *  - A pinned CA is read once from a file the person picks, checked to look like PEM certificates and capped at
 *    64 KiB; only its text is stored (CWE-295).
 *  - Every stored entry is re-validated on load; bad entries are dropped (CWE-20).
 *  - A gateway can instead be linked to a P2P-AI account (phase A3, ./accounts.ts): the link (account service address
 *    and the gateway's account id) holds no secret and is re-checked on load; the token is fetched per call.
 * Dependencies: the `vscode` API (provided by VS Code at run time), ./gateway/checks.ts, ./accounts.ts.
 */
import * as vscode from "vscode";
import { checkKey, checkName, checkUrl, isNexusLoopbackUrl } from "./gateway/checks.ts";
import type { GatewayTarget } from "./gateway/client.ts";
import type { FileState } from "./fileState.ts";
import { Accounts, AcctError, linkValid, makeLink, type AccountLink } from "./accounts.ts";

/** A saved gateway. The key is not part of it. */
export interface SavedGateway {
  id: string;
  name: string;
  url: string;
  /** Local engine kind when saved from NEXUS Engine discovery (no key or account). */
  kind?: "nexus";
  /** PEM text of a pinned CA, if any. */
  caPem?: string;
  /** Phase A3: sign in with this account instead of the key (no secret in it). */
  account?: AccountLink;
}

const LIST_KEY = "p2pAi.gateways";
const ACTIVE_KEY = "p2pAi.activeGateway";
const MAX_GATEWAYS = 32;
const MAX_CA_BYTES = 64 << 10;
const secretName = (id: string): string => `p2pAi.gatewayKey.${id}`;

/** True for a PEM text with at least one certificate and nothing but PEM blocks (CWE-20). */
export function caLooksValid(pem: string): boolean {
  if (pem.length === 0 || pem.length > MAX_CA_BYTES) return false;
  const blocks = pem.match(/-----BEGIN CERTIFICATE-----[A-Za-z0-9+/=\r\n]+-----END CERTIFICATE-----/g);
  return !!blocks && blocks.length > 0;
}

export class GatewayStore {
  private readonly changed = new vscode.EventEmitter<void>();
  /** Fires when the list or the active gateway changes. */
  readonly onDidChange = this.changed.event;

  private readonly ctx: vscode.ExtensionContext;
  /** Non-secret settings (the list, the active gateway); shared with the views. */
  readonly state: FileState;
  /** P2P-AI account sign-ins (refresh tokens in SecretStorage, homes in the state file). */
  readonly accounts: Accounts;
  constructor(ctx: vscode.ExtensionContext, state: FileState) {
    this.ctx = ctx;
    this.state = state;
    this.accounts = new Accounts(ctx.secrets, state);
  }

  /** The saved gateways, re-checked. */
  list(): SavedGateway[] {
    const raw = this.state.get<unknown>(LIST_KEY);
    if (!Array.isArray(raw)) return [];
    const out: SavedGateway[] = [];
    for (const g of raw.slice(0, MAX_GATEWAYS)) {
      if (typeof g !== "object" || g === null) continue;
      const { id, name, url, kind, caPem, account } = g as Record<string, unknown>;
      // SECURITY: stored values are untrusted too; anything that doesn't pass the same checks is dropped.
      if (typeof id !== "string" || !/^[a-z0-9]{8,32}$/.test(id)) continue;
      if (typeof name !== "string" || checkName(name) !== name) continue;
      if (typeof url !== "string" || checkUrl(url) !== url) continue;
      if (kind !== undefined && kind !== "nexus") continue;
      if (kind === "nexus" && !isNexusLoopbackUrl(url)) continue;
      if (caPem !== undefined && (typeof caPem !== "string" || !caLooksValid(caPem))) continue;
      // SECURITY: a damaged account link drops only the link, so the gateway falls back to its key (CWE-20).
      // Local NEXUS Engine entries never link to a remote account.
      const acct = kind !== "nexus" && linkValid(account) ? { home: account.home, audience: account.audience } : undefined;
      out.push({
        id,
        name,
        url,
        ...(kind === "nexus" ? { kind: "nexus" as const } : {}),
        caPem: caPem as string | undefined,
        ...(acct ? { account: acct } : {}),
      });
    }
    return out;
  }

  /** The active gateway, or the only one, or undefined. */
  active(): SavedGateway | undefined {
    const all = this.list();
    const id = this.state.get<string>(ACTIVE_KEY);
    return all.find((g) => g.id === id) ?? (all.length === 1 ? all[0] : undefined);
  }

  async setActive(id: string): Promise<void> {
    await this.state.update(ACTIVE_KEY, id);
    this.changed.fire();
  }

  /**
   * Adds a gateway. Returns an error message for the person, or undefined on success.
   * The key goes straight to SecretStorage.
   */
  async add(name: string, url: string, key: string, caPem?: string): Promise<string | undefined> {
    const n = checkName(name);
    if (n === null) return "the name must be 1 to 64 characters on one line";
    const u = checkUrl(url);
    if (u === null) return "the address must be https://host[:port] (plain http only for this computer)";
    const k = checkKey(key);
    if (k === null) return "the key must be 8 to 512 visible characters without spaces";
    if (caPem !== undefined && !caLooksValid(caPem)) return "the CA file must hold PEM certificates (at most 64 KiB)";
    const all = this.list();
    if (all.length >= MAX_GATEWAYS) return "at most 32 gateways can be saved";
    if (all.some((g) => g.url === u)) return "that gateway is already saved";
    // SECURITY: ids come from the CSPRNG (no guessable names in SecretStorage).
    const id = Buffer.from(crypto.getRandomValues(new Uint8Array(8))).toString("hex");
    await this.ctx.secrets.store(secretName(id), k);
    await this.state.update(LIST_KEY, [...all, { id, name: n, url: u, caPem }]);
    await this.setActive(id);
    return undefined;
  }

  /** Replaces a gateway's key. Returns an error message or undefined. */
  async setKey(id: string, key: string): Promise<string | undefined> {
    const k = checkKey(key);
    if (k === null) return "the key must be 8 to 512 visible characters without spaces";
    await this.ctx.secrets.store(secretName(id), k);
    this.changed.fire();
    return undefined;
  }

  /** The key, read only when a request is made. */
  key(id: string): Thenable<string | undefined> {
    return this.ctx.secrets.get(secretName(id));
  }

  /**
   * What goes in `Authorization` for `gw`: a fresh account token when the gateway is linked to an account, else its
   * key. Rejects with an AcctError (a fixed message) when the account cannot give a token.
   */
  async credential(gw: SavedGateway): Promise<string | undefined> {
    if (gw.kind === "nexus") return undefined;
    return gw.account ? this.accounts.tokenFor(gw.account) : this.key(gw.id);
  }

  /** Everything GatewayClient needs for `gw`; an account-linked gateway also gets `renew` for later requests. */
  async target(gw: SavedGateway): Promise<GatewayTarget> {
    if (gw.kind === "nexus") {
      return { url: gw.url, nexus: true };
    }
    const link = gw.account;
    return { url: gw.url, key: await this.credential(gw), caPem: gw.caPem, ...(link ? { renew: () => this.accounts.tokenFor(link) } : {}) };
  }

  /**
   * Links a gateway to an account (`home` + the gateway's account id), or with an empty `home`, back to its key.
   * Returns an error message or undefined. The key stays saved, so unlinking needs no new key.
   */
  async setAccount(id: string, home: string, audience: string): Promise<string | undefined> {
    let link: AccountLink | undefined;
    if (home.trim() !== "") {
      const l = makeLink(home, audience);
      if (l instanceof AcctError) return l.message;
      link = l;
    }
    const all = this.list();
    const target = all.find((g) => g.id === id);
    if (!target) return "that gateway is no longer saved";
    if (target.kind === "nexus" && link) return "NEXUS Engine runs on this computer without an account";
    await this.state.update(LIST_KEY, all.map((g) => {
      if (g.id !== id) return g;
      const { account: _old, ...rest } = g;
      return link ? { ...rest, account: link } : rest;
    }));
    this.changed.fire();
    return undefined;
  }

  /**
   * Saves (or updates) a local NEXUS Proxy or NEXUS Runtime entry and makes it active.
   * No API key is stored; local NEXUS Engine instances authenticate by loopback binding (`127.0.0.1`) only.
   */
  async saveNexus(url: string, name: string): Promise<string | undefined> {
    if (!isNexusLoopbackUrl(url)) return "NEXUS Engine addresses must be http://127.0.0.1:<port>";
    const n = checkName(name);
    if (n === null) return "give the gateway a name (1 to 64 characters on one line)";
    const prefix = n.startsWith("NEXUS Runtime") ? "NEXUS Runtime" : n.startsWith("NEXUS Proxy") ? "NEXUS Proxy" : "";
    const all = this.list();
    if (all.some((g) => g.url === url && g.kind !== "nexus")) return "that gateway is already saved";
    const match = all.find((g) => g.kind === "nexus" && (g.url === url || (prefix !== "" && g.name.startsWith(prefix))));
    if (match) {
      await this.state.update(LIST_KEY, all.map((g) => (g.id === match.id ? { id: match.id, name: n, url, kind: "nexus" as const } : g)));
      await this.setActive(match.id);
      return undefined;
    }
    if (all.length >= MAX_GATEWAYS) return "at most 32 gateways can be saved";
    const id = Buffer.from(crypto.getRandomValues(new Uint8Array(8))).toString("hex");
    await this.state.update(LIST_KEY, [...all, { id, name: n, url, kind: "nexus" as const }]);
    await this.setActive(id);
    return undefined;
  }

  /** Removes a gateway and deletes its key. */
  async remove(id: string): Promise<void> {
    await this.state.update(LIST_KEY, this.list().filter((g) => g.id !== id));
    await this.ctx.secrets.delete(secretName(id));
    if (this.state.get<string>(ACTIVE_KEY) === id) await this.state.update(ACTIVE_KEY, undefined);
    this.changed.fire();
  }
}
