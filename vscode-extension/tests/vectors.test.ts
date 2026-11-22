// vectors.test.ts -- runs the shared gateway-client cases (core/tests/vectors/gateway_client.json) against the
// extension's TypeScript client. The IDE's Rust client runs the same file (core/src/gateway_vectors.rs), so the
// two accept and refuse the same input. Security: covers text-only answers (CWE-79), header-safe keys (CWE-113)
// and no plain http to other hosts (CWE-319).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { NetError, checkKey, checkModel, checkName, checkStatus, checkUrl, parseModels, parseSseLine } from "../src/gateway/checks.ts";

const v = JSON.parse(readFileSync(new URL("../../core/tests/vectors/gateway_client.json", import.meta.url), "utf8"));

const kindOf = (f: () => unknown): string => {
  try {
    f();
    return "ok";
  } catch (e) {
    assert.ok(e instanceof NetError, String(e));
    return e.kind;
  }
};

test("SSE lines", () => {
  for (const c of v.sse) {
    if (c.want.kind === "bad_answer") {
      assert.equal(kindOf(() => parseSseLine(c.line)), "bad_answer", c.why);
      continue;
    }
    assert.deepEqual(parseSseLine(c.line), c.want, c.why);
  }
});

test("model lists and ids", () => {
  for (const c of v.models) {
    if (c.want === "bad_answer") assert.equal(kindOf(() => parseModels(c.body)), "bad_answer", c.why);
    else assert.deepEqual(parseModels(c.body), c.want, c.why);
  }
  for (const c of v.model_ids) assert.equal(kindOf(() => checkModel(c.id)) === "ok", c.ok, c.why);
});

test("HTTP status", () => {
  for (const c of v.status) assert.equal(kindOf(() => checkStatus(c.code)), c.want, c.why);
});

test("URLs, names and keys", () => {
  for (const c of v.urls) assert.equal(checkUrl(c.url), c.want, c.why);
  for (const c of v.names) assert.equal(checkName(c.name), c.want, c.why);
  for (const c of v.keys) assert.equal(checkKey(c.key), c.want, c.why);
});
