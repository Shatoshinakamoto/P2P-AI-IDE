# 2026-11-23_031826Z_core_ffb62c3 — IDE core tests with the accounts module

- **Date (UTC) / duration:** 2026-11-23T03:18:26Z / 13.4 s
- **Requested by / task or thread:** thread: Finish TandemForge and IDE roadmaps (A3 accounts)
- **Commit / branch / PR:** ffb62c3d5247f9bff9f1f5e2c034912a9be10971 / not recorded
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core tests after adding core/src/accounts.rs (PKCE, loopback listener, state/iss checks, rotation, cache, sign-out) and gateway account links.

## How it was run

cargo test in core/ (Linux x64 cloud container).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 13.4 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd core && cargo test

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_031826Z_core_ffb62c3`
