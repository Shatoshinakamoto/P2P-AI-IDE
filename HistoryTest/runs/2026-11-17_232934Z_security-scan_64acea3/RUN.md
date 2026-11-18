# 2026-11-17_232934Z_security-scan_64acea3 — npm audit for GHSA-fc5q-39xc-vjpf (picomatch ReDoS)

- **Date (UTC) / duration:** 2026-11-17T23:29:34Z / not recorded
- **Requested by / task or thread:** picomatch ReDoS thread
- **Commit / branch / PR:** 64acea317b35bcaac6645d1675eafa3b1c276a0a (plus uncommitted changes) / claude/project-thread-jm1j5r
- **Area / components under test:** security-scan (scan test)
- **Machine:** cloud container

## What was tested and why

Checks whether a fixed picomatch exists and whether the advisory reaches shipped code.

## How it was run

npm view picomatch version; npm audit --audit-level=high and npm audit --omit=dev --audit-level=high in app/.

The run was made elsewhere; its raw logs were copied in afterwards.

Commands: see `command.txt`. Environment: see `environment.md`.

## Results

Newest picomatch is 4.0.7 (affected). Full audit: 4 high, all from GHSA-fc5q-39xc-vjpf via vite/tinyglobby/fdir (exit 1). Shipped deps only: 0 vulnerabilities (exit 0).

passed not counted, failed not counted, skipped not counted.

## Failures and analysis

Full audit fails on a dev-only advisory with no fixed version; recorded as an accepted risk.

## Verdict: partial

Expected result; the advisory is dev-only and unfixed.

## How to reproduce

cd app && npm ci --ignore-scripts && npm audit --audit-level=high && npm audit --omit=dev --audit-level=high

## Files in this snapshot

- `RUN.md`: this description
- `command.txt`: the exact commands that were run
- `environment.md`: machine, OS, tools, models and settings of the run
- `logs/audit-ide.log`: log of the run
- `manifest.json`: run facts and the SHA-256 of every file in this folder
- `results/summary.txt`: verdict, counts, exit code and duration

Check this folder: `python3 scripts/history_test.py verify 2026-11-17_232934Z_security-scan_64acea3`
