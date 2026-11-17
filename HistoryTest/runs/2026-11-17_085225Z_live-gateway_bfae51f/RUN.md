# 2026-11-17_085225Z_live-gateway_bfae51f — Live: IDE core against a real Go gateway, node and sandbox (Autopilot mission)

- **Date (UTC) / duration:** 2026-11-17T08:52:25Z / 16.0 s
- **Requested by / task or thread:** IDE phase I4 thread
- **Commit / branch / PR:** bfae51fd7ee2d8d54b562758dd4efc65aae32487 / PR #4
- **Area / components under test:** live-gateway (e2e test)
- **Machine:** cloud container

## What was tested and why

Phase I4 against the real P2P-AI stack instead of fakes: create a project, send the folder (.env kept local), Autopilot switch, start a mission with a unittest check, approvals (stale digest refused), mission finishes with the check passing in the sandbox, collect, pull into review, accept; .env value in no stored file

## How it was run

P2P-AI main 56e497b: Go gateway and node built natively, sandbox image from deploy/Dockerfile.sandbox (Docker, no network), scripted test-only model on loopback (mock-model:1b). scripts/live_gateway_e2e.py --p2p-ai <checkout> --bin-dir <prebuilt>

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 16.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `bfae51fd7ee2` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_085225Z_live-gateway_bfae51f`
