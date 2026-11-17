# `scripts/history_test.py` - record and check test runs in `HistoryTest/`

**Purpose.** Keeps the test history of the repository. Every test run gets one folder under `HistoryTest/runs/` with a description (`RUN.md`), the commands, the environment, results, full logs, the test project before and after, and a checksum list. The script packs such a folder, keeps `HistoryTest/INDEX.csv` and the table in `HistoryTest/README.md`, and checks the folders. Rules and folder layout: [HistoryTest/README.md](../../HistoryTest/README.md) and `CLAUDE.md` Section 9.

This is repository tooling for the development record. It is not a product feature: no gateway, node or web code reads `HistoryTest/`, and the folder is excluded from image builds (root `.dockerignore`).

**Synopsis.**

```
python3 scripts/history_test.py [--dir DIR] run        OPTIONS -- COMMAND [ARG...]
python3 scripts/history_test.py [--dir DIR] add        OPTIONS --verdict V
python3 scripts/history_test.py [--dir DIR] note       RUN_ID --text TEXT
python3 scripts/history_test.py [--dir DIR] invalidate RUN_ID --reason TEXT
python3 scripts/history_test.py [--dir DIR] index
python3 scripts/history_test.py [--dir DIR] verify     [RUN_ID...] [--base REF] [--no-gitleaks] [--quiet]
```

**Privileges.** An ordinary user. No root, no network. `run` starts the command you give it with your own rights; `verify --base` and the commit lookup call `git` read-only; `verify` calls `gitleaks` when it is installed. Python 3.9 or newer, standard library only.

## Commands

| Command | What it does |
|---|---|
| `run` | Runs COMMAND (an argument list, no shell), saves its whole output to `logs/output.log`, records exit code, duration and, when the log is Python `unittest` or `go test` output, the pass/fail/skip counts. Verdict `pass` when the exit code is 0, else `fail`. Record type **captured**. |
| `add` | Records a run made elsewhere. With `--log` the record type is **imported**; without logs it is **reported** and needs `--results-text` or `--results`. |
| `note` | Appends a dated note or correction to the run's `NOTES.md`. |
| `invalidate` | Appends a dated reason to the run's `INVALID.md` and shows the run as `invalid` in the index. No recorded file is touched. |
| `index` | Rebuilds `INDEX.csv` and the README table from the run folders. Use it after a merge conflict in either file. |
| `verify` | Checks every run (or the given ids): checksums, missing and added files, symlinks, secret patterns in stored files, the link to the run before, and, for a full check, that `INDEX.csv` and the README table match. |

## Options

Global: `--dir DIR` (path; default `HistoryTest/` of this repository): the archive to work on.

Options of `run` and `add`:

| Option | Type, default | What it does |
|---|---|---|
| `--area` | name, required; lower case letters, digits, `.`, `-`, `_`, max 40 | Short tag of what was tested (`gateway`, `node-go`, `web`, `autopilot`, `toolkits`, `sandbox`, `release`, `security-scan`, ...). Part of the folder name. |
| `--title` | text, default the area | One line saying what the run was. Shown in the index. |
| `--kind` | `unit`, `integration`, `e2e`, `eval`, `scan`, `build`, `manual`, `other` (default) | Kind of test. |
| `--machine` | `cloud` (default), `pc`, `ci`, `other` | Where it ran: cloud container, user PC (Windows), CI runner. |
| `--commit` | hex commit id, default `HEAD` of this checkout | Commit under test. Without it the record also notes uncommitted changes. |
| `--harness-commit` | hex commit id | Commit of the test scripts when it differs from the commit under test. |
| `--branch-or-pr` | text | Branch name or pull request number. |
| `--by` | text | Task or thread that asked for the run. Do not put a person's name here. |
| `--what`, `--how`, `--results-text`, `--failures`, `--reason`, `--reproduce` | text | The sections of `RUN.md`: what was tested and why; steps, configuration, models, data; results in words; failures and analysis (product fault or test-setup fault); the reason for the verdict; how to reproduce. |
| `--note` | text | First entry of `NOTES.md` (follow-ups, links to related runs). |
| `--passed`, `--failed`, `--skipped` | integer | Counts for the index. They override what `run` reads from the log. |
| `--env` | `name=value`, repeatable | A line of `environment.md`: tool versions, hardware, model names and digests, image digests, settings. |
| `--env-name` | text, repeatable | Name of an environment variable that was set. The value is never stored. |
| `--log` | file or folder, repeatable | Copied into `logs/`. |
| `--results` | file or folder, repeatable | Machine-readable results (JUnit XML, JSON, SARIF, scorecards) for `results/`. |
| `--input` | folder, repeatable | The test project as given to the test, for `projects/input/`. |
| `--output` | folder, repeatable | The finished project, for `projects/output/`. With both, `projects/diff.patch` is written. |
| `--artifact` | file or folder, repeatable | Other evidence for `artifacts/`. |
| `--redact` | text, repeatable | Extra word to blank everywhere, for example the machine name. Replaced by `[REDACTED:hostname]`. |
| `--max-file-mb` | number, default 50 | Largest stored file. A longer log keeps its first and last part and the checksum of the original; other files are left out and listed. |
| `--max-run-mb` | number, default 200 | Largest stored run. Further files are left out and listed. |

Only `run`: `--cwd DIR` (folder to run in, default the current one), `--timeout SECONDS` (default 3600; the command is stopped and the exit code recorded as 124), `--strict` (exit 1 when the recorded command failed, for use in scripts), `--verdict` (override; default from the exit code).

Only `add`: `--verdict pass|fail|partial|invalid` (required; `invalid` needs `--reason`), `--when 2026-11-11T08:45:00Z` (start of the run in UTC, default now), `--duration SECONDS`, `--command-text TEXT` (the commands that were run), `--source-path TEXT` (where the logs were collected from).

`verify`: `--base REF` (Git ref, for example `origin/main`: fail when a file of a run that exists there was changed or deleted; `NOTES.md` and `INVALID.md` may be added or extended), `--no-gitleaks` (skip gitleaks even when installed), `--quiet` (print problems only).

**Security of the options.** `run` executes the command you pass: only wrap commands you would run anyway. `--redact`, the text options and the copied files are scrubbed, but redaction is best effort (see below). `--max-file-mb` and `--max-run-mb` above the defaults put large files into Git history for good. `--no-gitleaks` removes a check; the built-in scan still runs. There is no option that turns redaction or the copy rules off.

## Environment, exit codes, output

- Environment variables read: none. `PATH` is used to find `git` and `gitleaks`.
- Exit codes: `0` done (for `run`: the run was recorded, whatever its result); `1` `verify` found a problem, or `run --strict` and the command failed; `2` bad usage, or the command could not be started.
- Output: `run` and `add` print the new folder. `verify` prints `ok` or `FAIL` per run with the reasons, a line about gitleaks, and a summary.

## Examples

```bash
# Basic: run the Go gateway tests and record them.
python3 scripts/history_test.py run --area gateway --kind unit --title "Go gateway unit tests" \
    --what "All packages of the Go gateway after the change to X" --by "Harden gateway thread" \
    --cwd P2P-AI-Gateways/Go --env "go=$(go env GOVERSION)" -- go test ./...

# Common: record a PC batch run from its logs, with the test project before and after.
python3 scripts/history_test.py add --area autopilot --kind e2e --machine pc --verdict pass \
    --commit fdca8cb --when 2026-11-10T21:14:00Z --duration 1840 --title "Autopilot end to end, small model" \
    --log ./ap9-logs --source-path "test PC, CI logs folder" --input ./before --output ./after \
    --env model=qwen2.5-coder:7b --env "docker=Docker Desktop" --redact MYPCNAME --passed 23 --failed 0 --skipped 0

# A result known only from a written report (no raw logs): recorded as "reported".
python3 scripts/history_test.py add --area release --machine pc --verdict partial --commit 95183f5 \
    --title "Release batch 3" --results-text "59 of 59 smoke checks passed; docker-p2p stalled on a model pull" \
    --reason "one suite did not finish"

# Later: the run turned out to have used a stale image.
python3 scripts/history_test.py invalidate 2026-11-10_211400Z_autopilot_fdca8cb --reason "ran against a stale node image"

# Before committing: check everything, and that no earlier run was changed.
python3 scripts/history_test.py verify --base origin/main
gitleaks detect --no-git --source HistoryTest/runs/<run id>

# After a merge conflict in INDEX.csv or the README table: take either side, then
python3 scripts/history_test.py index
```

## Security

- **Threats considered.** A secret or personal detail in a log reaching the repository (CWE-532, CWE-200); a crafted project folder making the packer read or write outside the run folder (CWE-22, CWE-59); a huge file filling the repository (CWE-400); a later edit of a recorded result going unnoticed (CWE-345); a run title that acts as a spreadsheet formula when `INDEX.csv` is opened (CWE-1236); command injection through the wrapped command (CWE-78).
- **Controls.** The wrapped command is an argument list, never a shell string. Copied material is data: never executed or imported. Only regular files are copied, symlinks are never followed, every path segment is checked against an allow-list, run ids are checked before they are used as folder names. Files named like secrets (`.env`, `*.key`, `*.pem`, key stores, `credentials`, `.npmrc`, `.netrc`), model weights and disk images, and `.git`, `node_modules`, virtual environments and caches are never copied; each left-out file is listed with size and SHA-256. Text is redacted before it is stored: private key blocks, bearer and basic tokens, cookies, well-known token prefixes, `NAME=value` secrets, passwords in URLs, e-mail addresses, user names in home folder paths, private IPv4 addresses. No host name, user name or address of the recording machine is written. A run is packed in a private temporary folder and moved into place when complete. `manifest.json` holds SHA-256 (FIPS 180-4) of every file and the manifest checksum of the run before (NIST SP 800-53 AU-9, AU-10). `verify` scans stored files again and fails on a secret pattern. `INDEX.csv` cells that start with `=`, `+`, `-` or `@` get a leading apostrophe.
- **Limits.** Redaction works on patterns and cannot recognise every secret, for example a bare random string with no name next to it, or a host name it was not told about. Read what you commit and run gitleaks. The records are not signed: checksums and links detect accidental and partial changes, and the Git history shows who changed what; someone who can rewrite the repository can rewrite the archive. Over-redaction happens: a test named like a secret setting followed by a colon can lose the text after it.
- **Dependencies.** Python standard library only; nothing to check for CVEs. Optional: `gitleaks` (install with `scripts/ci-install-scanners.sh`, pinned and hash-verified).

## Troubleshooting

| Message | Cause and fix |
|---|---|
| `INDEX.csv or the README table does not match the run folders` | A merge mixed two versions, or a file was edited by hand. Run `python3 scripts/history_test.py index`. |
| `chain: the run before this one is missing` | A run folder was deleted or the branch was cut before the earlier run was merged. Restore the folder from Git; runs are never deleted. |
| `changed: <file>` | A recorded file differs from its checksum. Restore it with `git checkout <commit> -- <path>`; put corrections in a note. On Windows make sure Git does not convert line ends in `HistoryTest/` (the folder has a `.gitattributes` with `-text`). |
| `looks like a secret (...)` | A stored file matches a secret pattern. Rotate the secret if it was real, then record the run again with `--redact` or without that file, and mark the old run invalid. |
| `name not allowed` in `manifest.json` | The file name has characters outside the allow-list or the path is longer than 170 characters. Rename the file before recording if it matters. |

## Tests

`python3 -I scripts/test_history_test.py` (17 tests): redaction and what must stay readable, copy rules (symlinks, names, secret-looking files, weights, limits), checksum and chain checks, index generation and formula guard, notes and the invalid mark, the wrapped command. Evidence: see the `history-test` runs in [HistoryTest/README.md](../../HistoryTest/README.md).
