# 2026-11-21_215005Z_core_fd65ba7 — Phase I6b core unit tests (web extensions install, Node and proposed-API extensions refused)

- **Date (UTC) / duration:** 2026-11-21T21:50:05Z / 10.8 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** fd65ba793886f4d8583d070938ec0042901556a2 / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core library tests. The extension test now checks that a web extension (browser entry, also with a main entry) installs as kind web, a Node-only one is refused, and a web extension asking for proposed APIs is refused with nothing written.

## How it was run

cargo test --lib in core on the cloud container.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 10.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `fd65ba793886` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_215005Z_core_fd65ba7`
