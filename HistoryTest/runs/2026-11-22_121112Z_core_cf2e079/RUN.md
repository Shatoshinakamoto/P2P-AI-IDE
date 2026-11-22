# 2026-11-22_121112Z_core_cf2e079 — PC local CI core check on PR 24: 3 agent vector tests fail on Windows

- **Date (UTC) / duration:** 2026-11-22T12:11:12Z / not recorded
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** cf2e079c728ec65cd116fe5cc4252d22adc899d9 / 24
- **Area / components under test:** core (unit test)
- **Machine:** user PC (Windows)

## What was tested and why

PC local-ci core check (cargo test --locked) on the Windows PC for PR 24 head cf2e079, reported by the housekeeper thread.

## How it was run

Only a written report of this run is available here: the raw logs are not in this folder.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

97 passed, 3 failed; exit 101. shell, webview and supply-chain checks passed.

passed 97, failed 3, skipped not counted.

## Failures and analysis

agent::vectors::rel_path_cases, argument_cases and edit_range_and_delta_cases panic on unwrap() of OS error 32 (the process cannot access the file because it is being used by another process) when removing their temp folder.

## Verdict: fail

The tests removed the temp folder while the Workspace still held a handle on it; Windows refuses that. Fixed in the next commit (drop the workspace first). The full log stays on the PC under the local-ci runs folder.

## How to reproduce

Check out commit `cf2e079c728e` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121112Z_core_cf2e079`
