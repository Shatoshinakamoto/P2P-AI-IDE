# 2026-11-23_150116Z_supply-chain_f6a0000 — cargo audit of both IDE lockfiles after the cookie bump

- **Date (UTC) / duration:** 2026-11-23T15:01:16Z / 1.9 s
- **Requested by / task or thread:** ide-tf-finish-thread
- **Commit / branch / PR:** f6a0000014733af17cf7a88b0d672294e711cd04 / 30
- **Area / components under test:** supply-chain (scan test)
- **Machine:** cloud container

## What was tested and why

Confirm RUSTSEC-2026-0594 is gone after cookie 0.18.3.

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `f6a000001473` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_150116Z_supply-chain_f6a0000`
