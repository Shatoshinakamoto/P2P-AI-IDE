# Notes for 2026-11-17_140846Z_windows-containers_8bba2b4

## 2026-11-17 14:08 UTC

The '== end' time in the log equals the start time because %TIME% inside the parenthesised block is expanded once when cmd parses it; durations were not measured.

## 2026-11-17 14:09 UTC

Correction: environment.md lists 'Windows 11 PC, Docker Desktop 29.9.2 (Linux engine), Rust stable MSVC' as an environment variable name. It is not one; it was passed in the wrong field. Docker client and server 29.9.2 (server OS linux) are from the PC diagnostic D:\p2p-ai-ci\ide5d-diag.cmd; the Windows edition was not checked, so 'Windows 11' is unverified.
