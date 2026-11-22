/**
 * tests/vscodeVersion.test.ts -- unit tests for src/lib/vscodeVersion.ts (run with `npm test`, node --test).
 * Covers the match rule and the fail-closed handling of malformed versions.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { minorOf, sameMinor } from "../src/lib/vscodeVersion.ts";

test("minorOf keeps major.minor", () => {
  assert.equal(minorOf("1.145.2"), "1.145");
  assert.equal(minorOf("1.140.0"), "1.140");
  assert.equal(minorOf("1.140"), "1.140");
  assert.equal(minorOf("01.0140.3"), "1.140");
});

test("minorOf refuses what isn't a version", () => {
  for (const v of ["", "1", "x.1.2", "1.x", "1.140abc", "11111.1.1", " ", "1..2"]) assert.equal(minorOf(v), null, v);
});

test("sameMinor matches only the same minor version", () => {
  assert.ok(sameMinor("1.145.2", "1.145.0"));
  assert.ok(!sameMinor("1.145.2", "1.140.0"), "the PC case: 1.145 editor, 1.140 server");
  assert.ok(!sameMinor("1.145.2", "1.14.5"));
});

test("sameMinor fails closed on bad input", () => {
  assert.ok(!sameMinor("", ""));
  assert.ok(!sameMinor("bad", "bad"));
  assert.ok(!sameMinor("1.145.2", ""));
});
