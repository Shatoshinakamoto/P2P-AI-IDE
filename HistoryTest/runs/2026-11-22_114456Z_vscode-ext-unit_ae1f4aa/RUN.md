# 2026-11-22_114456Z_vscode-ext-unit_ae1f4aa — Extension unit tests with missions (X3)

- **Date (UTC) / duration:** 2026-11-22T11:44:56Z / 0.5 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X3
- **Commit / branch / PR:** ae1f4aac034cb832c093dba02f90c81042afab72 / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-unit (unit test)
- **Machine:** cloud container

## What was tested and why

All extension unit tests, now with tests/missions.test.ts: the shared mission cases, push and pull against an in-memory project (secrets kept home, digests checked, nothing written without review), and the client's JSON, multipart upload and download calls against a fake gateway (fixed error text, no redirects, only /v1/ paths).

## How it was run

npm test (node --test) in vscode-extension/.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 33, failed 0, skipped 0 (packages); exit code 0; duration 0.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `ae1f4aac034c` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_114456Z_vscode-ext-unit_ae1f4aa`
