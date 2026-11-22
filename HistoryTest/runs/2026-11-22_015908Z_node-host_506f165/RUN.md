# 2026-11-22_015908Z_node-host_506f165 — Phase I6c-3: the real pinned VSCodium 1.140.06880 server downloads, starts and needs its token (PC, Windows x64)

- **Date (UTC) / duration:** 2026-11-22T01:59:08Z / not recorded
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 506f165 / claude/project-thread-8u4b8a
- **Area / components under test:** node-host (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

First run against the real Node extension host. Through the core's own code (real_get, ensure_installed, TrustStore, stage_extensions, start): download the pinned Windows server from GitHub, check its SHA-256, unpack it safely, start it for a trusted temp folder on 127.0.0.1, then check /version is the pinned commit, a request without the connection token gets 403 and with it does not, and stopping closes the port. Also the node_host unit tests on Windows with the real pins, and the list of the server's own built-in extensions (to look for clashes with the editor's).

## How it was run

Background clone of the branch at 0125b87 on the PC with this commit's core changes applied as a patch (the same code as 506f165; only docs differ). Run at low priority: cargo test --test node_host_real -- --ignored --nocapture, then cargo test --lib node_host. Built-in extension names read from the tarball's file list.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

1 of 1 passed in 19.2 s: download, hash check and unpack 14.9 s; server listening after 0.70 s; /version 200 with commit 807ba2d6075c29587f884698cd7370c81ab0724c; / without token 403, with token 404 (accepted, no page at /); port closed after stop. Unit tests 6 of 6 passed (the seventh is Unix only). The server's built-ins are language features (TypeScript, JSON, CSS, HTML, Markdown, PHP, Emmet, Git, npm, js-debug) and no grammars or themes, so they don't duplicate the editor's grammar packages; github-authentication, microsoft-authentication and tunnel-forwarding are also there and still to be checked in the desktop app.

passed 7, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

On a Windows, Linux or macOS x64/arm64 computer with internet: cd core && cargo test --test node_host_real -- --ignored --nocapture

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/node_host_real.log`: log of the run
- `logs/node_host_unit.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/server-builtins.txt`: result file
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_015908Z_node-host_506f165`
