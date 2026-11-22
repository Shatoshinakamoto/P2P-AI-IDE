# 2026-11-22_121653Z_security-scan_01bc30e — Phase I6d static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices

- **Date (UTC) / duration:** 2026-11-22T12:16:53Z / 16.2 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 01bc30e9ade3bdbbaa441d05b69ddde81f2f77df (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Type check, formatting and lints for core and shell; RustSec scan of both lockfiles (no new crates); npm audit gate; licence gate.

## How it was run

/tmp/claude-0/i6-static.sh from the repository root (same checks as phase I6a).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 5, failed 0, skipped 0 (packages); exit code 1; duration 16.2 s.

## Failures and analysis

Not analysed in this record; see the logs.

## Verdict: fail

The command ended with exit code 1.

## How to reproduce

Check out commit `01bc30e9ade3` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121653Z_security-scan_01bc30e`
