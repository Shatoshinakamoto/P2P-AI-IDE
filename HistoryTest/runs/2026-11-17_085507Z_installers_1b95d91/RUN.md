# 2026-11-17_085507Z_installers_1b95d91 — Licence gate: IDE

- **Date (UTC) / duration:** 2026-11-17T08:55:07Z / 2.5 s
- **Requested by / task or thread:** ship-signed-installers
- **Commit / branch / PR:** 1b95d91ae3d957fa080b9ecdc3542ea923d49fa7 / claude/project-thread-mf0qoc (test harness at 13b1bc5)
- **Area / components under test:** installers (scan test)
- **Machine:** cloud container

## What was tested and why

Deny-by-default licence allow-list over 467 shipped crates and npm packages; notices file current

## How it was run

npm ci + cargo fetch --locked, then P2P-AI scripts/third_party_notices.py --check (P2P-AI 13b1bc5)

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 1, failed not counted, skipped not counted; exit code 0; duration 2.5 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `1b95d91ae3d9` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_085507Z_installers_1b95d91`
