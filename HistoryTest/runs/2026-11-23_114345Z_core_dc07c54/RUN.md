# 2026-11-23_114345Z_core_dc07c54 — Core tests incl. NEXUS Engine module, keyless Client, and Kind::Nexus

- **Date (UTC) / duration:** 2026-11-23T11:43:45Z / 2.5 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** dc07c54892d791b3739a3fa897566a292ca8d58a (plus uncommitted changes) / not recorded
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

p2p-ide-core unit and integration tests including nexus module discovery/probing, Kind::Nexus invariants, and keyless loopback Client.

## How it was run

cargo test --locked after cargo fmt --check and cargo clippy -- -D warnings.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `dc07c54892d7` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_114345Z_core_dc07c54`
