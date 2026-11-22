# 2026-11-22_121951Z_core_cf2e079 — Core tests after the Windows temp-folder fix in agent_vectors.rs

- **Date (UTC) / duration:** 2026-11-22T12:19:51Z / 16.6 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** cf2e079c728ec65cd116fe5cc4252d22adc899d9 (plus uncommitted changes) / 24
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

PC core check on PR 24 (cf2e079) failed 3 agent::vectors tests on Windows with OS error 32: the temp folder was deleted while the Workspace still held a handle on it. The tests now drop the workspace before removing the folder. Linux run here; the PC check re-runs it on Windows.

## How it was run

cargo test --locked in core on the cloud container.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 16.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `cf2e079c728e` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121951Z_core_cf2e079`
