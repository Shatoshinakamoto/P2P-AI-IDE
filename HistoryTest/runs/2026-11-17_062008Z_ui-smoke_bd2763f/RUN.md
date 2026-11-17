# 2026-11-17_062008Z_ui-smoke_bd2763f — Webview type check, unit tests, build and UI smoke (30 checks incl. gateways and chat)

- **Date (UTC) / duration:** 2026-11-17T06:20:08Z / 8.2 s
- **Requested by / task or thread:** agentic-IDE-thread:phase-I2a
- **Commit / branch / PR:** bd2763f3950da51a3d069e709e8d9645877f833f / PR#1
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

Gateways view and Agent chat in headless Chromium under the shipped CSP with a fake core: bad URL refused, key never shown, Check, dev badge, two-click remove, streamed answer rendered as text (img onerror stays text), code fence, request shape, history save/search/delete.

## How it was run

npm test; npm run build; npm run ui-smoke

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 8.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm test && npm run build && npm run ui-smoke

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-i2-chat.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_062008Z_ui-smoke_bd2763f`
