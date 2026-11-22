# 2026-11-22_100316Z_desktop-node-ext_311cb8d — Real Node extension (Code Spell Checker) in the Windows desktop app with our pinned 1.145.2 server

- **Date (UTC) / duration:** 2026-11-22T10:03:16Z / 0.0 s
- **Requested by / task or thread:** not recorded
- **Commit / branch / PR:** 311cb8d / not recorded
- **Area / components under test:** desktop-node-ext (manual test)
- **Machine:** user PC (Windows)

## What was tested and why

End-to-end on the PC: the release build of the desktop app (npm run release at 311cb8d) opens a trusted folder, downloads our pinned Windows server from the public pre-release on the 'Download and start Node extensions' click, reloads, connects, and runs Code Spell Checker (streetsidesoftware.code-spell-checker from Open VSX, installed earlier).

## How it was run

Screen control on the PC with Pheonix's go-ahead (2026-11-22 09:51Z): Extensions view, Download and start Node extensions, open spell.md ('This sentense has a mispelled word.'), hover the underlined word. Server log copied from the app data folder (user name redacted).

The recorder ran the command itself and saved its full output.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

The server 1.145.2-windows-x86_64 was installed next to the old 1.140 one. The extension host started and activated git-base, git, github, debug-auto-launch, merge-conflict, streetsidesoftware.code-spell-checker, then the Markdown extensions on opening the file. No errors in the server log. Both misspelled words are underlined, and the hover reads '"sentense": Misspelled word. cSpell' with View Problem and Quick Fix.

passed 1, failed not counted, skipped not counted; exit code 0; duration 0.0 s.

## Failures and analysis

None recorded.

## Verdict: pass

The command ended with exit code 0.

## How to reproduce

Check out commit `311cb8d` and run the commands in `command.txt` in an environment like the one in `environment.md`.

## Files in this snapshot

- `RUN.md`: this description
- `artifacts/cspell-hover.jpg`: other evidence
- `artifacts/spell-squiggles.jpg`: other evidence
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/output.log`: log of the run
- `logs/remoteexthost.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-22_100316Z_desktop-node-ext_311cb8d`
