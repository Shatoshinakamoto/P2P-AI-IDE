# 2026-11-17_074610Z_windows_0a3a48f — Windows click-through of I2 keychain and I3 chat tools against a local model

- **Date (UTC) / duration:** 2026-11-17T07:46:10Z / not recorded
- **Requested by / task or thread:** IDE phase I3 thread
- **Commit / branch / PR:** 0a3a48f / PR #3
- **Area / components under test:** windows (manual test)
- **Machine:** user PC (Windows)

## What was tested and why

Real-window check of the gateway key in Windows Credential Manager (I2) and of chat tools, .env denial, Stop and diff review with a real local model (I3).

## How it was run

Release build at f3cde42, then 0a3a48f. Computer use on the PC while Pheonix was away. Backend: local Ollama as a dev gateway (not a P2P-AI gateway). Models: qwen2.5-coder:7b, qwen3-coder:30b, qwen2.5:14b.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Keychain: pass. Tools: pass after the CRLF fix (bug found and fixed in 0a3a48f). Stop and diff review: pass. Accept wrote the file with CRLF kept, observed afterwards on disk. Reject, Tools off and history after switching views were not repeated on Windows.

passed not counted, failed not counted, skipped not counted.

## Failures and analysis

f3cde42: edit_file never matched CRLF files (fixed in 0a3a48f). qwen3-coder:30b got HTTP 500 from Ollama (environment, not the app).

## Verdict: partial

Reject, Tools off and history checks were not run on Windows; the Accept click was not observed directly.

## How to reproduce

Check out commit `0a3a48f` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/01-qwen25coder-text-calls.jpg`: other evidence
- `artifacts/02-qwen3coder-http500.jpg`: other evidence
- `artifacts/03-qwen25-14b-crlf-edit-fails.jpg`: other evidence
- `artifacts/05-stopped-changes-to-review.jpg`: other evidence
- `artifacts/06-diff-review.jpg`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/clickthrough-notes.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_074610Z_windows_0a3a48f`
