# `app/stubs/jschardet/` — stand-in for an unused LGPL library

**Added 2026-11-21** (phase I6a, thread "Finish the TandemForge and IDE roadmaps", requested by Pheonix).

## What it is

A two-function package named `jschardet` (`index.js`, `package.json`) that replaces the real
[jschardet](https://www.npmjs.com/package/jschardet) (LGPL-2.1+). `app/package.json` installs it with
`"jschardet": "file:stubs/jschardet"` and forces every dependency onto it with `"overrides": {"jschardet": "$jschardet"}`.

## Why it is there

`@codingame/monaco-vscode-api` (the VS Code editor services, phase I6a) lists jschardet as a dependency. VS Code uses
it only to guess the encoding of files it opens through its own file service. The IDE never does: the Rust core reads
every file as UTF-8 and hands the text to the editor. Shipping the real package would put an LGPL licence in the
installers, which the licence allow-list does not allow (P2P-AI `docs/files/licence-review.md`). The stand-in keeps
the allow-list as it is.

## Format and behaviour

`detect()` returns `{ encoding: null, confidence: 0 }` ("no guess"), and `detectAll()` returns `[]`. VS Code then uses
its configured encoding (UTF-8). The code parses no input and runs nothing.

## Who reads it

* npm installs it as a local link (`node_modules/jschardet -> stubs/jschardet`).
* Vite bundles it as a 193-byte chunk instead of the real detector.
* `scripts/third_party_notices.py` (P2P-AI) treats it as the project's own code (Apache-2.0) and does not list it.

## Changing or removing it

Remove it only if the IDE starts opening files through VS Code's file service and needs encoding guesses; then the
licence question must go to the project owner first. Check after every `@codingame/*` upgrade that the package still
calls only `detect()`: `grep -rn "jschardet" app/node_modules/@codingame/monaco-vscode-api/vscode/src`.

## Risks

If a future version of the editor services relied on real guesses, files in legacy encodings would open as UTF-8.
The IDE's own file reads are unaffected.
