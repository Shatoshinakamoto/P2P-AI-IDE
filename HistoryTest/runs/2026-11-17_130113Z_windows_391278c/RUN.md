# 2026-11-17_130113Z_windows_391278c — Windows build and tests of phase I5b (Run and test) at 391278c

- **Date (UTC) / duration:** 2026-11-17T13:01:13Z / not recorded
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 391278c / PR #9
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

P2P-AI-IDE on the Windows PC: Rust core fmt/clippy/tests (incl. runner and agent run tools), npm ci without install scripts, webview tests and build, npm audit, Tauri shell fmt/clippy with run_start and agent_decide, release binary.

## How it was run

D:\p2p-ai-ci\ide5b-build.cmd started hidden through a scheduled task; source tarball 391278c with SHA-256 in the log (matches the cloud copy).

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Every step exited 0: core 59 + 5 + 2 tests (2 unix-only tests do not build on Windows), webview tests, build, audit 0, shell fmt and clippy, release p2p-ide.exe 11.8 MB.

passed 66, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

All steps passed on Windows.

## How to reproduce

Copy the tarball and ide5b-build.cmd to D:\p2p-ai-ci and run the cmd file; log in D:\p2p-ai-ci\logs\ide5b.txt

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide5b-build.cmd`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5b.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_130113Z_windows_391278c`
