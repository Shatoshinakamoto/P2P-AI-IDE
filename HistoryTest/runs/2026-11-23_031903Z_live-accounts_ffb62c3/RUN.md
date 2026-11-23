# 2026-11-23_031903Z_live-accounts_ffb62c3 — Account sign-in end to end: real p2p-accounts + Go gateway + Chromium

- **Date (UTC) / duration:** 2026-11-23T03:19:03Z / 1.6 s
- **Requested by / task or thread:** thread: Finish TandemForge and IDE roadmaps (A3 accounts)
- **Commit / branch / PR:** ffb62c3d5247f9bff9f1f5e2c034912a9be10971 (plus uncommitted changes) / not recorded
- **Area / components under test:** live-accounts (e2e test)
- **Machine:** cloud container

## What was tested and why

IDE core example live_accounts against the real P2P-AI account service and Go gateway (built from P2P-AI main 901474c0, Go 1.27.4 toolchain), headless Chromium signing in on the real page.

## How it was run

go build ./cmd/p2p-accounts ./cmd/gateway in P2P-AI-Gateways/Go; node scripts/live_accounts_e2e.mjs --bin-dir <bins> --chromium /opt/pw-browsers/chromium. Plain http on 127.0.0.1 only (INSECURE - DEV ONLY).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 1.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

see docs/commands/live-accounts-e2e.md

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_031903Z_live-accounts_ffb62c3`
