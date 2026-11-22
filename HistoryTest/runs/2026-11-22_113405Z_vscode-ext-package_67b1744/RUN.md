# 2026-11-22_113405Z_vscode-ext-package_67b1744 — Extension .vsix package 0.2.0 (X2)

- **Date (UTC) / duration:** 2026-11-22T11:34:05Z / 0.9 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X2
- **Commit / branch / PR:** 67b174490e053089e4799308c3954953b31bdbbb / claude/project-thread-8u4b8a
- **Area / components under test:** vscode-ext-package (build test)
- **Machine:** cloud container

## What was tested and why

Builds the installable .vsix, to check the package still holds only out/, media/ and the metadata (no sources, tests or node_modules).

## How it was run

npm run package, then list the .vsix contents and take its SHA-256.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `67b174490e05` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_113405Z_vscode-ext-package_67b1744`
