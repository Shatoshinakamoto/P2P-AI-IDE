# 2026-11-17_125934Z_ui-smoke_618354d — Webview unit tests, build and UI smoke 75 checks incl. Run and test and the approval card

- **Date (UTC) / duration:** 2026-11-17T12:59:34Z / 9.9 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 618354d66538be58c2fcae53f060caf7654b5fb3 / claude/project-thread-dfop2y
- **Area / components under test:** ui-smoke (e2e test)
- **Machine:** cloud container

## What was tested and why

Webview against a fake IPC: Run and test view (runner list, test run result as text, command as words), chat Runs switch off by default, approval card text-only, nothing before Allow, Allow and Decline answer the request id.

## How it was run

npm test, npm run build, npm run ui-smoke (Chromium, shipped CSP)

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 9.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `618354d66538` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_125934Z_ui-smoke_618354d`
