# 2026-11-17_134223Z_windows-sandbox_8819a48 — Second Windows sandbox run on the PC at 8819a48 (excerpt only)

- **Date (UTC) / duration:** 2026-11-17T13:42:23Z / not recorded
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** 8819a48 / claude/project-thread-dfop2y
- **Area / components under test:** windows-sandbox (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

Re-run after the first fixes: core tests incl. the real Windows confinement test, then real toolchains.

## How it was run

D:\p2p-ai-ci\ide5w-test.cmd through a hidden scheduled task. Only an excerpt is kept: the log file was overwritten by the next run before it was archived.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Core 65 + 5 + 2 passed (the Windows confinement test passed in full). Toolchains: cargo, go, unittest PASS; make SKIP; npm FAIL (Program Files\nodejs unreadable by app containers); the cargo cache check FAIL (second run 27 s vs first 32 s).

passed 76, failed 2, skipped 1.

## Failures and analysis

The cache check measured a claim that was wrong (the project crate is rebuilt each run because the copy's path changes); replaced in dbbc5cd by a check that build output lands in the cache, and the docs and the earlier Linux record were corrected. npm: environment limit, documented.

## Verdict: fail

No reason recorded.

## How to reproduce

Check out commit `8819a48` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5w-2-excerpt.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_134223Z_windows-sandbox_8819a48`
