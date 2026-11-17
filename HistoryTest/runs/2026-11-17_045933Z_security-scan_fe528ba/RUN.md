# 2026-11-17_045933Z_security-scan_fe528ba — cargo audit of both Rust lockfiles (RustSec)

- **Date (UTC) / duration:** 2026-11-17T04:59:33Z / 3.8 s
- **Requested by / task or thread:** Build the P2P-AI IDE app shell (phase I1)
- **Commit / branch / PR:** fe528ba89de319c40dfac36d7f8d56cfcc984b36 / claude/project-thread-dfop2y
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Known advisories in the Rust dependency trees of the core and the Tauri shell.

## How it was run

cargo-audit 0.22.1 with the RustSec advisory database fetched at run time; cargo audit --file core/Cargo.lock; cargo audit --file app/src-tauri/Cargo.lock

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 3.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cargo install cargo-audit --locked --version 0.22.1 && cargo audit --file core/Cargo.lock && cargo audit --file app/src-tauri/Cargo.lock

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_045933Z_security-scan_fe528ba`
