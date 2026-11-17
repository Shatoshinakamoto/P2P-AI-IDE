# 2026-11-17_140846Z_windows-containers_8bba2b4 — Container runner on the PC with Docker Desktop 29.9.2 at 8bba2b4: core tests and real containers

- **Date (UTC) / duration:** 2026-11-17T14:08:46Z / not recorded
- **Requested by / task or thread:** ide-i5d
- **Commit / branch / PR:** 8bba2b4 / claude/project-thread-dfop2y
- **Area / components under test:** windows-containers (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

The I5d container runner on Windows with Docker Desktop (Linux engine): core fmt/clippy/tests, then Get image for all four images (three UBI pulls by digest and the local Rust build) and real confined runs.

## How it was run

D:\p2p-ai-ci\ide5d-test.cmd started hidden through a scheduled task (conhost --headless). Source tarball 8bba2b4, SHA-256 c4d34ddc... in the log matches the cloud copy. Commit 104f232 adds only docs and a wider forbidden-flag unit test on top.

The run was made elsewhere; its raw logs were copied in afterwards.

Collected from `D:\p2p-ai-ci\logs\ide5d.txt` on 2026-11-17T14:08:46Z.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

fmt and clippy exit 0. Core 67 + 5 + 2 passed, including local_runs_are_confined_on_windows. Containers: engine docker; get image python, node, go ok, rust built ok. Confinement: /work read/write, .env not copied, read-only image, no network, real folder untouched, not root, time limit ends the container. cargo, go, npm, make and unittest projects each found their failing test. 12 passed, 0 failed, 0 skipped.

passed 86, failed 0, skipped 0.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

Check out commit `8bba2b4` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `artifacts/ide5d-test.cmd`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5d-pc.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_140846Z_windows-containers_8bba2b4`
