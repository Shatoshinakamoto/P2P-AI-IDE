# Round 4 evidence (PC, Windows 11, non-admin, PowerShell 7, Python 3.14.10). All times UTC.

* Start 2026-11-23T13:25:33Z. Windows I started were moved off-screen (-32000,-32000); helper processes hidden.
  Nothing committed or pushed. Scratch data only (scratch check-sandbox database under a deleted scratch folder).
* tf\        Part A: TandemForge PR #174 @e646b00 (logs 01-04d, sandbox\ acl-probe + cleanup, steps.md)
* ide\       Part B: IDE PR #30 @f4bfbc6 (logs 01-05, screenshots 01-05, console.log, app-stderr.log, times.txt, steps.md)
* ide-main\  Extra, owner's request: same click-through on a clean clone of IDE main @6f439af (steps.md)
* drivers\   The driver scripts used (copies; source D:\p2p-ai-ci\click-drv and round3\offscreen.ps1)
* run-log.ps1 runs one command and writes its full output, UTC start/end and exit code to a .log (LF).
* Redaction: the Windows account name is replaced by <user> in every text file. PNGs are not in the zip;
  binaries.txt lists their names and SHA-256.