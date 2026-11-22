# 2026-11-22_114505Z_vscode-ext-ui_ae1f4aa — Missions view webview (X3)

- **Date (UTC) / duration:** 2026-11-22T11:45:05Z / 0.4 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X3
- **Commit / branch / PR:** ae1f4aac034cb832c093dba02f90c81042afab72 / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-ui (e2e test)
- **Machine:** cloud container

## What was tested and why

Draws the real missions.js and missions.css in Chromium with a sample state (linked project, running mission waiting for approval, proposal, push report, approval arguments, markup in a goal). Clicks Approve; checks no markup was rendered and the view posts no digest of its own.

## How it was run

node tests/ui/missions-smoke.cjs (Playwright Chromium preinstalled in the container); screenshot in artifacts/.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.4 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `ae1f4aac034c` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/missions-x3.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_114505Z_vscode-ext-ui_ae1f4aa`
