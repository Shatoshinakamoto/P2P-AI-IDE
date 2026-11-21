# 2026-11-21_173347Z_core_848b60e — Core unit tests incl. 9 new Open VSX extension tests

- **Date (UTC) / duration:** 2026-11-21T17:33:47Z / 10.9 s
- **Requested by / task or thread:** Finish the TandemForge and IDE roadmaps
- **Commit / branch / PR:** 848b60eea05e6f9400485da287d314e214ef32dd / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

p2p-ide-core after phase I6a: search, URL and name allow-lists, Ed25519 PEM parsing, signature, zip slip and bomb refusal, store list/enable/remove, safe file reads.

## How it was run

cargo test --lib in core/

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 10.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `848b60eea05e` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_173347Z_core_848b60e`
