# 2026-11-17_065240Z_ui-smoke_e919fcd — Webview: type check, unit tests, build, UI smoke with tool steps and diff review

- **Date (UTC) / duration:** 2026-11-17T06:52:40Z / 8.8 s
- **Requested by / task or thread:** Claude
- **Commit / branch / PR:** e919fcd / branch claude/project-thread-dfop2y (I3 PR)
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

Fake core with an agent turn (tool steps, hostile text, two proposed changes), the Changes list, the Monaco diff review, accept, reject, Tools off.

## How it was run

npm test, npm run build, npm run ui-smoke

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 8.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `e919fcd` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-i3-chat.png`: other evidence
- `artifacts/ui-i3-diff.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_065240Z_ui-smoke_e919fcd`
