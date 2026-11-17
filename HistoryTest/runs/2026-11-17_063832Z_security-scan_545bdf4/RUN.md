# 2026-11-17_063832Z_security-scan_545bdf4 — cargo audit of both lockfiles after adding ed25519-dalek, base64, getrandom

- **Date (UTC) / duration:** 2026-11-17T06:38:32Z / 1.8 s
- **Requested by / task or thread:** Claude
- **Commit / branch / PR:** 545bdf4 / PR #1
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

RustSec advisories for core and shell lockfiles.

## How it was run

cargo audit --file for each lockfile

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `545bdf4` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_063832Z_security-scan_545bdf4`
