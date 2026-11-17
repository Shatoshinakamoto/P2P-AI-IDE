# 2026-11-17_082359Z_security-scan_7e32c2d — cargo audit and npm audit for I4

- **Date (UTC) / duration:** 2026-11-17T08:23:59Z / 2.6 s
- **Requested by / task or thread:** IDE phase I4 thread
- **Commit / branch / PR:** 7e32c2df82985819e79ddbd175b063363b409bec / claude/project-thread-dfop2y
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Dependency CVE check for core, shell and webview lockfiles (no new dependencies in I4)

## How it was run

cargo audit on both Cargo.lock files (RustSec DB fetched live), npm audit

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.6 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_082359Z_security-scan_7e32c2d`
