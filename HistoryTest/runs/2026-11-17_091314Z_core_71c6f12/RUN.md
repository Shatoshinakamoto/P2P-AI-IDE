# 2026-11-17_091314Z_core_71c6f12 — Core fmt, clippy and tests after the Windows-safe real-git test fix

- **Date (UTC) / duration:** 2026-11-17T09:13:14Z / 5.7 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 71c6f12078855af0355583200ff29e30452ee570 / PR #7
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

Rust core of P2P-AI-IDE at the I5a fix commit: the real-git round trip now stages a file named [a].txt literally (it would match a.txt as a pattern), because Windows cannot create a file named *.

## How it was run

cargo fmt --check, clippy -D warnings, cargo test --locked in core/

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 5.7 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `71c6f1207885` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_091314Z_core_71c6f12`
