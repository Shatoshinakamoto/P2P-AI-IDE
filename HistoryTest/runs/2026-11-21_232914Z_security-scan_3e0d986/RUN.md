# 2026-11-21_232914Z_security-scan_3e0d986 — Phase I6c-1 static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices

- **Date (UTC) / duration:** 2026-11-21T23:29:14Z / 13.8 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 3e0d9862c4318359498b473cfa5fd45d75c387d1 / claude/project-thread-8u4b8a
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Type check, formatting and lints for core and shell; RustSec scan of both lockfiles (new crates: tar 0.4.46 and filetime 0.2.30); npm audit gate; licence gate.

## How it was run

/tmp/claude-0/i6-static.sh from the repository root (same checks as phase I6a).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 5, failed 0, skipped 0 (packages); exit code 1; duration 13.8 s.

## Failures and analysis

Not analysed in this record; see the logs.

## Verdict: fail

The command ended with exit code 1.

## How to reproduce

Check out commit `3e0d9862c431` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_232914Z_security-scan_3e0d986`
