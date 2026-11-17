# 2026-11-17_045806Z_ui-smoke_fe528ba — UI smoke: built webview in headless Chromium with a fake workspace, under the shipped CSP

- **Date (UTC) / duration:** 2026-11-17T04:58:06Z / 2.4 s
- **Requested by / task or thread:** Build the P2P-AI IDE app shell (phase I1)
- **Commit / branch / PR:** fe528ba89de319c40dfac36d7f8d56cfcc984b36 / claude/project-thread-dfop2y
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

Layout, explorer, Monaco open/edit, unsaved marker, Ctrl+S with the loaded version, multi-cursor edit, conflict banner and reload, tabs, search, later-phase views, no CSP violations or page errors.

## How it was run

cd app; npm run build; node tests/ui-smoke.mjs (Playwright 1.63.0 Chromium; the six Tauri commands are replaced by an in-memory fake; dist/ is served with the CSP from tauri.conf.json)

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.4 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm run build && npm run ui-smoke

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_045806Z_ui-smoke_fe528ba`
