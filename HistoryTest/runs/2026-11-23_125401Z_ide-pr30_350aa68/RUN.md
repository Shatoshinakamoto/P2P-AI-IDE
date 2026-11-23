# 2026-11-23_125401Z_ide-pr30_350aa68 — IDE PR #30 on the PC: core tests, shell check, app tests, UI smoke, release recheck

- **Date (UTC) / duration:** 2026-11-23T12:54:01Z / 900.0 s
- **Requested by / task or thread:** ide-tf-finish-thread
- **Commit / branch / PR:** 350aa68 / 30
- **Area / components under test:** ide-pr30 (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

Verify the three click-through fixes on the PC before merge.

## How it was run

cargo test (core), cargo check (app/src-tauri), npm ci, npm run check, npm test, npm run ui-smoke (via a local shim that uses the PC's existing Chromium; global playwright not installed), release exe build, then a recheck of the three bugs with a manual Ollama gateway and qwen2.5-coder:7b.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

core 140 passed, 0 failed, 4 ignored; shell check ok; tsc 0 errors; app unit 8/8; UI smoke 135/135; recheck a/b/c pass.

passed 284, failed 0, skipped 4; duration 900.0 s.

## Failures and analysis

npm run ui-smoke as written exits 1 (needs a global playwright); rerun through the shim passed. Cosmetic: the streamed JSON stayed above the tool step (fixed in 0bdf1d5, to be rechecked). Run and test still says 'link the folder in Missions' for a plain Ollama gateway.

## Verdict: pass

No reason recorded.

## How to reproduce

Check out commit `350aa68` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `artifacts/README.md`: other evidence
- `artifacts/round3-binaries.txt`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide/01-checkout.log`: log of the run
- `logs/ide/02-cargo-test-core.log`: log of the run
- `logs/ide/03-cargo-check-src-tauri.log`: log of the run
- `logs/ide/04-npm-ci.log`: log of the run
- `logs/ide/05-npm-run-check.log`: log of the run
- `logs/ide/06-npm-test.log`: log of the run
- `logs/ide/07-npm-ui-smoke.log`: log of the run
- `logs/ide/07b-npm-ui-smoke-nodepath.log`: log of the run
- `logs/ide/07c-npm-ui-smoke-shim.log`: log of the run
- `logs/ide/08-build-release.log`: log of the run
- `logs/ide/app-stderr.log`: log of the run
- `logs/ide/console.log`: log of the run
- `logs/ide/steps.md`: log of the run
- `logs/ide/times.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_125401Z_ide-pr30_350aa68`
