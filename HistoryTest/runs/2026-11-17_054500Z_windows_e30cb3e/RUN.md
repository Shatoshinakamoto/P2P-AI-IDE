# 2026-11-17_054500Z_windows_e30cb3e — Windows rebuild of the fix (e30cb3e) and manual click-through: 9/9

- **Date (UTC) / duration:** 2026-11-17T05:45:00Z / not recorded
- **Requested by / task or thread:** agentic IDE thread: Windows PC check
- **Commit / branch / PR:** e30cb3e / PR #1
- **Area / components under test:** windows (e2e test)
- **Machine:** user PC (Windows)

## What was tested and why

Checks the blank-window fix on the real Windows window: open folder, explorer, tabs, multi-cursor edit, save, outside-change conflict banner, reload, search, second tab.

## How it was run

ide1b-build.cmd (headless scheduled task): npm ci --ignore-scripts, npm test, shell clippy -D warnings, npm run release. Then screen control of the app only, test folder D:\p2p-ai-ci\ide1-ws; steps in ui-check.txt.

The run was made elsewhere; its raw logs were copied in afterwards.

Collected from `D:\p2p-ai-ci\logs\ide1b.txt` on 2026-11-17T05:50:56Z.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Build: npm ci, 4/4 unit tests, clippy clean, release 1m 38s, all exit 0. UI: 9/9 steps pass (ui-check.txt). One cosmetic bug found: status-bar language is stale after a tab switch (fixed next commit).

passed 9, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

Build e30cb3e on Windows (ide1b-build.cmd), open a small folder, follow ui-check.txt.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide1-shot-02-conflict.png`: other evidence
- `artifacts/ide1-shot-03-tabs.png`: other evidence
- `artifacts/ide1b-build.cmd`: other evidence
- `artifacts/ui-check.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide1b-windows.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_054500Z_windows_e30cb3e`
