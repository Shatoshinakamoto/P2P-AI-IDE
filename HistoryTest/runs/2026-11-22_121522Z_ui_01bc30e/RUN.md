# 2026-11-22_121522Z_ui_01bc30e — Phase I6d: UI smoke test with the Electron compatibility switch

- **Date (UTC) / duration:** 2026-11-22T12:15:22Z / 26.6 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 01bc30e9ade3bdbbaa441d05b69ddde81f2f77df (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

The whole editor UI smoke test, with 4 new checks: compatibility off by default and nothing called; the hint names extensions as text, never markup; turning it on is one call from the click and the start names both downloads; the running host says it uses Electron.

## How it was run

npm run build, then npm run ui-smoke in app (mocked shell).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 26.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `01bc30e9ade3` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke-node.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121522Z_ui_01bc30e`
