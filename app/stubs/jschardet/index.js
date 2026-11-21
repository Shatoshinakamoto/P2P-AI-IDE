/**
 * stubs/jschardet/index.js -- a stand-in for the jschardet package (phase I6a, docs/files/jschardet-stub.md).
 *
 * Purpose: @codingame/monaco-vscode-api lists jschardet (LGPL-2.1+) as a dependency. VS Code uses it only to guess the
 * text encoding of files it opens through its own file service. This IDE never does that: the Rust core reads every
 * file as UTF-8 and hands the text to the editor. package.json's "overrides" points jschardet here, so the LGPL code
 * is neither installed nor shipped, and the licence allow-list (no GPL-family licences) still holds.
 *
 * Behaviour: detect() always answers "no guess", which VS Code treats as "use the configured encoding" (UTF-8).
 * Security: no input is parsed and nothing is executed; the function only returns a constant.
 */
"use strict";

/** Never guesses: VS Code then falls back to its configured encoding. */
function detect() {
  return { encoding: null, confidence: 0 };
}

/** The list form of detect(): always empty. */
function detectAll() {
  return [];
}

module.exports = { detect, detectAll };
