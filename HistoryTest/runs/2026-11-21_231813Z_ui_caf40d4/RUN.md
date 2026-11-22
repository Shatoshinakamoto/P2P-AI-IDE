# 2026-11-21_231813Z_ui_caf40d4 — Phase I6b UI smoke test: internet for web extensions asked once per extension (headless Chromium, shipped CSP)

- **Date (UTC) / duration:** 2026-11-21T23:18:13Z / 16.0 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** caf40d480c68b85817313924b88668170b48cb76 (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

Whole UI smoke test. New: the first https request of a web extension shows the consent card naming the site and nothing is made before an answer; Not now refuses; XMLHttpRequest, http, ipc and the Tauri bridge stay blocked; Allow internet in the Extensions view saves the grant for that extension only; allowed requests go through the core (ext_fetch) with their headers and get the answer; a fetch with no extension code on the stack is refused; a forged hello and forged requests get nothing made; Stop internet takes it away at once. Every HTML file is served with the CSP from tauri.conf.json.

## How it was run

npm run build, then node tests/ui-smoke.mjs against the built app with a fake core; screenshot ui-smoke-webext.png kept.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 16.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `caf40d480c68` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke-webext.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_231813Z_ui_caf40d4`
