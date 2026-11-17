# 2026-11-17_090745Z_shell_86e1c4f — Tauri shell: fmt and clippy with the source control commands

- **Date (UTC) / duration:** 2026-11-17T09:07:45Z / 0.6 s
- **Requested by / task or thread:** IDE phase I5 thread
- **Commit / branch / PR:** 86e1c4fee534e381a9b6c24709784e9b2ea2b4b4 / claude/project-thread-dfop2y
- **Area / components under test:** shell (build test)
- **Machine:** cloud container

## What was tested and why

Shell compiles cleanly with the 6 scm commands

## How it was run

cargo fmt --check, cargo clippy --locked -D warnings

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `86e1c4fee534` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_090745Z_shell_86e1c4f`
