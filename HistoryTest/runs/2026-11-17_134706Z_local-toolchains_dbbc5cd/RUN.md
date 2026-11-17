# 2026-11-17_134706Z_local-toolchains_dbbc5cd — Real toolchains in the Linux sandbox with the corrected cache check

- **Date (UTC) / duration:** 2026-11-17T13:47:06Z / 6.2 s
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** dbbc5cd9eb2675eb40df1e3ce5f8cabea51967a1 / claude/project-thread-dfop2y
- **Area / components under test:** local-toolchains (integration test)
- **Machine:** cloud container

## What was tested and why

cargo x2, go, npm, make, unittest inside the Linux sandbox; the cache check now looks for cargo's build output in the per-folder cache

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 6.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `dbbc5cd9eb26` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_134706Z_local-toolchains_dbbc5cd`
