# `scripts/npm_audit_gate.py` and `app/npm-audit-exceptions.json`: the npm audit gate

## Overview
CI (`.github/workflows/ci.yml`) and the PC's local CI (P2P-AI `scripts/local-ci.json`) run this gate instead of a
plain `npm audit --audit-level=high`. It stays as strict as before, with one exception: a **named advisory that only
reaches dev dependencies, has no fixed version, and is listed with an expiry date** does not block. The first entry is
GHSA-fc5q-39xc-vjpf (picomatch ReDoS, 2026-11-17). The owner chose this option over `--omit=dev` or leaving the gate
red.

## How it works
1. `npm audit --omit=dev --json`: any high or critical advisory in **shipped** dependencies fails. Exceptions never
   apply here.
2. `npm audit --json`: any high or critical advisory in dev dependencies fails, unless its exact GHSA id and package
   name are listed, unexpired, in `app/npm-audit-exceptions.json`.
3. An exception that has expired, is malformed, lasts more than 120 days, or is listed twice fails the gate
   (fail closed). An exception that is no longer needed prints a note telling you to remove it.

Synopsis: `python3 -I scripts/npm_audit_gate.py <package folder>` (here `app`). Privileges: an ordinary user. It needs
the network, to reach the npm registry's audit service. Environment variables: none of its own (npm's proxy and
registry settings apply). Exit codes: `0` passed, `1` failed (advisory, or a bad/expired exception), `2` usage error,
or npm missing or could not run. Output: one line per advisory (`FAIL ...`, `excepted (dev only) ...`, `note: ...`)
and a final `npm audit gate: passed|FAILED`.

| Argument | Type | Default | What it does |
|---|---|---|---|
| `folder` (positional, required) | path | none | package folder with `package.json` and `package-lock.json` |
| `-h`, `--help` | flag | | prints usage |

Examples:
```
python3 -I scripts/npm_audit_gate.py app                                 # the gate CI runs
python3 -I -m unittest discover -s scripts -p test_npm_audit_gate.py   # its own tests (no network)
cd app && npm audit --omit=dev --audit-level=high                     # just the shipped part, by hand
```

## The exceptions file
`app/npm-audit-exceptions.json` (JSON, read only by this script). Keys: `comment` (optional string) and `exceptions` (list). Every entry
needs exactly these keys:

| Key | Type | Allowed values | Meaning |
|---|---|---|---|
| `id` | string | one GHSA id (`GHSA-xxxx-xxxx-xxxx`), no wildcards | the advisory let through |
| `package` | string | an npm package name | the vulnerable package; the id only counts for this package |
| `reason` | string | non-empty | why it is safe (dev only, how it is reached) |
| `requested_by` | string | non-empty, no personal data | who decided, and where |
| `added` | date | `YYYY-MM-DD` | the day it was added |
| `expires` | date | `added` to `added` + 120 days | after this day the gate fails until someone reviews it |
| `review` | string | non-empty | what to check at review (for example, is there a fixed version) |

Current entries:

| id | package | why | requested by | added | expires |
|---|---|---|---|---|---|
| GHSA-fc5q-39xc-vjpf | picomatch | ReDoS (CVE-2026-112366) with no fixed release; reached only through build and lint tools, with globs from our own config | project owner, via the picomatch ReDoS thread | 2026-11-17 | 2026-12-31 |

Changing entries: add or renew one only in a pull request that also adds a `docs/SECURITY-LOG.md` entry, and get the
project owner's approval. Remove an entry as soon as a fixed version is in the lock file. Misconfiguration risk: an
entry for an advisory that reaches shipped code has no effect (step 1 still fails). The real risk is renewing an entry
for ever without looking, which the 120-day limit and the `review` text are there to prevent.

## How it is secure
- Shipped code is never excepted (NIST SI-2, RA-5; OWASP A06:2021).
- Exact advisory ids bound to one package, short-lived, and reviewed in a PR (NIST CM-3, SSDF RV.2).
- Fail closed: npm errors, an old report format or bad JSON fail the gate (CWE-20).
- npm is started with a fixed argument list, with no shell (CWE-78), and a 300 s timeout (CWE-400).
- The gate runs from the checkout under test, like every other CI step, so a PR can change it. Changes to the script
  or the exceptions file show up in the PR diff and need review.

## Dependencies
Python 3.9+ standard library only; npm 7 or later (audit report v2). No new packages.

## Limitations and troubleshooting
- `npm audit reported an error`: no network, or a registry or proxy problem. Re-run when the registry is reachable.
- `... expired on ...`: look up the advisory again (`npm view <package> version`, the GHSA page). Then upgrade, or renew
  the entry with a new security-log entry.
- An advisory with no GHSA id is reported by its URL and cannot be excepted.

## Test evidence
[HistoryTest/runs/2026-11-17_235209Z_security-scan_dc6d647](../../HistoryTest/runs/2026-11-17_235209Z_security-scan_dc6d647/RUN.md): 14 unit tests pass, live gate passes on `app/`.
