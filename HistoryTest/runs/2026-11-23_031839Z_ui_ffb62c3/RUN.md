# 2026-11-23_031839Z_ui_ffb62c3 — UI smoke incl. P2P-AI accounts panel

- **Date (UTC) / duration:** 2026-11-23T03:18:39Z / 24.0 s
- **Requested by / task or thread:** thread: Finish TandemForge and IDE roadmaps (A3 accounts)
- **Commit / branch / PR:** ffb62c3d5247f9bff9f1f5e2c034912a9be10971 (plus uncommitted changes) / not recorded
- **Area / components under test:** ui (integration test)
- **Machine:** cloud container

## What was tested and why

Built webview in headless Chromium with the fake Tauri core: every view, plus the new A3 steps (bad address refused, sign in, one-time password change with short-password refusal, link a gateway, sign out, unlink).

## How it was run

npm run build; node tests/ui-smoke.mjs with NODE_PATH to the machine's playwright.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 24.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

cd app && npm ci && npm run build && node tests/ui-smoke.mjs

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke-accounts.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_031839Z_ui_ffb62c3`
