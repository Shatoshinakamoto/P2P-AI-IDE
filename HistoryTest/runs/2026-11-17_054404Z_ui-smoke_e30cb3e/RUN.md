# 2026-11-17_054404Z_ui-smoke_e30cb3e — UI smoke after the blank-window fix (freezePrototype off; test now applies the freeze when configured)

- **Date (UTC) / duration:** 2026-11-17T05:44:04Z / 14.3 s
- **Requested by / task or thread:** agentic IDE thread: Windows PC check
- **Commit / branch / PR:** e30cb3ee136651105e1ebba3a65de3a3e6615892 / PR #1
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

The release window was blank on Windows (Monaco throws under a frozen Object.prototype). Before this commit the same test with the freeze applied failed with that TypeError (2 of 3 checks failed: flow timeout, page error), reproducing the bug. This run checks the fixed config.

## How it was run

npm run build, then npm run ui-smoke (headless Chromium, shipped CSP, fake Tauri bridge).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 14.3 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm run build && npm run ui-smoke

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_054404Z_ui-smoke_e30cb3e`
