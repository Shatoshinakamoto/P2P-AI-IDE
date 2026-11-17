# 2026-11-17_131135Z_live-gateway_a6170be — Live run and test after merging main (cap-std workspace) into PR #9

- **Date (UTC) / duration:** 2026-11-17T13:11:35Z / 17.8 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** a6170bed3ff3df93e976e760b51d6f3b4ca40158 / PR #9
- **Area / components under test:** live-gateway (e2e test)
- **Machine:** cloud container

## What was tested and why

Phase I5b runner::gateway_run against the real Go gateway at P2P-AI 28f06ad0, re-run after main's workspace change (#8) was merged into the branch.

## How it was run

scripts/live_gateway_e2e.py --p2p-ai <checkout> --example live_runs; sandbox image p2p-ai/sandbox:dev; scripted test-only model on loopback

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 17.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `a6170bed3ff3` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_131135Z_live-gateway_a6170be`
