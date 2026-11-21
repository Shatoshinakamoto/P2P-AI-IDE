# 2026-11-21_205630Z_tf-engine-e2e_1e284ce — Phase I7 end to end: IDE core against a real TandemForge backend (fake Ollama)

- **Date (UTC) / duration:** 2026-11-21T20:56:30Z / 8.6 s
- **Requested by / task or thread:** Finish TandemForge and IDE roadmaps thread
- **Commit / branch / PR:** 1e284ce08423ed2b2a8ebe130b7e2497edeff82e / claude/project-thread-8u4b8a
- **Area / components under test:** tf-engine-e2e (e2e test)
- **Machine:** cloud container

## What was tested and why

The IDE core finds TandemForge through engine.json, a wrong code is refused (401), and models, streamed chat, a slow reply with keep-alives, an agent step with a tool call and the step after it all work through TandemForge's /v1 API. Also: the pairing code never opens /api, /v1 needs credentials, a browser Origin is refused, only the code's digest is stored, files are 0600, revoking stops the code and removes engine.json, every chat reached only the local Ollama, the code is not in the backend log.

## How it was run

TandemForge backend from branch claude/project-thread-8u4b8a at 0a89a60 with a file database in a temp folder, TANDEMFORGE_PORT 8798, OLLAMA_BASE_URL pointing at a fake Ollama (projects/input/fake_ollama.py) on 127.0.0.1:11555; the owner connects an app with the launch token; then cargo test --test tandemforge_engine -- --ignored with TF_ENGINE_DIR and TF_CODE. Script in projects/input/run.sh. No real model was used.

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 8.6 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `1e284ce08423` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `projects/input/fake_ollama.py`: test project as it was given to the test
- `projects/input/run.sh`: test project as it was given to the test
- `results/out2/backend-excerpt.log`: result file
- `results/out2/ide-core.txt`: result file
- `results/out2/steps.txt`: result file
- `results/summary.txt`: verdict, counts, exit code and duration
- Secrets or personal details were blanked in 1 file(s) before storing.

Check this folder: `python3 scripts/history_test.py verify 2026-11-21_205630Z_tf-engine-e2e_1e284ce`
