# 2026-11-17_082340Z_ui-smoke_7e32c2d — Webview: UI smoke with the Missions view

- **Date (UTC) / duration:** 2026-11-17T08:23:40Z / 9.3 s
- **Requested by / task or thread:** IDE phase I4 thread
- **Commit / branch / PR:** 7e32c2df82985819e79ddbd175b063363b409bec / claude/project-thread-dfop2y
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

Typecheck, build, and the headless UI smoke with faked shell commands, including the I4 Missions flow

## How it was run

npm test, npm run build, npm run ui-smoke (Playwright Chromium, faked invoke)

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 9.3 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `7e32c2df8298` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-i4.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_082340Z_ui-smoke_7e32c2d`
