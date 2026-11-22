# 2026-11-22_061550Z_core_dab272f — Core tests after pinning our own Node host build

- **Date (UTC) / duration:** 2026-11-22T06:15:50Z / 11.2 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** dab272feb5a7eb4cd71eebafeccd52c514b44b59 / not recorded
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

cargo test for core: pin shape, fail-closed platforms, everything else unchanged

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 11.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `dab272feb5a7` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_061550Z_core_dab272f`
