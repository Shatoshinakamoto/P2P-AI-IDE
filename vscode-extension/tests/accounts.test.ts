// accounts.test.ts -- the extension's P2P-AI account sign-in (phase A3) against a fake account service on 127.0.0.1.
// Same cases as the IDE core's accounts.rs tests: address and id rules, the sign-in link, callback checks (state,
// iss, repeated parameters), the whole browser sign-in, token cache and rotation, a home that answers for another
// audience, a dead session, the one-time password, sign-out (CWE-20, CWE-235, CWE-352, CWE-522, RFC 9207, RFC 9700).
import { test } from "node:test";
import assert from "node:assert/strict";
import * as http from "node:http";
import type { AddressInfo } from "node:net";
import { createHash } from "node:crypto";
import {
  Accounts, AcctError, authorizeUrl, checkAudience, checkHome, homeId, linkValid, makeLink, newPkce, parseCallback,
  tokenError, type Secrets, type StateBox,
} from "../src/accounts.ts";

/** In-memory SecretStorage and state file. */
function boxes(): { secrets: Secrets & { map: Map<string, string> }; state: StateBox & { map: Map<string, unknown> } } {
  const sm = new Map<string, string>();
  const st = new Map<string, unknown>();
  return {
    secrets: { map: sm, get: async (k) => sm.get(k), store: async (k, v) => void sm.set(k, v), delete: async (k) => void sm.delete(k) },
    state: { map: st, get: <T>(k: string) => st.get(k) as T | undefined, update: async (k, v) => void st.set(k, JSON.parse(JSON.stringify(v))) },
  };
}

interface Fake {
  url: string;
  refreshes: number;
  revoked: string[];
  mustChange: boolean;
  wrongAudience: boolean;
  current: string;
  close: () => Promise<void>;
}

/** A fake account service: /token (code and rotating refresh), /v1/me, /v1/me/password, /revoke. */
async function fakeHome(): Promise<Fake> {
  const f: Fake = { url: "", refreshes: 0, revoked: [], mustChange: false, wrongAudience: false, current: "", close: async () => {} };
  let codeChallenge = "";
  const srv = http.createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => {
      const body = Buffer.concat(chunks).toString();
      const form = new URLSearchParams(body);
      const send = (status: number, v: unknown) => { res.writeHead(status, { "Content-Type": "application/json" }); res.end(JSON.stringify(v)); };
      const u = new URL(req.url ?? "/", f.url);
      if (u.pathname === "/authorize") {
        codeChallenge = u.searchParams.get("code_challenge") ?? "";
        res.writeHead(302, { Location: `${u.searchParams.get("redirect_uri")}?code=code-0123456789abcdef&state=${encodeURIComponent(u.searchParams.get("state") ?? "")}&iss=${encodeURIComponent(f.url)}` });
        return res.end();
      }
      if (u.pathname === "/token" && form.get("grant_type") === "authorization_code") {
        const ok = form.get("client_id") === "p2p-ai-ide" && createHash("sha256").update(form.get("code_verifier") ?? "").digest("base64url") === codeChallenge;
        if (!ok) return send(400, { error: "invalid_grant" });
        f.current = "p2prt_000000000000";
        return send(200, { refresh_token: f.current, access_token: "acct-token-0123456789", password_change_required: f.mustChange });
      }
      if (u.pathname === "/token" && form.get("grant_type") === "refresh_token") {
        if (form.get("refresh_token") !== f.current) { f.current = ""; return send(400, { error: "invalid_grant" }); }
        const resource = form.get("resource") ?? "";
        if (f.mustChange && resource !== f.url) return send(403, { error: "password_change_required" });
        f.refreshes++;
        f.current = `p2prt_${String(f.refreshes).padStart(12, "0")}`;
        return send(200, { refresh_token: f.current, access_token: `tok-${resource}-${String(f.refreshes).padStart(8, "0")}`, expires_in: 300, audience: f.wrongAudience ? "someone-else" : resource, scope: form.get("scope") });
      }
      if (u.pathname === "/v1/me" && req.method === "GET") return send(200, { email: "me@example.org\u0007" });
      if (u.pathname === "/v1/me/password") {
        const j = JSON.parse(body);
        if (j.current !== "one-time-pass") return send(403, { error: "bad_password" });
        f.mustChange = false;
        return send(204, {});
      }
      if (u.pathname === "/revoke") { f.revoked.push(form.get("token") ?? ""); f.current = ""; return send(200, {}); }
      send(404, {});
    });
  });
  await new Promise<void>((r) => srv.listen(0, "127.0.0.1", r));
  f.url = `http://127.0.0.1:${(srv.address() as AddressInfo).port}`;
  f.close = () => new Promise((r) => { srv.closeAllConnections(); srv.close(() => r()); });
  return f;
}

/** Plays the browser: follows the home's redirect back to the extension's loopback listener. */
async function browser(url: string): Promise<boolean> {
  const r1 = await fetch(url, { redirect: "manual" });
  const loc = r1.headers.get("location");
  if (!loc) return false;
  const r2 = await fetch(loc);
  return r2.status === 200 && (await r2.text()).includes("Signed in");
}

test("address and gateway id rules match the IDE core", () => {
  assert.equal(checkHome(" https://Accounts.Example.org/ "), "https://accounts.example.org");
  assert.equal(checkHome("https://example.org/acct/v1"), "https://example.org/acct/v1");
  assert.equal(checkHome("http://127.0.0.1:9000"), "http://127.0.0.1:9000");
  for (const bad of ["http://example.org", "https://u@example.org", "https://example.org/a/../b", "https://example.org/a?x=1", "https://example.org/a#f", "ftp://example.org", "https://example.org/a b", "https://example.org//a"]) {
    assert.equal(checkHome(bad), null, bad);
  }
  assert.equal(checkAudience("gw-example-01"), "gw-example-01");
  for (const bad of ["ab", "-gw", "GW", "gw/1", "a".repeat(129)]) assert.equal(checkAudience(bad), null, bad);
  assert.match(homeId("https://accounts.example.org"), /^[0-9a-f]{16}$/);
  assert.ok(makeLink("https://x.example.org", "bad id") instanceof AcctError);
  assert.ok(linkValid(makeLink("https://x.example.org", "gw-1")));
  assert.equal(linkValid({ home: "https://x.example.org/", audience: "gw-1" }), false);
});

test("sign-in link: our client, S256, loopback return, URL-safe", () => {
  const p = newPkce();
  assert.equal(p.verifier.length, 43);
  const u = new URL(authorizeUrl("https://a.example.org", "http://127.0.0.1:5000/cb", p));
  assert.equal(u.pathname, "/authorize");
  assert.equal(u.searchParams.get("client_id"), "p2p-ai-ide");
  assert.equal(u.searchParams.get("code_challenge_method"), "S256");
  assert.equal(u.searchParams.get("redirect_uri"), "http://127.0.0.1:5000/cb");
  assert.equal(u.searchParams.get("code_challenge"), createHash("sha256").update(p.verifier).digest("base64url"));
  assert.match(authorizeUrl("https://a.example.org", "http://127.0.0.1:5000/cb", p), /^[A-Za-z0-9\-._~:/?#[\]@!$&'()*+,;=%]+$/);
});

test("callback: state, iss, path, method and repeated parameters", () => {
  const iss = "http://127.0.0.1:9";
  const q = (s: string) => `/cb?code=code-0123456789abcdef&state=${s}&iss=${encodeURIComponent(iss)}`;
  assert.deepEqual(parseCallback("GET", q("st"), "st", iss), { kind: "code", code: "code-0123456789abcdef" });
  assert.equal(parseCallback("GET", q("other"), "st", iss).kind, "ignore");
  assert.equal(parseCallback("POST", q("st"), "st", iss).kind, "ignore");
  assert.equal(parseCallback("GET", q("st").replace("/cb", "/x"), "st", iss).kind, "ignore");
  assert.equal(parseCallback("GET", `${q("st")}&state=st`, "st", iss).kind, "ignore");
  assert.equal(parseCallback("GET", q("st").replace(encodeURIComponent(iss), "https%3A%2F%2Fevil"), "st", iss).kind, "ignore");
  assert.equal(parseCallback("GET", "/cb?error=access_denied&state=st", "st", iss).kind, "error");
  assert.equal(parseCallback("GET", "/cb?error=access_denied&state=no", "st", iss).kind, "ignore");
  assert.equal(parseCallback("GET", "/cb?code=%ZZ&state=st", "st", iss).kind, "ignore");
});

test("token errors map to fixed messages", () => {
  assert.equal(tokenError(400, { error: "invalid_grant" }).kind, "signed_out");
  assert.equal(tokenError(403, { error: "password_change_required" }).kind, "password_change");
  assert.equal(tokenError(400, { error: "invalid_target" }).kind, "not_for_gateway");
  assert.equal(tokenError(429, {}).kind, "busy");
  assert.equal(tokenError(500, { error: "<script>" }).message, "the account service answered with an error (HTTP 500)");
});

test("sign in, token cache and rotation, sign out", async () => {
  const f = await fakeHome();
  const { secrets, state } = boxes();
  try {
    const acc = new Accounts(secrets, state);
    const v = await acc.signIn(f.url + "/", browser);
    assert.equal(v.signedIn, true);
    assert.equal(v.email, "me@example.org", "control characters are cut");
    assert.equal(v.dev, true);
    // SECURITY: the refresh token is only in SecretStorage, never in the state file.
    assert.ok(!JSON.stringify([...state.map.values()]).includes("p2prt_"));
    assert.equal(secrets.map.get(`p2pAi.accountRefresh.${homeId(f.url)}`), "p2prt_000000000000");
    const link = { home: f.url, audience: "gw-one" };
    const [a, b] = await Promise.all([acc.tokenFor(link), acc.tokenFor(link)]);
    assert.equal(a, b);
    assert.equal(f.refreshes, 1, "two calls at once make one refresh");
    assert.equal(secrets.map.get(`p2pAi.accountRefresh.${homeId(f.url)}`), f.current, "rotated token stored");
    assert.equal(await acc.tokenFor(link), a, "cached");
    const other = await acc.tokenFor({ home: f.url, audience: "gw-two" });
    assert.notEqual(other, a);
    await acc.signOut(v.id);
    assert.equal(f.revoked.length, 1);
    assert.deepEqual(await acc.views(), []);
    assert.equal(secrets.map.size, 0);
    await assert.rejects(acc.tokenFor(link), (e: AcctError) => e.kind === "not_signed_in");
  } finally {
    await f.close();
  }
});

test("one-time password: no gateway token until changed; short one refused before sending", async () => {
  const f = await fakeHome();
  f.mustChange = true;
  const { secrets, state } = boxes();
  try {
    const acc = new Accounts(secrets, state);
    const v = await acc.signIn(f.url, browser);
    assert.equal(v.mustChange, true);
    await assert.rejects(acc.tokenFor({ home: f.url, audience: "gw-one" }), (e: AcctError) => e.kind === "password_change");
    await assert.rejects(acc.changePassword(v.id, "one-time-pass", "short"), (e: AcctError) => e.kind === "bad_password");
    await assert.rejects(acc.changePassword(v.id, "wrong", "a long enough new passphrase"), (e: AcctError) => e.kind === "bad_password");
    await acc.changePassword(v.id, "one-time-pass", "a long enough new passphrase");
    assert.equal((await acc.views())[0].mustChange, false);
    assert.match(await acc.tokenFor({ home: f.url, audience: "gw-one" }), /^tok-gw-one-/);
  } finally {
    await f.close();
  }
});

test("a token for another audience is refused; a dead session forgets the token", async () => {
  const f = await fakeHome();
  const { secrets, state } = boxes();
  try {
    const acc = new Accounts(secrets, state);
    await acc.signIn(f.url, browser);
    f.wrongAudience = true;
    await assert.rejects(acc.tokenFor({ home: f.url, audience: "gw-one" }), (e: AcctError) => e.kind === "bad_answer");
    f.wrongAudience = false;
    f.current = "revoked-elsewhere-000";
    await assert.rejects(acc.tokenFor({ home: f.url, audience: "gw-one" }), (e: AcctError) => e.kind === "signed_out");
    assert.equal((await acc.views())[0].signedIn, false);
  } finally {
    await f.close();
  }
});

test("sign-in: cancel stops the wait; a browser that cannot open fails; wrong requests do not finish it", async () => {
  const f = await fakeHome();
  const { secrets, state } = boxes();
  try {
    const acc = new Accounts(secrets, state);
    const stop = new AbortController();
    let redirect = "";
    const p = acc.signIn(f.url, async (url) => {
      redirect = new URL(url).searchParams.get("redirect_uri") ?? "";
      return true;
    }, stop.signal);
    await new Promise((r) => setTimeout(r, 50));
    const wrong = await fetch(`${redirect}?code=code-0123456789abcdef&state=guess&iss=${encodeURIComponent(f.url)}`);
    assert.equal(wrong.status, 404);
    assert.equal(wrong.headers.get("content-security-policy"), "default-src 'none'; style-src 'unsafe-inline'");
    stop.abort();
    await assert.rejects(p, (e: AcctError) => e.kind === "cancelled");
    await assert.rejects(acc.signIn(f.url, async () => false), (e: AcctError) => e.kind === "browser");
    await assert.rejects(acc.signIn("http://example.org", browser), (e: AcctError) => e.kind === "bad_home");
  } finally {
    await f.close();
  }
});

test("an edited state file cannot point a secret at another home", async () => {
  const { secrets, state } = boxes();
  await state.update("p2pAi.accounts", [{ id: "0000000000000000", issuer: "https://evil.example.org", email: "x" }, { id: homeId("https://a.example.org"), issuer: "https://a.example.org" }]);
  const acc = new Accounts(secrets, state);
  assert.deepEqual((await acc.views()).map((h) => h.issuer), ["https://a.example.org"]);
});
