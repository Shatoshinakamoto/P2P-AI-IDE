# 2026-11-17_051458Z_windows_ba63e5c — Windows PC build and tests of phase I1 (core, webview, shell, release exe)

- **Date (UTC) / duration:** 2026-11-17T05:14:58Z / not recorded
- **Requested by / task or thread:** agentic IDE thread: Windows PC check
- **Commit / branch / PR:** ba63e5ca / PR #1
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

The phase I1 editor shell had only been built and tested on Linux. This run builds and tests the same commit on the Windows 11 PC (MSVC toolchain, WebView2) and checks that the release app starts and opens its window.

## How it was run

Source tarball of ba63e5ca (SHA-256 edbaa8c4d2ad2ba1198f0249798762d16ab3112f5dab3cf83138675bbf94a6b1, checked by certutil in the log) extracted to a fresh folder; ide1-build.cmd ran headless via a one-time scheduled task: core cargo fmt --check, clippy -D warnings, cargo test --locked; npm ci --ignore-scripts, npm test, npm run build, npm audit; shell cargo fmt --check, clippy -D warnings; npm run release (tauri build --no-bundle). Then the exe was launched and checked with tasklist /v.

The run was made elsewhere; its raw logs were copied in afterwards.

Collected from `D:\p2p-ai-ci\logs\ide1.txt` on 2026-11-17T05:22:28Z.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

All step exit codes 0. Core: 9/9 tests pass (the 10th, the symlink escape test, is unix-only and not compiled on Windows). Webview: 4/4 unit tests, type check and build pass. npm audit: 0 vulnerabilities. Shell clippy clean. Release build 2m 22s, p2p-ide.exe 7,641,600 bytes. App launch: process running, window title P2P-AI IDE. Interactive UI check not done (owner was gaming; screen control would hide the game).

passed 13, failed 0, skipped 1.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

On Windows with Rust stable MSVC, Node 22 and WebView2: extract the PR #1 head, run ide1-build.cmd (adjust the D:\p2p-ai-ci paths), read logs\ide1.txt for the *-exit markers.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide1-build.cmd`: other evidence
- `artifacts/window-check.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide1-windows.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_051458Z_windows_ba63e5c`
