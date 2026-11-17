# 2026-11-17_193821Z_installers_377af6f — Licence gate after regenerating THIRD-PARTY-NOTICES.txt (I5c/I5d crates)

- **Date (UTC) / duration:** 2026-11-17T19:38:21Z / 1.9 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 377af6fe7362fd49a4d058b230f7377db4ac4191 (plus uncommitted changes) / not recorded
- **Area / components under test:** installers (scan test)
- **Machine:** cloud container

## What was tested and why

PC release build stopped: the IDE notices file was stale after the I5c/I5d sandbox crates were added. Regenerated it and re-ran the gate.

## How it was run

third_party_notices.py from P2P-AI 728f1348 --app-dir app --check, after cargo fetch --locked and npm ci

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `377af6fe7362` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_193821Z_installers_377af6f`
