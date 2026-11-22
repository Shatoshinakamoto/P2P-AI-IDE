# 2026-11-22_101242Z_security-scan_9ebb059 — cargo audit of the desktop app after url 2.5.9

- **Date (UTC) / duration:** 2026-11-22T10:12:42Z / 1.2 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 9ebb059b92c95225418379cc3ef6e2dd8082b3b8 / not recorded
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

RUSTSEC-2026-0592 found by PC local CI on PR 23; url upgraded

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 1, failed not counted, skipped not counted; exit code 0; duration 1.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `9ebb059b92c9` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_101242Z_security-scan_9ebb059`
