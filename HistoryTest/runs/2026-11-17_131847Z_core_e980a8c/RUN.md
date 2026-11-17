# 2026-11-17_131847Z_core_e980a8c — Core: fmt, clippy, tests incl. real-kernel local sandbox confinement

- **Date (UTC) / duration:** 2026-11-17T13:18:47Z / 13.2 s
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** e980a8cf5f03ba1f5b44cfb2a71ca7e88e1e3390 / claude/project-thread-dfop2y
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

Core crate after phase I5c: localbox (confinement on the real kernel: no network, home hidden, writes only in the job folder, leftover processes gone) and testparse

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 13.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `e980a8cf5f03` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_131847Z_core_e980a8c`
