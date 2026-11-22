# 2026-11-22_071904Z_node-host-build_87b8c86 — Windows x64 Node host server build from VS Code 1.145.2 on the PC

- **Date (UTC) / duration:** 2026-11-22T07:19:04Z / 0.0 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 87b8c86 / not recorded
- **Area / components under test:** node-host-build (build test)
- **Machine:** user PC (Windows)

## What was tested and why

scripts/build_node_host.py on the PC (native Windows x64, portable Node 24.22.1, VS 2026 with Spectre-mitigated libs). First attempt failed at packaging (signtool.exe not on PATH); this second attempt with the fix in 87b8c86 passed.

## How it was run

D:\p2p-ai-ci\node-host-build\build.cmd (checks out the branch, runs the script). Log copied from the PC.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Archive p2pide-node-host-win32-x64-1.145.2.tar.gz, 86,575,517 bytes, SHA-256 fdbef6ee11aca9e18573815783cc5e536c1275ac3544e00bce76c6c9fadfd22e; product.json commit c4453f8e, version 1.145.2, quality stable. Uploaded to pre-release node-host-1.145.2 and downloaded back anonymously from the cloud: same SHA-256.

passed 1, failed not counted, skipped not counted; exit code 0; duration 0.0 s.

## Failures and analysis

First attempt (same day): gulp vscode-reh-win32-x64-min-ci failed with 'spawn signtool.exe ENOENT'; fixed in the script.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `87b8c86` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/build-log-win32.txt`: log of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- `results/winres/p2pide-node-host-win32-x64-1.145.2.json`: result file
- `results/winres/p2pide-node-host-win32-x64-1.145.2.tar.gz.sha256`: result file

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_071904Z_node-host-build_87b8c86`
