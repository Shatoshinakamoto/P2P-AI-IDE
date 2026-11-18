# 2026-11-18_011129Z_security-scan_aee787c — npm audit gate on app/ and its unit tests before merging PR 15

- **Date (UTC) / duration:** 2026-11-18T01:11:29Z / 1.1 s
- **Requested by / task or thread:** Claude (housekeeping thread)
- **Commit / branch / PR:** aee787c77ca3cf320ea8cc11f78770befd599eb6 / PR #15 before merge
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

the live npm audit gate on app/ plus its unit tests, after P2P-AI #598 put the gate in local CI

## How it was run

python3 -m unittest scripts/test_npm_audit_gate.py; python3 scripts/npm_audit_gate.py app

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 14, failed 0, skipped 0 (tests); exit code 0; duration 1.1 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

python3 scripts/npm_audit_gate.py app

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-18_011129Z_security-scan_aee787c`
