// nexus.test.ts -- unit tests for local NEXUS Engine discovery, probing, and keyless loopback GatewayClient.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as fs from "node:fs";
import * as http from "node:http";
import * as os from "node:os";
import * as path from "node:path";
import type { AddressInfo } from "node:net";
import { GatewayClient } from "../src/gateway/client.ts";
import { isNexusLoopbackUrl, NetError } from "../src/gateway/checks.ts";
import { discoverNexusEndpoints, nexusUrlForPort, parseNexusSettingsUrl, probeNexusEndpoint } from "../src/nexus.ts";

async function fakeServer(handler: http.RequestListener): Promise<{ url: string; close: () => Promise<void> }> {
  const srv = http.createServer(handler);
  await new Promise<void>((r) => srv.listen(0, "127.0.0.1", r));
  const port = (srv.address() as AddressInfo).port;
  return { url: `http://127.0.0.1:${port}`, close: () => new Promise((r) => { srv.closeAllConnections(); srv.close(() => r()); }) };
}

test("nexusUrlForPort and isNexusLoopbackUrl allow canonical loopback unprivileged ports only", () => {
  assert.equal(nexusUrlForPort(11435), "http://127.0.0.1:11435");
  assert.equal(nexusUrlForPort(80), null);
  assert.equal(nexusUrlForPort(1023), null);
  assert.ok(isNexusLoopbackUrl("http://127.0.0.1:11435"));
  assert.ok(!isNexusLoopbackUrl("https://127.0.0.1:11435"));
  assert.ok(!isNexusLoopbackUrl("http://127.0.0.1:011435"));
  assert.ok(!isNexusLoopbackUrl("http://127.0.0.1:11435/v1"));
  assert.ok(!isNexusLoopbackUrl("http://10.0.0.5:11435"));
  assert.equal(parseNexusSettingsUrl("HTTP://Localhost:12435/"), "http://127.0.0.1:12435");
  assert.equal(parseNexusSettingsUrl("http://example.org:11435"), null);
});

test("discoverNexusEndpoints reads studio-settings.json and refuses symlinks", () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "p2p-ext-nexus-"));
  const cfgDir = path.join(tmp, ".nexus-studio");
  fs.mkdirSync(cfgDir, { recursive: true });
  const cfgPath = path.join(cfgDir, "studio-settings.json");
  fs.writeFileSync(cfgPath, JSON.stringify({ proxyUrl: "http://localhost:12435", runtimeUrl: "https://remote.example:443" }));
  const ep = discoverNexusEndpoints(tmp);
  assert.equal(ep.proxyUrl, "http://127.0.0.1:12435");
  assert.equal(ep.runtimeUrl, "http://127.0.0.1:11436");

  const other = path.join(tmp, "other.json");
  fs.writeFileSync(other, JSON.stringify({ proxyUrl: "http://127.0.0.1:15000" }));
  fs.unlinkSync(cfgPath);
  fs.symlinkSync(other, cfgPath);
  const linked = discoverNexusEndpoints(tmp);
  assert.equal(linked.proxyUrl, "http://127.0.0.1:11435");
  fs.rmSync(tmp, { recursive: true, force: true });
});

test("probeNexusEndpoint verifies service identity and cleans telemetry", async () => {
  const srv = await fakeServer((_req, res) => {
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ status: "ok", service: "nexus-runtime", models_loaded: 1, models_available: 2, simd: "avx2\n" }));
  });
  try {
    const rt = await probeNexusEndpoint(srv.url, "nexus-runtime");
    assert.equal(rt.running, true);
    assert.equal(rt.modelsLoaded, 1);
    assert.equal(rt.simd, "avx2");
    const wrong = await probeNexusEndpoint(srv.url, "nexus-proxy");
    assert.equal(wrong.running, false);
  } finally {
    await srv.close();
  }
});

test("GatewayClient keyless mode works solely for nexus loopback URLs and sends no Authorization", async () => {
  assert.throws(() => new GatewayClient({ url: "https://gw.example.org", nexus: true }), (e: unknown) => e instanceof NetError && e.kind === "bad_request");
  assert.throws(() => new GatewayClient({ url: "http://127.0.0.1:11435" }), (e: unknown) => e instanceof NetError && e.kind === "no_key");

  let authSeen: string | undefined;
  const srv = await fakeServer((req, res) => {
    authSeen = req.headers.authorization;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ data: [{ id: "nexus-native-3b" }] }));
  });
  try {
    const client = new GatewayClient({ url: srv.url, nexus: true });
    const { ids } = await client.models();
    assert.deepEqual(ids, ["nexus-native-3b"]);
    assert.equal(authSeen, undefined);
  } finally {
    await srv.close();
  }
});
