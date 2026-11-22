# 2026-11-22_071929Z_node-host-real_0c3d711 — Core downloads the pinned Windows Node host from the public pre-release, checks it, starts it, token enforced

- **Date (UTC) / duration:** 2026-11-22T07:19:29Z / 0.0 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 0c3d711 / not recorded
- **Area / components under test:** node-host-real (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

core/tests/node_host_real.rs (ignored by default) on the PC: real_get downloads PINS' windows/x86_64 file from github.com, the SHA-256 check, safe unpack, start through Launcher::for_root, /version, and token check

## How it was run

D:\p2p-ai-ci\node-host-build\real.cmd at 0c3d711

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Installed 1.145.2 in 12.1 s; listening after 0.48 s; /version 200 with commit c4453f8e (the pinned SERVER_COMMIT); / without token 403, with token 404 (accepted, no page at /). 1 passed.

passed 1, failed not counted, skipped not counted; exit code 0; duration 0.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `0c3d711` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `logs/real-log-win32.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_071929Z_node-host-real_0c3d711`
