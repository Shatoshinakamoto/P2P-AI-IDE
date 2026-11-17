# 2026-11-17_132300Z_windows_a8b439e — Windows build and tests of phase I5c (local sandbox, Linux code left out) at a8b439e

- **Date (UTC) / duration:** 2026-11-17T13:23:00Z / not recorded
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** a8b439e / PR #10
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

P2P-AI-IDE on the Windows PC: the Linux-only sandbox code is cfg-gated out and the rest builds: Rust core fmt/clippy/tests, npm ci without install scripts, webview tests and build, npm audit, Tauri shell fmt/clippy with runners_list and run_on, release binary.

## How it was run

D:\p2p-ai-ci\ide5c-build.cmd started hidden through a scheduled task; source tarball a8b439e with SHA-256 c8fe9ca9... in the log (matches the cloud copy).

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Every step exited 0: core 62 + 5 + 2 tests (the Linux-only sandbox tests do not build on Windows), webview tests, build, audit 0, shell fmt and clippy, release p2p-ide.exe 11.8 MB.

passed 69, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

Check out commit `a8b439e` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5c.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_132300Z_windows_a8b439e`
