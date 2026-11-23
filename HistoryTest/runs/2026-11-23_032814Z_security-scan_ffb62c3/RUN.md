# 2026-11-23_032814Z_security-scan_ffb62c3 — Accounts (A3) security scan: RustSec, npm audit, token grep

- **Date (UTC) / duration:** 2026-11-23T03:28:14Z / 2.1 s
- **Requested by / task or thread:** Finish TandemForge and IDE thread
- **Commit / branch / PR:** ffb62c3d5247f9bff9f1f5e2c034912a9be10971 / claude/project-thread-w0i40c
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Dependency and secret checks for the account sign-in change: cargo audit on the core and the app shell lockfiles, npm audit (high and above) on the app, and a search for refresh-token-shaped strings in the source. gitleaks is not installed in this container, so it was skipped.

## How it was run

/tmp/claude-0/sp/a3-scan.sh from the repository root (script saved as an artifact).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 1; exit code 0; duration 2.1 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `ffb62c3d5247` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `artifacts/a3-scan.sh`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_032814Z_security-scan_ffb62c3`
