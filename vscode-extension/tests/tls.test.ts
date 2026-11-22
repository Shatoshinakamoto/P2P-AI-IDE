// tls.test.ts -- the pinned CA (CWE-295): with the gateway's own CA saved the request works; without it, or with
// another CA, it fails with the fixed `tls` error. Certificates are made at test time with openssl (skipped when
// openssl isn't installed); nothing leaves 127.0.0.1.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as https from "node:https";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { AddressInfo } from "node:net";
import { GatewayClient } from "../src/gateway/client.ts";
import { NetError } from "../src/gateway/checks.ts";

function hasOpenssl(): boolean {
  try {
    execFileSync("openssl", ["version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

/** Makes a CA and a server certificate for 127.0.0.1 signed by it, in a temp folder. */
function makeCerts(dir: string, name: string): { ca: string; key: string; cert: string } {
  const p = (f: string): string => join(dir, `${name}-${f}`);
  const run = (args: string[]): void => void execFileSync("openssl", args, { stdio: "ignore" });
  run(["req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-keyout", p("ca.key"), "-out", p("ca.pem"), "-days", "1", "-subj", `/CN=${name} test CA`]);
  run(["req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-keyout", p("srv.key"), "-out", p("srv.csr"), "-subj", "/CN=127.0.0.1"]);
  writeFileSync(p("ext.cnf"), "subjectAltName=IP:127.0.0.1\nbasicConstraints=CA:FALSE\n");
  run(["x509", "-req", "-in", p("srv.csr"), "-CA", p("ca.pem"), "-CAkey", p("ca.key"), "-CAcreateserial", "-out", p("srv.pem"), "-days", "1", "-extfile", p("ext.cnf")]);
  return { ca: readFileSync(p("ca.pem"), "utf8"), key: readFileSync(p("srv.key"), "utf8"), cert: readFileSync(p("srv.pem"), "utf8") };
}

test("a pinned CA is trusted; no pin or another CA is refused", { skip: !hasOpenssl() }, async () => {
  const dir = mkdtempSync(join(tmpdir(), "p2pai-tls-"));
  try {
    const good = makeCerts(dir, "good");
    const other = makeCerts(dir, "other");
    const srv = https.createServer({ key: good.key, cert: good.cert }, (_req, res) => res.end(JSON.stringify({ data: [{ id: "m" }] })));
    await new Promise<void>((r) => srv.listen(0, "127.0.0.1", r));
    const url = `https://127.0.0.1:${(srv.address() as AddressInfo).port}`;
    try {
      const key = "test-key-0123456789";
      assert.deepEqual((await new GatewayClient({ url, key, caPem: good.ca }).models()).ids, ["m"]);
      for (const caPem of [undefined, other.ca]) {
        await assert.rejects(new GatewayClient({ url, key, caPem }).models(), (e: unknown) => e instanceof NetError && e.kind === "tls");
      }
    } finally {
      srv.closeAllConnections();
      await new Promise<void>((r) => srv.close(() => r()));
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
