# 2026-11-22_053624Z_ui-node-host-real_4096b2e — Editor with a real Node host built from VS Code 1.145.2 (Linux x64)

- **Date (UTC) / duration:** 2026-11-22T05:36:24Z / 23.9 s
- **Requested by / task or thread:** tf-ide-roadmap-thread
- **Commit / branch / PR:** 4096b2efca0be6ac0168268c5feeb1174ce5128b (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui-node-host-real (e2e test)
- **Machine:** cloud container

## What was tested and why

The first end-to-end check of the editor against a Node extension host server of its own VS Code version: the UI smoke test with NODE_HOST_REAL forwards the editor's connection to a real server (built by scripts/build_node_host.py, installed and started through the core's own checks by core/examples/node_host_serve) and checks in the server's own extension host log that it received the workspace and started extensions.

## How it was run

Server: python3 scripts/build_node_host.py --tag 1.145.2 --commit c4453f8e... (Linux x64, Node 24.22.1), then cargo run --example node_host_serve -- --tgz <archive> --sha256 <hex> --data <tmp> --folder <tmp workspace>. Test: npm run build && NODE_HOST_REAL=<serve json> NODE_HOST_REAL_DATA=<data> npm run ui-smoke in app/ (headless Chromium).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 23.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `4096b2efca0b` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_053624Z_ui-node-host-real_4096b2e`
