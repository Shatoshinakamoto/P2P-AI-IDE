# HistoryTest: the record of every test run

This folder is the test history of the P2P-AI IDE repository (the recorder and these rules are copied from the P2P-AI repository). Every test run made while working on the project is kept
here with its description, commands, environment, results, full logs, the test project it started from and the
finished project it ended with, so that anyone can later go back, read it, check it and run it again.

It covers **all** testing: unit, integration, end-to-end, evals and scorecards, smoke tests, security scans (SAST,
SCA, secret, container), performance, PC/Windows batch runs, cloud runs and manual checks, whether they passed,
failed, were aborted, were flaky or turned out invalid.

**This is a development record, not a product feature.** Nothing in the IDE app, its core or its installers reads
this folder, and it is kept out of shipped code, packages, container images and installers (the app bundles only `app/dist` and the Rust binaries).
`CLAUDE.md` Section 9 makes archiving part of every task; this page is the full rule set it points to.

## How to read it

- The table at the end of this page lists every run, newest first. `INDEX.csv` holds the same list for spreadsheets
  and scripts.
- Each run has one folder: `runs/<date>_<time>Z_<area>_<commit>/`. Open its `RUN.md` first.
- The commit in a run is the one the test ran on. Pull requests are squash-merged, so a run made on a work branch
  names a branch commit: find it through the branch or pull request named in `RUN.md`, not in the history of `main`.

| In a run folder | What it is |
|---|---|
| `RUN.md` | Plain-language description: what was tested and why, how it was run, results, failures and analysis, verdict, how to reproduce, and one line per file |
| `manifest.json` | The run facts in machine-readable form, the SHA-256 and size of every file in the folder, and the link to the run recorded before it |
| `command.txt` | The exact commands that were run (names of environment variables only, never their values) |
| `environment.md` | Machine type, OS, hardware, tool versions, model names, settings |
| `results/` | Machine-readable results; `summary.txt` always has the verdict, counts, exit code and duration |
| `logs/` | Full output and logs of the run (secrets and personal details blanked) |
| `projects/input/` | The test project exactly as it was given to the test |
| `projects/output/` | The finished project after the run |
| `projects/diff.patch` | Unified diff from input to output |
| `artifacts/` | Other evidence: screenshots, scan reports, SBOMs, timings |
| `NOTES.md` | Optional dated notes, follow-ups and corrections, added to and never rewritten |
| `INVALID.md` | Present only when the run was later found unreliable; says why. The run itself stays as recorded |

`<area>` in the folder name is a short tag such as `gateway`, `node-go`, `node-python`, `web`, `autopilot`,
`toolkits`, `sandbox`, `discovery`, `release`, `security-scan`.

`INDEX.csv` columns: `run_id,date_utc,area,commit,branch_or_pr,machine,verdict,passed,failed,skipped,summary`.

Verdicts: `pass`, `fail`, `partial` (some checks failed or part of the run is missing) and `invalid` (the result is
kept but must not be relied on, for example a run against a stale image).

Record types in `RUN.md`: **captured** (the recorder ran the command and saved its output), **imported** (raw logs of
a run made on another machine were copied in), **reported** (only a written report is available; the raw logs are
not in the folder, and `RUN.md` says so).

## How to check it

```
python3 scripts/history_test.py verify                    # every run, the index files and the chain
python3 scripts/history_test.py verify <run id>           # one run
python3 scripts/history_test.py verify --base origin/main # also: no earlier run was changed (append-only)
```

`verify` recomputes the SHA-256 of every file and compares it with `manifest.json`, reports missing and added files,
checks that `INDEX.csv` and the table below match the run folders, scans every stored file again for things that
look like secrets, and runs `gitleaks` when it is installed. Each manifest also names the run recorded before it
together with the checksum of that run's manifest, so rewriting an old run breaks the link kept by the next one. Two
branches may continue from the same earlier run, as Git commits do.

What this does and does not prove: the checksums and links show accidental or partial changes, and the Git history
of the repository shows who changed what and when. The records are not signed, so someone with write access to the
repository could rewrite a run together with every later manifest; compare against an older clone or a release tag
when that matters. To reproduce a result, follow "How to reproduce" in the run's `RUN.md`.

## How to add a run

Use the recorder; do not build folders by hand. Full reference: `docs/commands/history-test.md`.

```
# run a test and record it (output, exit code, counts, duration are captured)
python3 scripts/history_test.py run --area gateway --kind unit --title "Go gateway unit tests" \
    --what "..." --by "<task or thread>" --cwd P2P-AI-Gateways/Go -- go test ./...

# record a run made on another machine from its logs
python3 scripts/history_test.py add --area autopilot --kind e2e --machine pc --verdict pass --commit <sha> \
    --when 2026-11-11T08:45:00Z --title "..." --log <log file or folder> --source-path "<where the logs were>" \
    --input <test project> --output <finished project> --env model=<name>

# later: a note, or mark a run as unreliable (the run is kept)
python3 scripts/history_test.py note <run id> --text "..."
python3 scripts/history_test.py invalidate <run id> --reason "..."
```

## `RUN.md` template (required in every run folder)

The recorder writes it from the options you pass (`--title`, `--by`, `--what`, `--how`, `--results-text`,
`--failures`, `--reason`, `--reproduce`); fill them in so the description stands on its own.

```
# <run id> — <short title>

- **Date (UTC) / duration:**
- **Requested by / task or thread:**
- **Commit / branch / PR:**  (and the commit of the test harness if different)
- **Area / components under test:**
- **Machine:** cloud container | user PC (Windows) | CI runner | ...

## What was tested and why
## How it was run          (steps, commands, configuration, models, data)
## Results                 (passed / failed / skipped counts, scores, timings, resource use)
## Failures and analysis   (root cause; product fault vs. test-setup fault)
## Verdict: pass | fail | partial | invalid   — and the reason
## How to reproduce
## Files in this snapshot  (one line per file: what it is)
```

## Rules

- **Capture everything possible**, then redact. Full logs, inputs, outputs, configs, versions and timings are kept;
  nothing is summarized away.
- **Redact before committing** (CWE-532, CWE-200, NIST AU-9, SI-11): no secrets, tokens, API keys, private keys,
  passwords, enrollment tokens, cookies, personal data, private IPs/hostnames, or usernames in paths. They are
  replaced with `[REDACTED:<type>]`. The recorder does this for private keys, tokens, `NAME=value` secrets, cookies,
  passwords in URLs, e-mail addresses, user names in home folder paths and private IP addresses; pass
  `--redact <machine name>` for host names. Redaction is best effort: read what you commit and run
  `gitleaks detect --no-git --source HistoryTest/runs/<run>` before every commit. A run that cannot be safely
  redacted is stored as `RUN.md` + results only, with a note saying why.
- **Append-only:** never delete or rewrite past runs. Corrections go in `NOTES.md` (dated, `history_test.py note`)
  or a new run that links the old one.
- **Honesty:** results that are copied, stale, from a mismatched build, or otherwise unreliable are kept but marked
  `invalid`, with the reason: record them with `--verdict invalid --reason ...`, or mark an existing run with
  `history_test.py invalidate <run> --reason ...`, which adds `INVALID.md` and leaves every recorded file untouched.
  A run known only from a written report is recorded as **reported** and says that its raw logs are missing.
- **Integrity** (NIST AU-10): `manifest.json` lists the SHA-256 of every file recorded with the run; `NOTES.md` and
  `INVALID.md` are the two dated additions allowed afterwards. Each new manifest records the id and manifest SHA-256
  of the newest run present when it was packed, so the history forms a tamper-evident chain. Because many branches
  add runs at the same time, two runs may name the same earlier run, as Git commits do.
- **Index:** every run appears in `INDEX.csv` and in the table below. Both are generated by the recorder and never
  edited by hand. When two branches both add runs Git reports a conflict in them: take either side and run
  `python3 scripts/history_test.py index`. `verify` fails when they do not match the run folders.
- **Size:** never commit model weights, container images, `node_modules`, virtualenvs or build caches; record their
  name, version and digest instead (the recorder leaves such files out and lists their size and SHA-256 in
  `manifest.json`). Files over 50 MB go through Git LFS or external storage with their SHA-256 recorded; the
  recorder keeps the first and last part of a longer log and notes the checksum of the original. A run over 200 MB
  stops taking files.
- **Results from other machines** (for example PC batch runs) are copied in by whoever ran or collected them,
  noting the source path (`--source-path`) and the time they were collected.
- **Every report of test results** (PR description, chat reply, docs page) links its `HistoryTest/runs/...` folder.
- The archive uses the shared recorder `scripts/history_test.py`; do not build run folders by hand.

## What is not here yet

This repository started on 2026-11-17; every run since then is here.

## Runs

<!-- index:start (generated by scripts/history_test.py index) -->
| Date (UTC) | Area | Commit | Verdict | Run |
|---|---|---|---|---|
<!-- index:end -->
