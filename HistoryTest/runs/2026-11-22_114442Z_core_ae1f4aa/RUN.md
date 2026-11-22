# 2026-11-22_114442Z_core_ae1f4aa — Core tests with the shared mission cases (X3)

- **Date (UTC) / duration:** 2026-11-22T11:44:42Z / 13.5 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X3
- **Commit / branch / PR:** ae1f4aac034cb832c093dba02f90c81042afab72 / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

The IDE core after adding core/src/projects_vectors.rs, which runs core/tests/vectors/missions.json (also run by the VS Code extension). Checks the Rust mission code matches the shared path grammar, ids, encoding, upload names, error codes, push and pull plans, view shapes and acceptance checks.

## How it was run

cargo test in core/ (Linux x86_64, cloud).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 13.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `ae1f4aac034c` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_114442Z_core_ae1f4aa`
