# 2026-11-23_122301Z_ui-click_dc07c54 — IDE desktop click-through on main (run 1)

- **Date (UTC) / duration:** 2026-11-23T12:23:01Z / 462.0 s
- **Requested by / task or thread:** ide-tf-finish-thread
- **Commit / branch / PR:** dc07c54 / main
- **Area / components under test:** ui-click (manual test)
- **Machine:** user PC (Windows)

## What was tested and why

Hands-on click-through of the debug IDE build on the Windows PC: open folder, editor, accounts panel, add a manual Ollama gateway, chat with and without Tools, extensions, missions, run and test, source control, remove gateway, close.

## How it was run

Fresh clone, npm ci --ignore-scripts, npx tauri build --debug --no-bundle; driven through WebView2 CDP with Playwright connectOverCDP; models qwen2.5-coder:7b and qwen2.5:14b on host Ollama. Screenshots stay on the PC (names and SHA-256 in artifacts/click-out-binaries.txt).

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

12 pass, 1 fail, 1 n/a of 14 steps. No console errors.

passed 12, failed 1, skipped 1; duration 462.0 s.

## Failures and analysis

Step 7: qwen2.5-coder:7b with Tools on returned its tool call as plain JSON text; nothing ran. Also seen: Run and test kept a stale 'add a gateway' reason; Missions offered a project link on a plain Ollama endpoint. Fixed in IDE PR #30.

## Verdict: partial

No reason recorded.

## How to reproduce

Check out commit `dc07c54` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `artifacts/click-out-binaries.txt`: other evidence
- `artifacts/commands.txt`: other evidence
- `artifacts/times.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide/app-stderr.log`: log of the run
- `logs/ide/build.log`: log of the run
- `logs/ide/console.log`: log of the run
- `logs/ide/steps.md`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_122301Z_ui-click_dc07c54`
