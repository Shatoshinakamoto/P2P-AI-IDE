# 2026-11-22_053759Z_node-host-build_4096b2e — Node extension host server built from VS Code 1.145.2, Linux x64

- **Date (UTC) / duration:** 2026-11-22T05:37:59Z / not recorded
- **Requested by / task or thread:** tf-ide-roadmap-thread
- **Commit / branch / PR:** 4096b2e / claude/project-thread-8u4b8a
- **Area / components under test:** node-host-build (build test)
- **Machine:** cloud container

## What was tested and why

First build of our own Node extension host server (Pheonix's choice 2026-11-22: build our own so it always matches the editor library's VS Code version, 1.145.2 today).

## How it was run

python3 scripts/build_node_host.py --tag 1.145.2 --commit c4453f8ea430795f941c1b2ff6c9321135a74a02 --out <out> --work <work>, Node 24.22.1 (from nodejs.org, SHA-256 checked against SHASUMS256.txt), Ubuntu 24.04 x64 cloud container. Cloud-only workaround: the cloud's proxy refuses api.github.com, so @vscode/ripgrep's download cache was filled from its github.com release file before npm ci (same file the package would fetch). Earlier attempts in this session failed on that, then on the marketplace debugger extensions (api.github.com) and on the Copilot ripgrep shim; the script now leaves Copilot and those extensions out.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Archive p2pide-node-host-linux-x64-1.145.2.tar.gz, 92,923,602 bytes, SHA-256 75deb573a1841099c9b17bffe5680a6f32e244439480ab3c25579aa4de38d80e, 3,077 files unpacked (293 MB). Server product.json commit c4453f8e... matches the tag. gulp vscode-reh-linux-x64-min took about 4 min after npm ci (about 4 min). The server starts through the core and the real editor works with it: run 2026-11-22_053720Z_ui-node-host-real_832a0a7.

passed 1, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

pass: the build finished, the commit and archive checks held, and the server runs real extensions with the editor.

## How to reproduce

On Linux x64 with Node 24.22.1 and VS Code's native build deps: python3 scripts/build_node_host.py --tag 1.145.2 --commit c4453f8ea430795f941c1b2ff6c9321135a74a02 --out dist-node-host

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/build4.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/ev/p2pide-node-host-linux-x64-1.145.2.json`: result file
- `results/ev/p2pide-node-host-linux-x64-1.145.2.tar.gz.sha256`: result file
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_053759Z_node-host-build_4096b2e`
