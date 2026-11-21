# 2026-11-21_215016Z_ui_fd65ba7 — Phase I6b UI smoke test with a web extension in the worker host (headless Chromium, shipped CSP)

- **Date (UTC) / duration:** 2026-11-21T21:50:16Z / 12.1 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** fd65ba793886f4d8583d070938ec0042901556a2 (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

Whole UI smoke test with new I6b checks: a web extension installs and is labelled as one; its code runs in the web worker extension host and its hover shows in the editor; from there it cannot reach https, Tauri IPC, the Tauri bridge or the page DOM, and cannot open a link; the IDE page itself still cannot evaluate strings; the host runs in one hidden sandboxed iframe; removing the extension stops its code. Every HTML file is served with the CSP from tauri.conf.json, as Tauri does.

## How it was run

npm run build, then node tests/ui-smoke.mjs against the built app with a fake core; screenshot ui-smoke-webext.png kept.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 12.1 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `fd65ba793886` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke-webext.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_215016Z_ui_fd65ba7`
