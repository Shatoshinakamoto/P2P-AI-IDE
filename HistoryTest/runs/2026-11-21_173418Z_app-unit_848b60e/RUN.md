# 2026-11-21_173418Z_app-unit_848b60e — App unit tests (paths, language ids)

- **Date (UTC) / duration:** 2026-11-21T17:34:18Z / 0.2 s
- **Requested by / task or thread:** Finish the TandemForge and IDE roadmaps
- **Commit / branch / PR:** 848b60eea05e6f9400485da287d314e214ef32dd / claude/project-thread-8u4b8a
- **Area / components under test:** app-unit (unit test)
- **Machine:** cloud container

## What was tested and why

languageFor now returns VS Code language ids (typescriptreact, shellscript).

## How it was run

npm test

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 4, failed 0, skipped 0 (packages); exit code 0; duration 0.2 s.

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

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_173418Z_app-unit_848b60e`
