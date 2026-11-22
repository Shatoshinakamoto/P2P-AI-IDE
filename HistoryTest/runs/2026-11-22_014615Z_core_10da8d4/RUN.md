# 2026-11-22_014615Z_core_10da8d4 — Phase I6c-2 core unit tests (Node extensions install, staged for the host, pinned commit)

- **Date (UTC) / duration:** 2026-11-22T01:46:15Z / 10.9 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 10da8d496c863f405e81f62d1a6193b5358d3bed / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core tests. New or changed: Node extensions install as kind node and can't get the web worker's internet grant, extensions asking for proposed APIs are refused for every kind with code; stage_extensions copies only enabled Node extensions into the host's folder, rebuilt fresh, never copying links; pins need a 40-hex commit.

## How it was run

cargo test --lib in core on the cloud container.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 10.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `10da8d496c86` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_014615Z_core_10da8d4`
