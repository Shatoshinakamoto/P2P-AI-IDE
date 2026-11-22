# 2026-11-22_121549Z_node-host-electron_01bc30e — Phase I6d: real server under Electron 43.8.3 runs an extension with an Electron-only native module

- **Date (UTC) / duration:** 2026-11-22T12:15:49Z / 35.6 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 01bc30e9ade3bdbbaa441d05b69ddde81f2f77df (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** node-host-electron (e2e test)
- **Machine:** cloud container

## What was tested and why

End to end through the core: our pinned-build VS Code 1.145.2 server (Linux x64) and Electron 43.8.3 installed by the core's checks (SHA-256, safe unpack), started by node_host::start with electron_host::launcher, the abi-probe test extension staged, the real editor connected (UI smoke with NODE_HOST_REAL). Expect: host program is the installed Electron, abi-probe activates with no error, no error in the extension host log.

## How it was run

app/tests/electron-host-e2e.sh electron with SERVER_TGZ, ELECTRON_ZIP, ELECTRON_SHA256 (the pin) and PROBE_DIR (fixture built with node-gyp 11.5.0 against Electron 43.8.3 headers).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 35.6 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121549Z_node-host-electron_01bc30e`
