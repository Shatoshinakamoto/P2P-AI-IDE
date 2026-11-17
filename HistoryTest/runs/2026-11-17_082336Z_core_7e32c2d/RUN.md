# 2026-11-17_082336Z_core_7e32c2d — Core: fmt, clippy and tests with I4 projects sync

- **Date (UTC) / duration:** 2026-11-17T08:23:36Z / 3.2 s
- **Requested by / task or thread:** IDE phase I4 thread
- **Commit / branch / PR:** 7e32c2df82985819e79ddbd175b063363b409bec / claude/project-thread-dfop2y
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

Phase I4 core: gateway API helpers (gwapi), project push and pull (projects), external proposals; plus all earlier tests

## How it was run

cargo fmt --check, clippy -D warnings, cargo test --locked; projects integration test against an in-process fake gateway on 127.0.0.1

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 3.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `7e32c2df8298` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_082336Z_core_7e32c2d`
