# 2026-11-22_121745Z_security-scan_882c782 — Phase I6d static checks after regenerating the notices

- **Date (UTC) / duration:** 2026-11-22T12:17:45Z / 7.2 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 882c7826d65cc4a2973a045240598dde4b37b31e (plus uncommitted changes) / claude/project-thread-8u4b8a
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Same checks as the previous security-scan run, after regenerating THIRD-PARTY-NOTICES.txt for url 2.5.9.

## How it was run

/tmp/claude-0/i6-static.sh from the repository root.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed 5, failed 0, skipped 0 (packages); exit code 0; duration 7.2 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `882c7826d65c` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_121745Z_security-scan_882c782`
