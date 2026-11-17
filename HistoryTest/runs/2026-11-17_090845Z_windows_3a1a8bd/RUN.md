# 2026-11-17_090845Z_windows_3a1a8bd — Windows build and tests of phase I5a (Source Control) at 3a1a8bd

- **Date (UTC) / duration:** 2026-11-17T09:08:45Z / not recorded
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 3a1a8bd / PR #7
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

P2P-AI-IDE on the Windows PC (Git for Windows): Rust core fmt/clippy/tests, npm ci without install scripts, webview tests and build, npm audit, Tauri shell fmt/clippy, release binary.

## How it was run

D:\p2p-ai-ci\ide5a-build.cmd started hidden through a scheduled task; source tarball 3a1a8bd with SHA-256 in the log.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Every step exited 0 except the core tests: 54 passed, 1 failed. Release binary p2p-ide.exe 11.7 MB built.

passed 54, failed 1, skipped not counted.

## Failures and analysis

scm::tests::real_git_round_trip_without_hooks failed with InvalidFilename (os error 123): the test created a file named * which Windows does not allow. Test setup fault, not product code. Fixed in 71c6f12 (the test now uses [a].txt); see the run 2026-11-17 ide5a2 core re-run on the PC.

## Verdict: fail

One core test failed on Windows (test setup); fixed and re-run separately.

## How to reproduce

Copy the tarball and ide5a-build.cmd to D:\p2p-ai-ci and run the cmd file; log in D:\p2p-ai-ci\logs\ide5a.txt

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide5a-build.cmd`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5a.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_090845Z_windows_3a1a8bd`
