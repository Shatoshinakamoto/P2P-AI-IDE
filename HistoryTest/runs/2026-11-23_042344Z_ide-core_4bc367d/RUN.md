# 2026-11-23_042344Z_ide-core_4bc367d — PC IDE checks for agent commits, run 1: 1 core failure (Windows line endings in the test)

- **Date (UTC) / duration:** 2026-11-23T04:23:44Z / not recorded
- **Requested by / task or thread:** git_commit local mode thread
- **Commit / branch / PR:** 4bc367d / claude/project-thread-2jylpu
- **Area / components under test:** ide-core (unit test)
- **Machine:** user PC (Windows)

## What was tested and why

Core fmt/clippy/tests, app typecheck, unit tests and build, shell fmt/clippy/tests on the PC for the agent commits feature.

## How it was run

D:\p2p-ai-ci\run-ideac1.cmd (shallow clone, cargo with the MSVC toolchain, npm ci).

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Everything passed except one core test; core integration tests did not run after the lib failure.

passed 114, failed 1, skipped not counted.

## Failures and analysis

real_git_applies_an_agent_checkpoint: the PC's git has core.autocrlf on, so the applied file had CRLF. The person's own setting applying is right; the test now fixes autocrlf=false in its own repository (5d10b7b).

## Verdict: fail

No reason recorded.

## How to reproduce

run D:\p2p-ai-ci\run-ideac1.cmd

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ideac1.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_042344Z_ide-core_4bc367d`
