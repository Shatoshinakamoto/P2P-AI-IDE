# 2026-11-22_015916Z_core_506f165 — Phase I6c-3 core unit tests with the real pins (plus clippy on all targets)

- **Date (UTC) / duration:** 2026-11-22T01:59:16Z / 14.6 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 506f165cd7ebf5e3b111145fc445f1f359f22084 / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core tests with the five real VSCodium pins: every pin is well formed and on an allowed host, unlisted platforms (Windows on ARM, FreeBSD) get nothing. The new real-server test compiles and is ignored by default.

## How it was run

cargo clippy --all-targets -D warnings, then cargo test in core on the cloud container (the real-server test is skipped as ignored).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 14.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `506f165cd7eb` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_015916Z_core_506f165`
