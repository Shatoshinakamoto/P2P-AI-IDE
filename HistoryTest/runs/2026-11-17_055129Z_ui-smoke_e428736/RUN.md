# 2026-11-17_055129Z_ui-smoke_e428736 — Type check, unit tests, build and UI smoke with the tab-language check

- **Date (UTC) / duration:** 2026-11-17T05:51:29Z / 6.6 s
- **Requested by / task or thread:** agentic IDE thread: Windows PC check
- **Commit / branch / PR:** e428736b8b64b6d0fee024225244eb2a34ec3108 / PR #1
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

Verifies the status-bar language fix. Before the fix, the new check failed (16 passed, 1 failed: footer still said python after opening README.md); this run is with the fix.

## How it was run

npm test (tsc + node unit tests), npm run build, npm run ui-smoke (headless Chromium, shipped CSP).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 6.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm test && npm run build && npm run ui-smoke

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_055129Z_ui-smoke_e428736`
