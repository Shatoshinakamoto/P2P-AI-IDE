# 2026-11-17_225240Z_ui_64acea3 — UI smoke test after adding the docs screenshot script

- **Date (UTC) / duration:** 2026-11-17T22:52:40Z / 5.8 s
- **Requested by / task or thread:** Write docs and README with screenshots
- **Commit / branch / PR:** 64acea317b35bcaac6645d1675eafa3b1c276a0a (plus uncommitted changes) / claude/project-thread-fudnju
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

The UI smoke test still passes; the change only adds docs and a documentation script, so this confirms the fake bridge the script reuses is unchanged.

## How it was run

node tests/ui-smoke.mjs on the built dist/, screenshots to a scratch folder.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 5.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm run build && npm run ui-smoke

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_225240Z_ui_64acea3`
