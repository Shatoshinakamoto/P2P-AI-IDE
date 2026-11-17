# 2026-11-17_061800Z_windows_71a1e22 — Windows build and tests of phase I2a (core 29 tests, webview, shell, release exe)

- **Date (UTC) / duration:** 2026-11-17T06:18:00Z / not recorded
- **Requested by / task or thread:** agentic-IDE-thread:phase-I2a
- **Commit / branch / PR:** 71a1e22 / PR #1
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

Phase I2a (gateways, keychain, chat) on Windows 11 with MSVC: compile with keyring's Windows Credential Manager store, core tests including the loopback fake gateway, webview build, release exe.

## How it was run

ide2-build.cmd via a headless one-time scheduled task: core fmt/clippy/test, npm ci --ignore-scripts, npm test, build, npm audit, shell fmt/clippy, npm run release.

The run was made elsewhere; its raw logs were copied in afterwards.

Collected from `D:\p2p-ai-ci\logs\ide2.txt` on 2026-11-17T06:30:19Z.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

All exit codes 0. Core 29/29 (2 unix-only tests not compiled on Windows). npm audit 0. Release 2m 38s, p2p-ide.exe 10,570,240 bytes. The click-through with a local test gateway (fake_gateway.py) was started but stopped before any screen action because the owner was back in a game; it is still to do.

passed 29, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

On Windows: extract 71a1e22 over the ide1 folder and run ide2-build.cmd; read logs\ide2.txt.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/fake_gateway.py`: other evidence
- `artifacts/ide2-build.cmd`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide2-windows.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_061800Z_windows_71a1e22`
