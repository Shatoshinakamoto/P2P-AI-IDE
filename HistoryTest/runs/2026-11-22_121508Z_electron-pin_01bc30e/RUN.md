# 2026-11-22_121508Z_electron-pin_01bc30e — Phase I6d: Electron 43.8.3 pins equal our own downloads and Electron's SHASUMS256.txt

- **Date (UTC) / duration:** 2026-11-22T12:15:08Z / 0.2 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 01bc30e9ade3bdbbaa441d05b69ddde81f2f77df / claude/project-thread-8u4b8a
- **Area / components under test:** electron-pin (manual test)
- **Machine:** cloud container

## What was tested and why

The pinned SHA-256 values in core/src/electron_host.rs (Linux x64, Windows x64) must equal the hash of the zips downloaded from Electron's GitHub release v43.8.3 and the lines in that release's SHASUMS256.txt. VS Code 1.145.2's .npmrc names Electron 43.8.3.

## How it was run

Downloaded SHASUMS256.txt and both zips over https from github.com/electron/electron/releases/download/v43.8.3/, then sha256sum and grep.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `01bc30e9ade3` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/SHASUMS256.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121508Z_electron-pin_01bc30e`
