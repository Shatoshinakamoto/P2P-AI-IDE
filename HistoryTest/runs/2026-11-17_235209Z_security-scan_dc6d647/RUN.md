# 2026-11-17_235209Z_security-scan_dc6d647 — npm audit gate: unit tests and live gate on app/

- **Date (UTC) / duration:** 2026-11-17T23:52:09Z / 0.8 s
- **Requested by / task or thread:** picomatch ReDoS thread
- **Commit / branch / PR:** dc6d6473062820a4ff42436332672c54c02fa950 (plus uncommitted changes) / #15
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

The new gate (owner's choice): 14 negative/positive unit tests, then the live gate on app/ with the picomatch exception.

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 14, failed 0, skipped 0 (tests); exit code 0; duration 0.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `dc6d64730628` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_235209Z_security-scan_dc6d647`
