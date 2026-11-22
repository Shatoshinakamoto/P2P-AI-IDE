# 2026-11-21_232948Z_security-scan_f363048 — Phase I6c-1 static checks (re-run after notices update)

- **Date (UTC) / duration:** 2026-11-21T23:29:48Z / 7.9 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** f363048a371c8d47ef6aa035014e02ead33064c5 / claude/project-thread-8u4b8a
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Same checks as the failed run before it: tsc, rustfmt, clippy, cargo audit of both lockfiles (new crates tar 0.4.46, filetime 0.2.30), npm audit gate, licence gate.

## How it was run

/tmp/claude-0/i6-static.sh from the repository root.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 5, failed 0, skipped 0 (packages); exit code 0; duration 7.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `f363048a371c` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_232948Z_security-scan_f363048`
