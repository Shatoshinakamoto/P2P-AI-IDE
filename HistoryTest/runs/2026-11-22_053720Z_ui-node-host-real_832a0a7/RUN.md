# 2026-11-22_053720Z_ui-node-host-real_832a0a7 — Editor with a real Node host built from VS Code 1.145.2 (Linux x64), fresh logs only

- **Date (UTC) / duration:** 2026-11-22T05:37:20Z / 27.4 s
- **Requested by / task or thread:** tf-ide-roadmap-thread
- **Commit / branch / PR:** 832a0a77f17200af72f63a087de48876d0085e3c (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** ui-node-host-real (e2e test)
- **Machine:** cloud container

## What was tested and why

End-to-end check of the editor against a Node extension host server of its own VS Code version: the UI smoke test with NODE_HOST_REAL forwards the editor's connection to a real server (built by scripts/build_node_host.py, installed and started through the core's checks by core/examples/node_host_serve) and checks in the extension host logs created during this run that the server received the workspace, started extensions and logged no error.

## How it was run

Server: python3 scripts/build_node_host.py --tag 1.145.2 --commit c4453f8ea430795f941c1b2ff6c9321135a74a02 (Linux x64, Node 24.22.1); cargo run --example node_host_serve -- --tgz <archive> --sha256 <hex> --data <tmp> --folder <tmp workspace>. Test: npm run build && NODE_HOST_REAL=<serve json> NODE_HOST_REAL_DATA=<data> npm run ui-smoke in app/.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 27.4 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `832a0a77f172` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_053720Z_ui-node-host-real_832a0a7`
