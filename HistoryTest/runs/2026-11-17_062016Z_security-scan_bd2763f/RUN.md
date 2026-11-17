# 2026-11-17_062016Z_security-scan_bd2763f — cargo audit of both lockfiles after adding ureq, serde_json, keyring

- **Date (UTC) / duration:** 2026-11-17T06:20:16Z / 1.9 s
- **Requested by / task or thread:** agentic-IDE-thread:phase-I2a
- **Commit / branch / PR:** bd2763f3950da51a3d069e709e8d9645877f833f / PR#1
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Supply-chain check of the new crates.

## How it was run

cargo audit 0.22 against the RustSec database fetched at run time

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cargo audit --file core/Cargo.lock && cargo audit --file app/src-tauri/Cargo.lock

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_062016Z_security-scan_bd2763f`
