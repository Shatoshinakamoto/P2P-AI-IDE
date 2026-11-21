# 2026-11-21_173357Z_ovsx-cached_848b60e — Real install path on Open VSX files cached with curl

- **Date (UTC) / duration:** 2026-11-21T17:33:57Z / 0.9 s
- **Requested by / task or thread:** Finish the TandemForge and IDE roadmaps
- **Commit / branch / PR:** 848b60eea05e6f9400485da287d314e214ef32dd / claude/project-thread-8u4b8a
- **Area / components under test:** ovsx-cached (integration test)
- **Machine:** cloud container

## What was tested and why

The real ExtStore::install on files downloaded from open-vsx.org today (search, Dracula 2.25.1 with signature, material-icon-theme and redhat.vscode-yaml must be refused as Node extensions). The IDE client does not use proxies, so it cannot reach Open VSX from the cloud container; the files were fetched with curl through the proxy first.

## How it was run

cargo run --example ovsx_cached -- <cache folder>

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `848b60eea05e` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_173357Z_ovsx-cached_848b60e`
