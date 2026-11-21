# 2026-11-21_173412Z_ui_848b60e — UI smoke test under the shipped CSP, with the new Extensions view checks

- **Date (UTC) / duration:** 2026-11-21T17:34:12Z / 5.9 s
- **Requested by / task or thread:** Finish the TandemForge and IDE roadmaps
- **Commit / branch / PR:** 848b60eea05e6f9400485da287d314e214ef32dd / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

The built webview with a fake Tauri core, in Chromium: every view, plus phase I6a: Open VSX results as text, a code extension refused, theme install with verified badge, theme switch recolours the editor, removing the theme in use falls back to Dark Modern, no CSP violations, no page errors.

## How it was run

npm run build, then node tests/ui-smoke.mjs

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 5.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `848b60eea05e` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/i6shots/chat.png`: other evidence
- `artifacts/i6shots/diff.png`: other evidence
- `artifacts/i6shots/extensions.png`: other evidence
- `artifacts/i6shots/run.png`: other evidence
- `artifacts/i6shots/ui-smoke.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_173412Z_ui_848b60e`
