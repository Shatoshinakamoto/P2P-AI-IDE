# 2026-11-23_114348Z_live-nexus_dc07c54 — Live NEXUS Engine E2E against real nexus-proxy and nexus-runtime

- **Date (UTC) / duration:** 2026-11-23T11:43:48Z / 3.0 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** dc07c54892d791b3739a3fa897566a292ca8d58a (plus uncommitted changes) / not recorded
- **Area / components under test:** live-nexus (e2e test)
- **Machine:** cloud container

## What was tested and why

Runs core/tests/nexus_engine.rs against compiled nexus-runtime (serve) and nexus-proxy (--mock-ollama and live tool-forwarding OpenAI upstream).

## How it was run

scripts/live_nexus_e2e.py --nexus-engine /home/[REDACTED:username]/Shatoshinakamoto/nexus-engine

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 3.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `dc07c54892d7` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_114348Z_live-nexus_dc07c54`
