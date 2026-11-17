# 2026-11-17_074553Z_windows_0a3a48f — Windows build and tests of phase I3 (f3cde42, then 0a3a48f with the CRLF fix)

- **Date (UTC) / duration:** 2026-11-17T07:45:53Z / 600.0 s
- **Requested by / task or thread:** IDE phase I3 thread
- **Commit / branch / PR:** 0a3a48f / PR #3
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

Phase I3 (chat tools and diff review) built and tested natively on the Windows 11 PC before merging PR #3, as Pheonix asked.

## How it was run

git archive tarball copied to D:\p2p-ai-ci, extracted and built by ide3b-build.cmd (artifact) launched hidden through schtasks: core fmt/clippy/tests, npm ci --ignore-scripts, npm test, vite build, npm audit, shell fmt/clippy, tauri release build. Rust stable MSVC toolchain.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Both commits: every step exit 0. Core 46 tests at f3cde42 and 47 at 0a3a48f (Windows skips 2 Unix-only tests), npm audit 0 vulnerabilities, release build 2m28s and 2m53s.

passed 47, failed 0, skipped not counted; duration 600.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

Copy the source tarball and ide3b-build.cmd to D:\p2p-ai-ci and run the .cmd; read logs\ide3b.txt.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/ide3b-build.cmd`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide3-windows-build-summary.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_074553Z_windows_0a3a48f`
