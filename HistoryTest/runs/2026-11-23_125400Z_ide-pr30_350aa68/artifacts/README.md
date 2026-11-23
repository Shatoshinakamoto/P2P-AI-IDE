# Round 3 evidence (PC, Windows 11, non-admin, PowerShell 7). All times UTC.

* Start 2026-11-23T12:54:59Z. No game running (no steamapps process); windows I started were moved off-screen
  (-32000,-32000) right after they appeared, helper consoles hidden. Nothing committed or pushed.
* ide\      Part A: IDE PR #30 @350aa68 (logs 01-08, screenshots 01-14, steps.md, times.txt)
* tf\       Part B: TandemForge PR #174 @948951d (logs 01-05, screenshots 10-13, steps.md, times.txt)
* sandbox\  Part B4: Windows check-sandbox ACL diagnosis (RESULT.md, acl-probe.py, acl-probe.json, acl-probe.log)
* run-log.ps1 (runs a command, logs output, UTC start/end, exit code); offscreen.ps1 (moves a window off-screen).
  The other driver scripts are the same as in click-out (click-drv: launch.ps1, launch-tf-backend.ps1, step.mjs,
  log.mjs, pickfolder.ps1, a\*.mjs).
* Redaction: the Windows account name is replaced by <user> in every text file.
zip 2026-11-23T13:13:48Z
