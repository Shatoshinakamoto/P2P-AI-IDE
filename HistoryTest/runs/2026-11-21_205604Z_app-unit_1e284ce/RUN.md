# 2026-11-21_205604Z_app-unit_1e284ce — Phase I7 app unit tests

- **Date (UTC) / duration:** 2026-11-21T20:56:04Z / 0.3 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 1e284ce08423ed2b2a8ebe130b7e2497edeff82e / claude/project-thread-8u4b8a
- **Area / components under test:** app-unit (unit test)
- **Machine:** cloud container

## What was tested and why

App unit tests (unchanged by I7, run to show nothing broke).

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

Check out commit `1e284ce08423` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_205604Z_app-unit_1e284ce`
