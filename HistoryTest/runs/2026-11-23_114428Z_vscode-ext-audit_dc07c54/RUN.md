# 2026-11-23_114428Z_vscode-ext-audit_dc07c54 — Supply-chain npm audit gate on vscode-extension

- **Date (UTC) / duration:** 2026-11-23T11:44:28Z / 1.6 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** dc07c54892d791b3739a3fa897566a292ca8d58a (plus uncommitted changes) / not recorded
- **Area / components under test:** vscode-ext-audit (scan test)
- **Machine:** cloud container

## What was tested and why

Runs scripts/npm_audit_gate.py on vscode-extension/ plus scripts/test_npm_audit_gate.py.

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.6 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_114428Z_vscode-ext-audit_dc07c54`
