# 2026-11-17_125226Z_shell_6ebd731 — Tauri shell clippy (not possible in the cloud container)

- **Date (UTC) / duration:** 2026-11-17T12:52:26Z / 0.7 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 6ebd731048dc7e623d68f32e86579910ca485438 / claude/project-thread-1x3ij1
- **Area / components under test:** shell (build test)
- **Machine:** cloud container

## What was tested and why

Not described.

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 101; duration 0.7 s.

## Failures and analysis

Not analysed in this record; see the logs.

## Verdict: invalid

The command ended with exit code 101.

## How to reproduce

Check out commit `6ebd731048dc` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_125226Z_shell_6ebd731`
