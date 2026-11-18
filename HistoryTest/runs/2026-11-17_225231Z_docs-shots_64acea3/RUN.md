# 2026-11-17_225231Z_docs-shots_64acea3 — IDE documentation screenshots (10) from the built webview with the ui-smoke fake bridge

- **Date (UTC) / duration:** 2026-11-17T22:52:31Z / 8.7 s
- **Requested by / task or thread:** Write docs and README with screenshots
- **Commit / branch / PR:** 64acea317b35bcaac6645d1675eafa3b1c276a0a (plus uncommitted changes) / claude/project-thread-fudnju
- **Area / components under test:** docs-shots (e2e test)
- **Machine:** cloud container

## What was tested and why

Retakes the IDE pictures for README.md and docs/user-guide.md by walking every main view like a user. Each step must find its page and buttons; a failure stops the run.

## How it was run

npm ci --ignore-scripts && npm run build in app/, then node tests/docs-shots.mjs (headless Chromium, 1400x860, CSP from tauri.conf.json).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 10, failed not counted, skipped not counted; exit code 0; duration 8.7 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm ci --ignore-scripts && npm run build && node tests/docs-shots.mjs

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_225231Z_docs-shots_64acea3`
