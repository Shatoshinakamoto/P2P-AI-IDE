# 2026-11-17_085943Z_windows_bfae51f — Windows build and tests of phase I4 (bfae51f)

- **Date (UTC) / duration:** 2026-11-17T08:59:43Z / not recorded
- **Requested by / task or thread:** IDE phase I4 thread
- **Commit / branch / PR:** bfae51f / PR #4
- **Area / components under test:** windows (build test)
- **Machine:** user PC (Windows)

## What was tested and why

Phase I4 (missions) built and tested natively on the Windows 11 PC, as Pheonix asked before merging PR #4.

## How it was run

git archive tarball of bfae51f copied to D:\p2p-ai-ci, extracted and built by ide4-build.cmd (artifact) launched hidden through schtasks: core fmt/clippy/tests, npm ci --ignore-scripts, npm test, vite build, npm audit, shell fmt/clippy, tauri release build. Rust stable MSVC toolchain.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Every step exit 0. Core 51 unit + 5 discovery + 2 projects tests (Windows skips 2 Unix-only tests), npm audit 0 vulnerabilities, release p2p-ide.exe built.

passed 58, failed 0, skipped not counted.

## Failures and analysis

None recorded.

## Verdict: pass

No reason recorded.

## How to reproduce

Copy the source tarball and ide4-build.cmd to D:\p2p-ai-ci and run the .cmd; read logs\ide4.txt.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `artifacts/ide4-build.cmd.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide4-windows-build.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_085943Z_windows_bfae51f`
