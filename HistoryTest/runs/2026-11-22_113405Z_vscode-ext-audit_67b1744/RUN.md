# 2026-11-22_113405Z_vscode-ext-audit_67b1744 — npm audit of the extension (X2)

- **Date (UTC) / duration:** 2026-11-22T11:34:05Z / 0.4 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X2
- **Commit / branch / PR:** 67b174490e053089e4799308c3954953b31bdbbb / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-audit (scan test)
- **Machine:** cloud container

## What was tested and why

Known-vulnerability check of every npm package the extension's build uses (dev dependencies only; it has no run-time dependencies). X2 added no package.

## How it was run

npm audit in vscode-extension/ against the npm registry advisory database.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.4 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_113405Z_vscode-ext-audit_67b1744`
