# 2026-11-22_042952Z_node-host_c727219 — Real Node extension in the desktop app on the PC: 3 bugs fixed, version mismatch found, version check verified

- **Date (UTC) / duration:** 2026-11-22T04:29:52Z / not recorded
- **Requested by / task or thread:** tf-ide-roadmap-thread
- **Commit / branch / PR:** c727219 / claude/project-thread-8u4b8a
- **Area / components under test:** node-host (manual test)
- **Machine:** user PC (Windows)

## What was tested and why

Phase I6c-3 click-through in the real desktop app on the Windows PC with Code Spell Checker 4.10.0 (Open VSX) in a trusted folder, to see a real Node extension run in the pinned VSCodium 1.140.06880 server.

## How it was run

Release builds of the IDE from branch claude/project-thread-8u4b8a (4a24e3c, 7cade19, then c727219) on the PC; folder D:\p2p-ai-ci\i6c3-ws trusted in the Extensions view, then Start Node extensions. For the diagnosis only, a debug build added --log trace to the server and exposed the editor services on window.__dbg, read over the WebView2 debug port on 127.0.0.1 (input/debug-hook.patch.txt). VS Code's proxy identifier lists were compared for the server bundle, the editor library and VS Code tags 1.140.0 to 1.145.2 (raw extHost.protocol.ts from GitHub).

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Three PC bugs found and fixed (results/pc-bugs-fixed.txt). With them fixed the server starts cleanly and the editor lists the extensions, but none activates: the editor library is VS Code 1.145.2 and the server 1.140; VS Code's message numbering changed in 1.144, so the editor's calls reach the wrong actors (results/editor-probes.txt, results/proxy-identifier-diff.txt). With c727219 the real app's panel says Node extensions can't run with this version (VS Code 1.145.2 editor vs 1.140.0 host) and offers no start button (artifacts/node-panel-version-mismatch.jpg).

passed 4, failed 1, skipped not counted.

## Failures and analysis

Product fault, not test setup: the pinned server's VS Code minor version does not match the editor library's. No VSCodium release matches 1.145 (newest 1.143.17232). Not fixable by code in this PR; Pheonix to choose how to get a matching server.

## Verdict: partial

partial: the 3 bugs are fixed and the new version check works in the real app, but no Node extension ran.

## How to reproduce

Build the IDE at c727219 on Windows, open a local folder, install Code Spell Checker, trust the folder in Extensions > Node extensions: the panel shows the version message. To see the silent hang, build 7cade19 instead and start the host: remoteexthost.log stops after 'Lock acquired'.

## Files in this snapshot

- `NOTES.md`: dated notes and corrections added to this run
- `RUN.md`: this description
- `artifacts/node-panel-version-mismatch.jpg`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/server-logs-excerpt.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `projects/input/debug-hook.patch.txt`: test project as it was given to the test
- `projects/input/i6c3-act-probe.js`: test project as it was given to the test
- `projects/input/i6c3-eval.mjs`: test project as it was given to the test
- `results/results/editor-1.145.2-proxy-ids.txt`: result file
- `results/results/editor-probes.txt`: result file
- `results/results/pc-bugs-fixed.txt`: result file
- `results/results/proxy-identifier-diff.txt`: result file
- `results/results/server-1.140.06880-proxy-ids.txt`: result file
- `results/results/vscode-tags-proxy-id-lists.txt`: result file
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_042952Z_node-host_c727219`
