/**
 * tests/paths.test.ts -- unit tests for src/lib/paths.ts (run with `npm test`, node --test).
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { baseName, crumbs, dirName, languageFor, sizeLabel } from "../src/lib/paths.ts";

test("baseName and dirName split on /", () => {
  assert.equal(baseName("src/a/main.py"), "main.py");
  assert.equal(baseName("README.md"), "README.md");
  assert.equal(dirName("src/a/main.py"), "src/a");
  assert.equal(dirName("README.md"), "");
});

test("crumbs", () => {
  assert.deepEqual(crumbs("src/a/b.ts"), ["src", "a", "b.ts"]);
  assert.deepEqual(crumbs(""), []);
});

test("languageFor maps extensions and names", () => {
  assert.equal(languageFor("src/App.tsx"), "typescript");
  assert.equal(languageFor("main.py"), "python");
  assert.equal(languageFor("core/lib.RS"), "rust");
  assert.equal(languageFor("Dockerfile"), "dockerfile");
  assert.equal(languageFor(".env"), "plaintext");
  assert.equal(languageFor("noext"), "plaintext");
  assert.equal(languageFor("x.unknown"), "plaintext");
});

test("sizeLabel", () => {
  assert.equal(sizeLabel(10), "10 B");
  assert.equal(sizeLabel(2048), "2.0 KiB");
  assert.equal(sizeLabel(5 * 1024 * 1024), "5.0 MiB");
});
