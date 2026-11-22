# 2026-11-22_113321Z_core_67b1744 — Core tests with the shared agent-tool cases (X2)

- **Date (UTC) / duration:** 2026-11-22T11:33:21Z / 13.0 s
- **Requested by / task or thread:** tf-ide-roadmap-finish/X2
- **Commit / branch / PR:** 67b174490e053089e4799308c3954953b31bdbbb / claude/project-thread-8u4b8a
- **Area / components under test:** core (unit test)
- **Machine:** cloud container

## What was tested and why

The IDE core after adding core/src/agent_vectors.rs, which runs core/tests/vectors/agent_tools.json (the cases the VS Code extension's TypeScript agent also runs). Checks the Rust agent still passes everything and matches the shared tool definitions, prompt, path, deny, edit, range and argument cases.

## How it was run

cargo test in core/ on the cloud container (Linux x86_64).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

passed not counted, failed not counted, skipped not counted; exit code 0; duration 13.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `67b174490e05` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_113321Z_core_67b1744`
