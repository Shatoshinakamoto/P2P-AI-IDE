# 2026-11-17_054000Z_windows_ba63e5c — Windows real-window check of ba63e5c: blank window (invalidates the earlier launch-only pass)

- **Date (UTC) / duration:** 2026-11-17T05:40:00Z / not recorded
- **Requested by / task or thread:** agentic IDE thread: Windows PC check
- **Commit / branch / PR:** ba63e5ca / PR #1
- **Area / components under test:** windows (e2e test)
- **Machine:** user PC (Windows)

## What was tested and why

First real look at the release window on Windows after the build run 051458Z had recorded 'window opens' from the process list only.

## How it was run

Launched p2p-ide.exe, captured the window, then read page errors over the WebView2 debug port (ide1-cdp.mjs).

The run was made elsewhere; its raw logs were copied in afterwards.

Collected from `D:\p2p-ai-ci\logs` on 2026-11-17T05:50:56Z.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Window blank: React never mounted. Page error: TypeError Cannot assign to read only property 'toString' (Monaco under freezePrototype). Cause fixed in e30cb3e.

passed 0, failed 1, skipped not counted.

## Failures and analysis

Not analysed in this record; see the logs.

## Verdict: fail

No reason recorded.

## How to reproduce

Run the ba63e5c release exe on Windows; set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9229 and run node ide1-cdp.mjs.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide1-cdp.mjs`: other evidence
- `artifacts/ide1-shot-01-blank.png`: other evidence
- `artifacts/ide1-shot.py`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/cdp-console.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_054000Z_windows_ba63e5c`
