# 2026-11-17_091518Z_windows_71c6f12 — Windows core re-run after the real-git test fix (71c6f12)

- **Date (UTC) / duration:** 2026-11-17T09:15:18Z / not recorded
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 71c6f12 / PR #7
- **Area / components under test:** windows (unit test)
- **Machine:** user PC (Windows)

## What was tested and why

Rust core of P2P-AI-IDE on the Windows PC after the test fix: fmt, clippy, all tests including the real-git round trip with Git for Windows 2.56 (hook must not run, pattern-like name staged literally).

## How it was run

D:\p2p-ai-ci\ide5a2-core.cmd started hidden through a scheduled task, own target folder; source tarball 71c6f12 with SHA-256 in the log.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

fmt 0, clippy 0, tests 55 + 5 + 2 passed, 0 failed (2 fewer than Linux: unix-only tests).

passed 62, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

All core checks pass on Windows, including the real-git round trip.

## How to reproduce

Copy the tarball and ide5a2-core.cmd to D:\p2p-ai-ci and run the cmd file; log in D:\p2p-ai-ci\logs\ide5a2.txt

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide5a2-core.cmd`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5a2.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_091518Z_windows_71c6f12`
