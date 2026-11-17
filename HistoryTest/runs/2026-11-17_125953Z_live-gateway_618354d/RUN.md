# 2026-11-17_125953Z_live-gateway_618354d — Live run and test against a real P2P-AI gateway, node and sandbox (live_runs)

- **Date (UTC) / duration:** 2026-11-17T12:59:53Z / 16.4 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 618354d66538be58c2fcae53f060caf7654b5fb3 / claude/project-thread-dfop2y
- **Area / components under test:** live-gateway (e2e test)
- **Machine:** cloud container

## What was tested and why

Phase I5b runner::gateway_run against the real Go gateway at P2P-AI 28f06ad0: stages, unittest with a failing test named, local fix reaches the node copy, command as argument list, no network in the sandbox, time limit, refused folder, .env never stored.

## How it was run

P2P-AI main 28f06ad0: Go gateway and node built natively, sandbox image p2p-ai/sandbox:dev (Docker, no network), scripted test-only model on loopback. scripts/live_gateway_e2e.py --p2p-ai <checkout> --example live_runs

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 16.4 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_125953Z_live-gateway_618354d`
