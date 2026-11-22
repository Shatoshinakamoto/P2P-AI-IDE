# 2026-11-22_114456Z_vscode-ext-integration_ae1f4aa — Extension in real VS Code (stable) with missions (X3)

- **Date (UTC) / duration:** 2026-11-22T11:44:56Z / 5.9 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X3
- **Commit / branch / PR:** ae1f4aac034cb832c093dba02f90c81042afab72 / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-integration (integration test)
- **Machine:** cloud container

## What was tested and why

The full suite in the newest stable VS Code against a fake gateway with an in-memory project: X1 chat, X2 agent turn and review, and X3 create and link a project, send the folder (.env kept home), switches, start Autopilot (checks required), approval bound to the fetched digest, bring work back for review (.env never brought back), accept, triage a proposal, fixed text on a version conflict.

## How it was run

npm run build, then xvfb-run -a node tests/integration/run.mjs.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 1; duration 5.9 s.

## Failures and analysis

Not analysed in this record; see the logs.

## Verdict: fail

The command ended with exit code 1.

## How to reproduce

Check out commit `ae1f4aac034c` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_114456Z_vscode-ext-integration_ae1f4aa`
