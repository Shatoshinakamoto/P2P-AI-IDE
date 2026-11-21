# 2026-11-21_231802Z_core_caf40d4 — Phase I6b internet consent: core unit tests (ext_net request checks, network grant)

- **Date (UTC) / duration:** 2026-11-21T23:18:02Z / 10.9 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** caf40d480c68b85817313924b88668170b48cb76 / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

All core library tests, with 4 new ext_net tests (public https hosts only: localhost, private, metadata, IPv6 and numeric IPv4 tricks refused; method and header allow-lists, CRLF injection refused, Cookie/Host/Proxy headers dropped; every redirect checked, at most 3; base64 bodies with size caps) and the network grant (off by default and after reinstall, web extensions only).

## How it was run

cargo test --lib in core on the cloud container.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 10.9 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `caf40d480c68` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_231802Z_core_caf40d4`
