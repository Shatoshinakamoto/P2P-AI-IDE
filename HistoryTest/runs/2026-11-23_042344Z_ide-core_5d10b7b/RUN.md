# 2026-11-23_042344Z_ide-core_5d10b7b — PC IDE checks for agent commits, run 2: 1 core failure (test hook ran in the test's own setup)

- **Date (UTC) / duration:** 2026-11-23T04:23:44Z / not recorded
- **Requested by / task or thread:** git_commit local mode thread
- **Commit / branch / PR:** 5d10b7b / claude/project-thread-2jylpu
- **Area / components under test:** ide-core (unit test)
- **Machine:** user PC (Windows)

## What was tested and why

Re-run after the line-ending fix.

## How it was run

D:\p2p-ai-ci\run-ideac2.cmd

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Everything passed except one core test.

passed 114, failed 1, skipped not counted.

## Failures and analysis

real_git_applies_an_agent_checkpoint: the marker hook fired during the test's own plain 'git commit' of the base state (git for Windows runs hooks without an exec bit; on Linux the hook ran too but the cloud run had stopped earlier). Not a product issue: apply_checkpoint keeps hooks off. The test now adds five hooks after its base commit (f256ea5).

## Verdict: fail

No reason recorded.

## How to reproduce

run D:\p2p-ai-ci\run-ideac2.cmd

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ideac2.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_042344Z_ide-core_5d10b7b`
