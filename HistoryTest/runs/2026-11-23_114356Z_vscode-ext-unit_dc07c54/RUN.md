# 2026-11-23_114356Z_vscode-ext-unit_dc07c54 — VS Code extension unit tests incl. NEXUS Engine discovery and keyless client

- **Date (UTC) / duration:** 2026-11-23T11:43:56Z / 3.5 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** dc07c54892d791b3739a3fa897566a292ca8d58a (plus uncommitted changes) / not recorded
- **Area / components under test:** vscode-ext-unit (unit test)
- **Machine:** cloud container

## What was tested and why

Type check and 48 Node unit tests including shared gateway_client vectors and tests/nexus.test.ts.

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 48, failed 0, skipped 0 (packages); exit code 0; duration 3.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `dc07c54892d7` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_114356Z_vscode-ext-unit_dc07c54`
