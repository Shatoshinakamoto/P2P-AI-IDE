# 2026-11-23_040711Z_ide-core_d834367 — IDE core scm tests for agent checkpoints (cloud, first try: 2 failures)

- **Date (UTC) / duration:** 2026-11-23T04:07:11Z / not recorded
- **Requested by / task or thread:** git_commit local mode thread
- **Commit / branch / PR:** d83436778e23f9d41838af701742f07f78d404ac (plus uncommitted changes) / claude/project-thread-2jylpu
- **Area / components under test:** ide-core (unit test)
- **Machine:** cloud container

## What was tested and why

First compile-and-test of the new check_patch and apply_checkpoint in core/src/scm.rs, run in the cloud container before the PC-only rule was applied to this step.

## How it was run

cd core && cargo test -q --lib scm (uncommitted working tree on top of d834367).

Only a written report of this run is available here: the raw logs are not in this folder.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

4 passed, 2 failed.

passed 4, failed 2, skipped not counted.

## Failures and analysis

check_patch accepted '../evil' when the caller's allow-list accepted everything (the checker now refuses .., ., empty, absolute, backslash, colon and .git parts itself). apply_checkpoint failed with NotApplied after an unstage: git apply --index compared stale stat data (now runs update-index --refresh first). Both fixed before the PC run.

## Verdict: fail

No reason recorded.

## How to reproduce

Check out commit `d83436778e23` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_040711Z_ide-core_d834367`
