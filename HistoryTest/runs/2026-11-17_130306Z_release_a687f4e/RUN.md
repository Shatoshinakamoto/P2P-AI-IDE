# 2026-11-17_130306Z_release_a687f4e — IDE PR 6 (installers) with main merged: config parses, merge has no conflict markers

- **Date (UTC) / duration:** 2026-11-17T13:03:06Z / 0.0 s
- **Requested by / task or thread:** Finish housekeeping and leftover work (merger)
- **Commit / branch / PR:** a687f4e06e88f6a0789cdbc5c72a07be42bd3e49 / 6
- **Area / components under test:** release (manual test)
- **Machine:** cloud container

## What was tested and why

After merging main into the installers PR: tauri.conf.json parses as JSON and no file holds a conflict marker. The PR's own build evidence is in its earlier runs.

## How it was run

python json.load + git grep for conflict markers

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `a687f4e06e88` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_130306Z_release_a687f4e`
