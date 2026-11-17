# 2026-11-17_131900Z_local-toolchains_e980a8c — Real toolchains in the local sandbox (cargo x2, go, npm, make, unittest)

- **Date (UTC) / duration:** 2026-11-17T13:19:00Z / 5.9 s
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** e980a8cf5f03ba1f5b44cfb2a71ca7e88e1e3390 / claude/project-thread-dfop2y
- **Area / components under test:** local-toolchains (integration test)
- **Machine:** cloud container

## What was tested and why

toolchain mode lets real build and test tools run inside the confinement; second cargo run reuses the cache

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 5.9 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_131900Z_local-toolchains_e980a8c`
