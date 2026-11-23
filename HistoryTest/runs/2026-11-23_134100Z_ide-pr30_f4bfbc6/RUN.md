# 2026-11-23_134100Z_ide-pr30_f4bfbc6 — IDE PR #30 at f4bfbc6 on the PC: core, check, UI smoke, chat recheck of the JSON retract

- **Date (UTC) / duration:** 2026-11-23T13:41:00Z / 420.0 s
- **Requested by / task or thread:** ide-tf-finish-thread
- **Commit / branch / PR:** f4bfbc6 / 30
- **Area / components under test:** ide-pr30 (integration test)
- **Machine:** user PC (Windows)

## What was tested and why

Recheck PR #30 after the retract event (raw JSON taken off the chat).

## How it was run

cargo test (core), npm run check, UI smoke via the local Chromium shim, release exe, manual Ollama gateway, qwen2.5-coder:7b with Tools on, 'read hello.py and tell me what it prints', two turns.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

core 140 passed, 4 ignored; tsc 0 errors; UI smoke 135/135; read_file ran and no raw JSON line stayed above it (2 of 2 turns).

passed 276, failed 0, skipped 4; duration 420.0 s.

## Failures and analysis

None for the fix. Model behaviour: on turn 1 qwen2.5-coder:7b looped read_file/edit_file 6 times and proposed an unasked edit, which was rejected (hello.py unchanged).

## Verdict: pass

No reason recorded.

## How to reproduce

Check out commit `f4bfbc6` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/README.md`: other evidence
- `artifacts/binaries.txt`: other evidence
- `artifacts/drivers/a/click.mjs`: other evidence
- `artifacts/drivers/a/ide-addgw.mjs`: other evidence
- `artifacts/drivers/a/ide-chat.mjs`: other evidence
- `artifacts/drivers/a/ide-chatread.mjs`: other evidence
- `artifacts/drivers/a/ide-difftext.mjs`: other evidence
- `artifacts/drivers/a/ide-openfile.mjs`: other evidence
- `artifacts/drivers/a/ide-openfolder.mjs`: other evidence
- `artifacts/drivers/a/ide-removegw.mjs`: other evidence
- `artifacts/drivers/a/ide-reviewbtns.mjs`: other evidence
- `artifacts/drivers/a/list-buttons.mjs`: other evidence
- `artifacts/drivers/launch.ps1`: other evidence
- `artifacts/drivers/log.mjs`: other evidence
- `artifacts/drivers/offscreen.ps1`: other evidence
- `artifacts/drivers/pickfolder.ps1`: other evidence
- `artifacts/drivers/step.mjs`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/ide/01-checkout.log`: log of the run
- `logs/ide/02-cargo-test-core.log`: log of the run
- `logs/ide/03-npm-run-check.log`: log of the run
- `logs/ide/04-npm-ui-smoke-shim.log`: log of the run
- `logs/ide/05-build-release.log`: log of the run
- `logs/ide/app-stderr.log`: log of the run
- `logs/ide/console.log`: log of the run
- `logs/ide/steps.md`: log of the run
- `logs/ide/times.txt`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-23_134100Z_ide-pr30_f4bfbc6`
