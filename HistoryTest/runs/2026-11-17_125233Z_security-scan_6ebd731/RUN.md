# 2026-11-17_125233Z_security-scan_6ebd731 — OSV lookup of the crates cap-std adds

- **Date (UTC) / duration:** 2026-11-17T12:52:33Z / 0.3 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 6ebd731048dc7e623d68f32e86579910ca485438 / claude/project-thread-1x3ij1
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

every crate new in core/Cargo.lock (cap-std 4.0.3 and its dependencies) against api.osv.dev

## How it was run

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 0.3 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `6ebd731048dc` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `projects/input/osv_q.json`: test project as it was given to the test
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_125233Z_security-scan_6ebd731`
