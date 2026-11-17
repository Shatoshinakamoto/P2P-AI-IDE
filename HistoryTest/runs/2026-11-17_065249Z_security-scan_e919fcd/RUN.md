# 2026-11-17_065249Z_security-scan_e919fcd — cargo audit and npm audit after phase I3 (no new dependencies)

- **Date (UTC) / duration:** 2026-11-17T06:52:49Z / 2.1 s
- **Requested by / task or thread:** Claude
- **Commit / branch / PR:** e919fcd / branch claude/project-thread-dfop2y (I3 PR)
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

RustSec for both lockfiles and npm advisories.

## How it was run

cargo audit --file each lockfile; npm audit in app

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.1 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `e919fcd` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_065249Z_security-scan_e919fcd`
