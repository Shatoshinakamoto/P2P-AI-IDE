# 2026-11-23_114250Z_ui_dc07c54 — UI smoke incl. NEXUS Engine and account-linked Chat/Missions

- **Date (UTC) / duration:** 2026-11-23T11:42:50Z / 34.1 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** dc07c54892d791b3739a3fa897566a292ca8d58a (plus uncommitted changes) / not recorded
- **Area / components under test:** ui (integration test)
- **Machine:** cloud container

## What was tested and why

Playwright UI smoke over dist/ covering Gateways, NEXUS Engine Proxy/Runtime, account-linked Chat and Missions, Node host, and CSP.

## How it was run

Built app/dist with vite, launched Chromium against fake Tauri bridge.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

All 133 UI smoke checks passed.

passed not counted, failed not counted, skipped not counted; exit code 0; duration 34.1 s.

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
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_114250Z_ui_dc07c54`
