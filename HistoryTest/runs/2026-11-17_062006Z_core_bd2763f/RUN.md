# 2026-11-17_062006Z_core_bd2763f — Core fmt, clippy, tests with gateways/client/chats (31 tests)

- **Date (UTC) / duration:** 2026-11-17T06:20:06Z / 1.6 s
- **Requested by / task or thread:** agentic-IDE-thread:phase-I2a
- **Commit / branch / PR:** bd2763f3950da51a3d069e709e8d9645877f833f / PR#1
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

Phase I2a core: gateway list checks, URL allow-list, key never in files, fake loopback gateway (key only in header, redirects not followed, status mapping), SSE parsing and caps, chat history scopes and id checks, owner-only files.

## How it was run

cargo fmt --check; cargo clippy --all-targets --locked -D warnings; cargo test --locked

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd core && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_062006Z_core_bd2763f`
