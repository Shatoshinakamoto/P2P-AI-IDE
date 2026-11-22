# 2026-11-22_112220Z_vscode-ext-integration_a704618 — Extension in real VS Code 1.95.0 (oldest supported) against a fake gateway

- **Date (UTC) / duration:** 2026-11-22T11:22:20Z / 2.5 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** a704618f4d39d4acf7e18f5c64eeeea02da9d658 / not recorded
- **Area / components under test:** vscode-ext-integration (e2e test)
- **Machine:** cloud container

## What was tested and why

same suite on engines.vscode minimum

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 2.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `a704618f4d39` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_112220Z_vscode-ext-integration_a704618`
