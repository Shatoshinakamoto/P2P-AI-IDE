# 2026-11-22_042549Z_core_c727219 — Core unit tests after the I6c-3 version check

- **Date (UTC) / duration:** 2026-11-22T04:25:49Z / 10.8 s
- **Requested by / task or thread:** tf-ide-roadmap-thread
- **Commit / branch / PR:** c727219c84eb42b22b5ed2c150218906c9ada5a4 (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

Core library tests (node_host and the rest) after VSCODE_VERSION was documented as the server's version and the shell started reporting it.

## How it was run

cargo test --lib in core/ (Linux, cloud).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 10.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `c727219c84eb` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_042549Z_core_c727219`
