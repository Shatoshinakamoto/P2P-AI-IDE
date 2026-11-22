# 2026-11-22_121508Z_core_01bc30e — Phase I6d core: unit tests with the Electron compatibility host

- **Date (UTC) / duration:** 2026-11-22T12:15:08Z / 13.5 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 01bc30e9ade3bdbbaa441d05b69ddde81f2f77df / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core tests, with 7 new electron_host tests: pins well formed and only for platforms with a server pin, bad hash or host refused; download hash check; unzip keeps files, skips the set-uid chrome-sandbox, owner-only modes, refuses .., drive paths and links; install once and only with the program present; switch off by default and only 'on' means on; launcher adds only ELECTRON_RUN_AS_NODE=1 and the scrubbed environment drops NODE_OPTIONS and ELECTRON_*; native module classification and the capped, link-free scan.

## How it was run

cargo test in core on the cloud container.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 13.5 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121508Z_core_01bc30e`
