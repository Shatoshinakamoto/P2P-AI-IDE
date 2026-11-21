# 2026-11-21_232857Z_core_3e0d986 — Phase I6c-1 Node extension host core: unit tests

- **Date (UTC) / duration:** 2026-11-21T23:28:57Z / 16.5 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 3e0d9862c4318359498b473cfa5fd45d75c387d1 / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core library tests, with 6 new node_host tests: pins fail closed (no pin, bad hash shape, http, userinfo or other hosts refused); the download must match its pinned SHA-256 and a tampered one leaves nothing installed; unpack refuses .., absolute, backslash and drive paths, hard links, devices, duplicate entries and non-gzip data, skips symbolic links and keeps only the owner's execute bit; folder trust is per canonical folder; the server environment keeps only allow-listed names; the listen-line parser; start refuses an untrusted folder, passes the token in a 0600 file and never on the command line, and passes no secret environment (fake server script).

## How it was run

cargo test --lib in core on the cloud container.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 16.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `3e0d9862c431` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_232857Z_core_3e0d986`
