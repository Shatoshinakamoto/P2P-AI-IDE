# 2026-11-17_125223Z_core_6ebd731 — Core tests, fmt and clippy after moving workspace file access to cap-std handles

- **Date (UTC) / duration:** 2026-11-17T12:52:23Z / 2.9 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 6ebd731048dc7e623d68f32e86579910ca485438 / claude/project-thread-1x3ij1
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

workspace.rs on a cap-std Dir handle; new tests: folder swapped for a link after open is refused; workspace follows its renamed folder

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `6ebd731048dc` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_125223Z_core_6ebd731`
