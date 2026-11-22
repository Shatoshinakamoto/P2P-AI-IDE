// client.test.ts -- the extension's gateway client against a fake gateway on 127.0.0.1 (no real network).
// Checks: the key goes only in Authorization; answers stream as text; Stop aborts; redirects are not followed;
// size caps hold; error text never contains the key or the address (CWE-522, CWE-601, CWE-400, CWE-209).
import { test } from "node:test";
import assert from "node:assert/strict";
import * as http from "node:http";
import type { AddressInfo } from "node:net";
import { GatewayClient } from "../src/gateway/client.ts";
import { MAX_STREAM_BYTES, NetError } from "../src/gateway/checks.ts";
import { parseFromView } from "../src/viewMessages.ts";

const KEY = "test-key-0123456789";

/** Starts a fake gateway; `handler` answers each request. */
async function fake(handler: http.RequestListener): Promise<{ url: string; close: () => Promise<void> }> {
  const srv = http.createServer(handler);
  await new Promise<void>((r) => srv.listen(0, "127.0.0.1", r));
  const port = (srv.address() as AddressInfo).port;
  return { url: `http://127.0.0.1:${port}`, close: () => new Promise((r) => { srv.closeAllConnections(); srv.close(() => r()); }) };
}

const chunk = (t: string): string => `data: ${JSON.stringify({ choices: [{ delta: { content: t } }] })}\n\n`;

test("models: key only in Authorization, sorted ids", async () => {
  let auth = "";
  let path = "";
  const gw = await fake((req, res) => {
    auth = req.headers.authorization ?? "";
    path = req.url ?? "";
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ data: [{ id: "b" }, { id: "a" }] }));
  });
  try {
    const { ids } = await new GatewayClient({ url: gw.url, key: KEY }).models();
    assert.deepEqual(ids, ["a", "b"]);
    assert.equal(auth, `Bearer ${KEY}`);
    assert.equal(path, "/v1/models");
  } finally {
    await gw.close();
  }
});

test("chat streams text and sends only checked fields", async () => {
  let body: unknown;
  const gw = await fake(async (req, res) => {
    let s = "";
    for await (const c of req) s += c;
    body = JSON.parse(s);
    res.setHeader("Content-Type", "text/event-stream");
    res.write(chunk("Hel"));
    res.write(chunk("lo <b>x</b>"));
    res.write(`data: ${JSON.stringify({ choices: [{ delta: {}, finish_reason: "stop" }] })}\n\n`);
    res.end("data: [DONE]\n\n");
  });
  try {
    const got: string[] = [];
    const msgs = [{ role: "user" as const, content: "hi", extra: "dropped" }];
    const end = await new GatewayClient({ url: gw.url, key: KEY }).chatStream("m1", msgs, (t) => got.push(t));
    assert.equal(got.join(""), "Hello <b>x</b>");
    assert.equal(end.finishReason, "stop");
    assert.deepEqual(body, { model: "m1", messages: [{ role: "user", content: "hi" }], stream: true });
  } finally {
    await gw.close();
  }
});

test("stop aborts a turn", async () => {
  const gw = await fake((_req, res) => {
    res.setHeader("Content-Type", "text/event-stream");
    res.write(chunk("first"));
    // never ends on its own
  });
  try {
    const ac = new AbortController();
    const p = new GatewayClient({ url: gw.url, key: KEY }).chatStream("m1", [{ role: "user", content: "hi" }], () => ac.abort(), ac.signal);
    await assert.rejects(p, (e: unknown) => e instanceof NetError && e.kind === "stopped");
  } finally {
    await gw.close();
  }
});

test("a redirect is not followed and the key goes nowhere else", async () => {
  let elsewhere = 0;
  const other = await fake((_req, res) => {
    elsewhere++;
    res.end("{}");
  });
  const gw = await fake((_req, res) => {
    res.statusCode = 302;
    res.setHeader("Location", `${other.url}/v1/models`);
    res.end();
  });
  try {
    await assert.rejects(new GatewayClient({ url: gw.url, key: KEY }).models(), (e: unknown) => e instanceof NetError && e.kind === "status");
    assert.equal(elsewhere, 0);
  } finally {
    await gw.close();
    await other.close();
  }
});

test("status codes map to fixed errors without the key or address", async () => {
  for (const [code, kind] of [[401, "auth"], [404, "not_offered"], [429, "rate_limited"], [500, "status"]] as const) {
    const gw = await fake((_req, res) => {
      res.statusCode = code;
      res.end(`secret page ${KEY}`);
    });
    try {
      await assert.rejects(new GatewayClient({ url: gw.url, key: KEY }).models(), (e: unknown) => {
        assert.ok(e instanceof NetError);
        assert.equal(e.kind, kind);
        assert.ok(!e.message.includes(KEY) && !e.message.includes("127.0.0.1"));
        return true;
      });
    } finally {
      await gw.close();
    }
  }
});

test("caps: a huge model list and an endless stream are cut off", async () => {
  const big = await fake((_req, res) => {
    res.end("x".repeat((1 << 20) + 10));
  });
  const endless = await fake((_req, res) => {
    res.setHeader("Content-Type", "text/event-stream");
    const line = chunk("y".repeat(60_000));
    let sent = 0;
    const pump = (): void => {
      while (sent <= MAX_STREAM_BYTES + 100_000) {
        sent += line.length;
        if (!res.write(line)) return void res.once("drain", pump);
      }
      res.end();
    };
    pump();
  });
  const noNewline = await fake((_req, res) => {
    res.setHeader("Content-Type", "text/event-stream");
    res.end("data: " + "z".repeat((1 << 20) + 10));
  });
  try {
    await assert.rejects(new GatewayClient({ url: big.url, key: KEY }).models(), (e: unknown) => e instanceof NetError && e.kind === "too_large");
    await assert.rejects(new GatewayClient({ url: endless.url, key: KEY }).chatStream("m", [{ role: "user", content: "x" }], () => {}), (e: unknown) => e instanceof NetError && e.kind === "too_large");
    await assert.rejects(new GatewayClient({ url: noNewline.url, key: KEY }).chatStream("m", [{ role: "user", content: "x" }], () => {}), (e: unknown) => e instanceof NetError && e.kind === "too_large");
  } finally {
    await big.close();
    await endless.close();
    await noNewline.close();
  }
});

test("bad inputs are refused before any request", async () => {
  assert.throws(() => new GatewayClient({ url: "http://gw.example.org", key: KEY }), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
  assert.throws(() => new GatewayClient({ url: "https://gw.example.org", key: undefined }), (e: unknown) => e instanceof NetError && e.kind === "no_key");
  const c = new GatewayClient({ url: "https://gw.example.org", key: KEY });
  await assert.rejects(c.chatStream("bad model", [{ role: "user", content: "x" }], () => {}), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
  await assert.rejects(c.chatStream("m", [{ role: "tool" as never, content: "x" }], () => {}), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
  await assert.rejects(c.chatStream("m", [], () => {}), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
});

test("an unreachable gateway is a fixed connect error", async () => {
  const gw = await fake((_req, res) => res.end());
  const url = gw.url;
  await gw.close();
  await assert.rejects(new GatewayClient({ url, key: KEY }).models(), (e: unknown) => e instanceof NetError && e.kind === "connect" && !e.message.includes("127.0.0.1"));
});

test("webview messages are allow-listed and capped", () => {
  assert.deepEqual(parseFromView({ type: "send", text: "hi" }), { type: "send", text: "hi" });
  assert.deepEqual(parseFromView({ type: "stop", extra: 1 }), { type: "stop" });
  for (const bad of [null, 1, "send", {}, { type: "eval" }, { type: "send" }, { type: "send", text: 5 }, { type: "send", text: "   " }, { type: "send", text: "x".repeat((256 << 10) + 1) }]) {
    assert.equal(parseFromView(bad), undefined, JSON.stringify(bad)?.slice(0, 40));
  }
});
