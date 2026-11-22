# 2026-11-22_113352Z_vscode-ext-integration_67b1744 — Extension in VS Code 1.95.0 (oldest supported), with an agent turn (X2)

- **Date (UTC) / duration:** 2026-11-22T11:33:52Z / 2.8 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X2
- **Commit / branch / PR:** 67b174490e053089e4799308c3954953b31bdbbb / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-integration (integration test)
- **Machine:** cloud container

## What was tested and why

Same suite as the stable run, on the oldest VS Code the extension supports (engines ^1.95.0), to check the diff editor and tab APIs used by the review.

## How it was run

xvfb-run -a node tests/integration/run.mjs 1.95.0 after npm run build.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `67b174490e05` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_113352Z_vscode-ext-integration_67b1744`
