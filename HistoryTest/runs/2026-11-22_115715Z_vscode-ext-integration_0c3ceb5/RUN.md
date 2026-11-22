# 2026-11-22_115715Z_vscode-ext-integration_0c3ceb5 — Extension in VS Code 1.95.0 with missions (X3)

- **Date (UTC) / duration:** 2026-11-22T11:57:15Z / 3.3 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X3
- **Commit / branch / PR:** 0c3ceb549336daf705ee72a006c011fc890ae8c0 / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-integration (integration test)
- **Machine:** cloud container

## What was tested and why

Same suite on the oldest supported VS Code (engines ^1.95.0).

## How it was run

xvfb-run -a node tests/integration/run.mjs 1.95.0 after the build.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 3.3 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `0c3ceb549336` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_115715Z_vscode-ext-integration_0c3ceb5`
