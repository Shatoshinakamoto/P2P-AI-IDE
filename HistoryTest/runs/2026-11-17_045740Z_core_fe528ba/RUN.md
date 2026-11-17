# 2026-11-17_045740Z_core_fe528ba — Rust core: fmt, clippy and unit tests (path confinement, saves, search)

- **Date (UTC) / duration:** 2026-11-17T04:57:40Z / 0.8 s
- **Requested by / task or thread:** Build the P2P-AI IDE app shell (phase I1)
- **Commit / branch / PR:** fe528ba89de319c40dfac36d7f8d56cfcc984b36 / claude/project-thread-dfop2y
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

The workspace core that every file operation goes through: traversal, symlink escape, odd names, size caps, lost-update check, atomic saves, search limits.

## How it was run

cd core; cargo fmt --check; cargo clippy --all-targets --locked -- -D warnings; cargo test --locked

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd core && cargo test --locked

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_045740Z_core_fe528ba`
