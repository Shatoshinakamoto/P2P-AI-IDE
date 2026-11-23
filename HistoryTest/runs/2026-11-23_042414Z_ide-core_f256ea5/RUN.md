# 2026-11-23_042414Z_ide-core_f256ea5 — PC IDE core checks for agent commits, run 3: pass (fmt, clippy, 122 tests)

- **Date (UTC) / duration:** 2026-11-23T04:24:14Z / not recorded
- **Requested by / task or thread:** git_commit local mode thread
- **Commit / branch / PR:** f256ea5 / claude/project-thread-2jylpu
- **Area / components under test:** ide-core (unit test)
- **Machine:** user PC (Windows)

## What was tested and why

Core re-run after the test-hook fix. The app (tsc, unit tests, vite build) and shell (fmt, clippy, tests) passed in run 2 at 5d10b7b; f256ea5 changes only a core test.

## How it was run

D:\p2p-ai-ci\run-ideac3.cmd: core cargo fmt --check, clippy -D warnings, cargo test --locked.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

fmt and clippy clean; 122 passed, 0 failed, 3 ignored (two need live services). Includes check_patch with 15 hostile patches and real_git_applies_an_agent_checkpoint with git for Windows (five hooks never run, staged work refused, a patch that does not fit changes nothing).

passed 122, failed 0, skipped 3.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

run D:\p2p-ai-ci\run-ideac3.cmd

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ideac3.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_042414Z_ide-core_f256ea5`
