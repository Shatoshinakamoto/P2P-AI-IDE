# 2026-11-17_063819Z_core_545bdf4 — Core: fmt, clippy, tests incl. shared advert vectors (phase I2b)

- **Date (UTC) / duration:** 2026-11-17T06:38:19Z / 1.7 s
- **Requested by / task or thread:** Claude
- **Commit / branch / PR:** 545bdf4 / PR #1
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

The IDE core after phase I2b: advert verifier ported from the node, tested against the shared protocol vectors, directory list, public-only resolver, listing with a fake directory.

## How it was run

cargo fmt --check, clippy -D warnings, cargo test --locked in core/

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.7 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_063819Z_core_545bdf4`
