# 2026-11-22_042600Z_ui_c727219 — UI smoke with a mismatched Node host

- **Date (UTC) / duration:** 2026-11-22T04:26:00Z / 25.7 s
- **Requested by / task or thread:** tf-ide-roadmap-thread
- **Commit / branch / PR:** c727219c84eb42b22b5ed2c150218906c9ada5a4 (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

Full UI smoke test (Playwright, mocked shell) after the version check: the Node host flow still works with a matching server, and a running server from another VS Code version is neither offered nor connected. The download size check now says 85 MB.

## How it was run

npm run build && npm run ui-smoke in app/ (headless Chromium).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 25.7 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `c727219c84eb` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_042600Z_ui_c727219`
