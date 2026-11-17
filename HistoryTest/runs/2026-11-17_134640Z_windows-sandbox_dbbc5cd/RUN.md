# 2026-11-17_134640Z_windows-sandbox_dbbc5cd — Windows local sandbox (AppContainer) on the PC at dbbc5cd: confinement test and real toolchains

- **Date (UTC) / duration:** 2026-11-17T13:46:40Z / not recorded
- **Requested by / task or thread:** claude
- **Commit / branch / PR:** dbbc5cd / claude/project-thread-dfop2y
- **Area / components under test:** windows-sandbox (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

The Windows sandbox on the real PC after the fixes from the first run: core fmt/clippy/tests incl. local_runs_are_confined_on_windows, then real toolchains inside it.

## How it was run

D:\p2p-ai-ci\ide5w-test.cmd started hidden through a scheduled task; source tarball dbbc5cd, SHA-256 7c3ad8ba... in the log matches the cloud copy. A run at 8819a48 between the two (core 65/65 and the same toolchain results except the old cache check) is not archived separately; this run supersedes it.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

fmt and clippy 0. Core 65 + 5 + 2 passed, including the real Windows confinement test: the copy is readable and writable, .env is not copied, the real folder is untouched, a file outside is unreadable and a write outside is refused, no internet and no loopback (curl), a background program does not hold the run, the time limit ends a busy loop, .bat/.cmd and paths are refused, no grant is left behind. Toolchains: cargo PASS (tests counted, failing test found; about 27 s per run), cargo build output in the per-folder cache PASS, go PASS, unittest PASS (python.org Python shared for the run), make SKIP (not installed), npm FAIL: C:\Program Files\nodejs has no entry for app containers and changing it needs admin rights, so node cannot read npm-cli.js.

passed 76, failed 1, skipped 1.

## Failures and analysis

npm on this PC is an environment limit, not a sandbox bug: documented in features/local-sandbox.md section 7 with the one-time admin command or a per-user Node.js install. Not changed by the IDE (it does not run as admin).

## Verdict: partial

No reason recorded.

## How to reproduce

Check out commit `dbbc5cd` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide5w-3.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_134640Z_windows-sandbox_dbbc5cd`
