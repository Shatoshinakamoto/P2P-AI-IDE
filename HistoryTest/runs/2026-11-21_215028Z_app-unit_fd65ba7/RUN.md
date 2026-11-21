# 2026-11-21_215028Z_app-unit_fd65ba7 — Phase I6b app unit tests

- **Date (UTC) / duration:** 2026-11-21T21:50:28Z / 0.3 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** fd65ba793886f4d8583d070938ec0042901556a2 (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** app-unit (unit test)
- **Machine:** cloud container

## What was tested and why

App unit tests, run to show nothing broke.

## How it was run

npm test in app.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 0.3 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_215028Z_app-unit_fd65ba7`
