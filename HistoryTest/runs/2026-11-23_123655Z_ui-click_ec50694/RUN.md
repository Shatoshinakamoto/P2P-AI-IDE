# 2026-11-23_123655Z_ui-click_ec50694 — IDE desktop click-through on latest main (run 2, fresh clone)

- **Date (UTC) / duration:** 2026-11-23T12:36:55Z / 245.0 s
- **Requested by / task or thread:** ide-tf-finish-thread
- **Commit / branch / PR:** ec50694 / main
- **Area / components under test:** ui-click (manual test)
- **Machine:** user PC (Windows)

## What was tested and why

Rerun of the click-through on a clean clone of the latest main, as asked 2026-11-23 12:32Z.

## How it was run

Same method as run 1 (fresh clone, debug build, WebView2 CDP + Playwright, host Ollama).

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

12 pass, 1 fail, 1 n/a of 14 steps. No console errors.

passed 12, failed 1, skipped 1; duration 245.0 s.

## Failures and analysis

Same 3 bugs as run 1 (text tool call shown as JSON; stale Run and test gateway reason; Missions link offered on a plain Ollama endpoint). Fixed in IDE PR #30.

## Verdict: partial

No reason recorded.

## How to reproduce

Check out commit `ec50694` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/click-out-binaries.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide2/app-stderr.log`: log of the run
- `logs/ide2/build.log`: log of the run
- `logs/ide2/console.log`: log of the run
- `logs/ide2/npm-ci.log`: log of the run
- `logs/ide2/steps.md`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_123655Z_ui-click_ec50694`
