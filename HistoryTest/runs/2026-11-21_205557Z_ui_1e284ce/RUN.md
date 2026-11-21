# 2026-11-21_205557Z_ui_1e284ce — Phase I7 UI smoke test with TandemForge pairing (headless Chromium, shipped CSP)

- **Date (UTC) / duration:** 2026-11-21T20:55:57Z / 6.8 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 1e284ce08423ed2b2a8ebe130b7e2497edeff82e / claude/project-thread-8u4b8a
- **Area / components under test:** ui (e2e test)
- **Machine:** cloud container

## What was tested and why

Whole UI smoke test with 4 new I7 checks: TandemForge found without a port, a wrong code refused and the field cleared, the right code pairs (listed, active, code never shown), the loopback TandemForge entry not marked INSECURE and without key or CA buttons.

## How it was run

npm run build, then node tests/ui-smoke.mjs against the built app with a fake core; screenshot ui-smoke-tf.png kept.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 6.8 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `1e284ce08423` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ui-smoke-tf.png`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_205557Z_ui_1e284ce`
