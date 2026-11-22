# 2026-11-22_014626Z_ui_10da8d4 — Phase I6c-2 UI smoke test: Node extensions panel, trust, download, connection to the host (headless Chromium, shipped CS

- **Date (UTC) / duration:** 2026-11-22T01:46:26Z / 21.2 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 10da8d496c863f405e81f62d1a6193b5358d3bed (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

Whole UI smoke test. New: web extensions can't reach other local ports by XMLHttpRequest or WebSocket even though the page may now reach 127.0.0.1 for the Node host (a counting local server gets 0 hits); the Node extensions panel explains the risk and starts nothing before the folder is trusted; trusting and downloading are separate clicks; after the reload the editor connects only to ws://127.0.0.1:<host port>, announces the pinned commit, and sends the connection token over the socket (a stand-in server receives it), not in the URL or web storage; taking trust back stops the host and the reloaded editor connects to nothing.

## How it was run

npm run build, then node tests/ui-smoke.mjs against the built app with a fake core; screenshot ui-smoke-node.png kept.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 21.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `10da8d496c86` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke-node.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_014626Z_ui_10da8d4`
