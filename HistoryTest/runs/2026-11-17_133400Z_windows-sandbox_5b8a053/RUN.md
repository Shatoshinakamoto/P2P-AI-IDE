# 2026-11-17_133400Z_windows-sandbox_5b8a053 — First real run of the Windows local sandbox (AppContainer) on the PC at 5b8a053

- **Date (UTC) / duration:** 2026-11-17T13:34:00Z / not recorded
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** 5b8a053 / claude/project-thread-dfop2y
- **Area / components under test:** windows-sandbox (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

The new Windows sandbox on the real PC: core fmt/clippy/tests incl. local_runs_are_confined_on_windows, then real toolchains (cargo x2, go, npm, make, unittest) inside it.

## How it was run

D:\p2p-ai-ci\ide5w-test.cmd started hidden through a scheduled task; source tarball 5b8a053.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

fmt and clippy 0. Core 64 passed, 1 failed: the Windows sandbox test passed every confinement check (copy readable and writable, .env not copied, real folder untouched, file outside unreadable, write outside refused, no internet, no loopback, background program did not hold the run) but failed at the time-limit check, because ping exits at once without loopback, so it cannot be used to wait. Toolchains: cargo and go PASS; the cargo cache check FAIL (second run over 15 s); npm FAIL (MODULE_NOT_FOUND: C:\Program Files\nodejs has no entry for app containers); make SKIP (not installed); unittest FAIL (0xC0000135 DLL not found: python3 resolved to MSYS2's python3.exe).

passed 66, failed 4, skipped 1.

## Failures and analysis

Fixed in 8819a48: the time-limit and background checks use a busy-loop script instead of ping; python3 prefers python.exe; Program Files folders get a grant when allowed; the cache check compares run times and the example prints the start of stderr.

## Verdict: fail

No reason recorded.

## How to reproduce

Check out commit `5b8a053` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5w-1.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_133400Z_windows-sandbox_5b8a053`
