# 2026-11-17_085509Z_installers_1b95d91 — Linux .deb/.rpm bundle of the IDE with the new bundle settings

- **Date (UTC) / duration:** 2026-11-17T08:55:09Z / 115.2 s
- **Requested by / task or thread:** ship-signed-installers
- **Commit / branch / PR:** 1b95d91ae3d957fa080b9ecdc3542ea923d49fa7 / claude/project-thread-mf0qoc
- **Area / components under test:** installers (build test)
- **Machine:** cloud container

## What was tested and why

tauri build --bundles deb,rpm on the committed tauri.conf.json; checks the .deb installs LICENSE, NOTICE and THIRD-PARTY-NOTICES.txt

## How it was run

Cloud container with libwebkit2gtk-4.1-dev librsvg2-dev, host Rust 1.97, Node 22; not the pinned builder image

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 1, failed not counted, skipped not counted; exit code 0; duration 115.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `1b95d91ae3d9` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `projects/input/bundle-linux.sh`: test project as it was given to the test
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_085509Z_installers_1b95d91`
