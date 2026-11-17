# 2026-11-17_140653Z_core_104f232 — Core fmt, clippy and tests (I5d)

- **Date (UTC) / duration:** 2026-11-17T14:06:53Z / 12.8 s
- **Requested by / task or thread:** ide-i5d
- **Commit / branch / PR:** 104f2326c1f5a31d5131c19955fc2aa2a97cbc27 / claude/project-thread-dfop2y
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

Core crate with the new container module and shared localbox helpers

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 12.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `104f2326c1f5` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_140653Z_core_104f232`
