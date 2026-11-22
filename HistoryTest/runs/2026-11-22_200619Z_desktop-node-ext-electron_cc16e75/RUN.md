# 2026-11-22_200619Z_desktop-node-ext-electron_cc16e75 — I6d Electron compatibility host in the Windows desktop app

- **Date (UTC) / duration:** 2026-11-22T20:06:19Z / not recorded
- **Requested by / task or thread:** TandemForge and IDE roadmap thread
- **Commit / branch / PR:** cc16e75 / #25
- **Area / components under test:** desktop-node-ext-electron (manual test)
- **Machine:** user PC (Windows)

## What was tested and why

Checks I6d end to end in the real Windows desktop app: the Electron compatibility switch is off by default, turning it on downloads the pinned Electron 43.8.3, and the pinned Node extension server and extension host then run under Electron in Node mode with a real Open VSX extension (Code Spell Checker) working.

## How it was run

PC, Windows x64. Built main cc16e75 (core electron_host and node_host tests, npm ci, release build; log in logs). Opened a trusted test folder, opened the Extensions view, ticked 'Run Node extensions with Electron 43.8.3 (compatibility)', clicked 'Download and start Node extensions'. Listed the processes (i6d-procs.txt), copied the extension host log, opened a Markdown file with a misspelled word and hovered it. Then unticked the switch to restore the default and closed the app.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Build: electron_host 7/7, node_host 7/7, npm ci exit 0, release exit 0. Switch off by default; panel named the Electron 43.8.3 (about 150 MB) download. After the click the panel read 'running ... with Electron 43.8.3 (compatibility)'. electron.exe from the pinned electron-host folder ran server-main and extensionHost. Code Spell Checker activated (log line) and the hover reads 'Misspelled word. cSpell'. The extension host log has no errors. Unticking removed the switch and the panel showed the plain Start button again.

passed 4, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

On Windows x64: build the app from main, open a trusted folder, tick the Electron compatibility box in Extensions, click Download and start Node extensions, install Code Spell Checker, open a Markdown file with a typo.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/cspell-hover.jpg`: other evidence
- `artifacts/panel-electron-running.jpg`: other evidence
- `artifacts/panel-switch-off-again.jpg`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/i6d-build.txt`: log of the run
- `logs/i6d-exthost.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/i6d-procs.txt`: result file
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_200619Z_desktop-node-ext-electron_cc16e75`
