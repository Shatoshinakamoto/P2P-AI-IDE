# 2026-11-21_173522Z_security-scan_848b60e — Phase I6a static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices

- **Date (UTC) / duration:** 2026-11-21T17:35:22Z / 6.9 s
- **Requested by / task or thread:** Finish the TandemForge and IDE roadmaps
- **Commit / branch / PR:** 848b60eea05e6f9400485da287d314e214ef32dd / claude/project-thread-8u4b8a
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Type check, formatting and lints for the core and the shell, RustSec scan of both lockfiles (new crates zip 9.8.1 and flate2 1.1.11), npm audit gate, and the licence gate with the jschardet stand-in. The notices script came from P2P-AI local commit 5a9580c4 (skip the app's own stand-in folder).

## How it was run

bash i6-static.sh (copied to projects/input)

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

No vulnerabilities in either lockfile. cargo audit warnings (allowed, all present before I6a, none in zip or flate2): chrono, proc-macro-error, zbus_names, zvariant, zvariant_derive unmaintained; glib 0.18 unsound iterator (GTK stack of Tauri on Linux). npm gate passed; it notes the dated picomatch exception is no longer needed. 527 packages, all licences allowed.

passed 5, failed 0, skipped 0 (packages); exit code 0; duration 6.9 s.

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
- `projects/input/i6-static.sh`: test project as it was given to the test
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_173522Z_security-scan_848b60e`
