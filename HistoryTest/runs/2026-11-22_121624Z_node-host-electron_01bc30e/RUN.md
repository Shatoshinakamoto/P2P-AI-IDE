# 2026-11-22_121624Z_node-host-electron_01bc30e — Phase I6d control: the same extension fails on the plain Node host (expected)

- **Date (UTC) / duration:** 2026-11-22T12:16:24Z / 29.0 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 01bc30e9ade3bdbbaa441d05b69ddde81f2f77df (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** node-host-electron (e2e test)
- **Machine:** cloud container

## What was tested and why

Control for the Electron run: the same server and abi-probe extension with compatibility off (the server's own Node 24.22.1). Expected and seen: abi-probe FAILS to activate with NODE_MODULE_VERSION 148 vs 137, so the UI smoke's 'no errors' check fails. This proves the probe tests the runtime, not a fluke. Verdict pass = the expected failure happened.

## How it was run

app/tests/electron-host-e2e.sh node.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 0, failed 2, skipped 0 (packages); exit code 0; duration 29.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `01bc30e9ade3` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121624Z_node-host-electron_01bc30e`
