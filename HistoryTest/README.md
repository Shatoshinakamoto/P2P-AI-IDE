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
| 2026-11-17 06:20:58 | branding | `ea76006` | pass | [IDE icons verified (run in the P2P-AI repository)](runs/2026-11-17_062058Z_branding_ea76006/RUN.md) |
| 2026-11-17 06:20:16 | security-scan | `bd2763f` | pass | [cargo audit of both lockfiles after adding ureq, serde_json, keyring](runs/2026-11-17_062016Z_security-scan_bd2763f/RUN.md) |
| 2026-11-17 06:20:08 | ui-smoke | `bd2763f` | pass | [Webview type check, unit tests, build and UI smoke (30 checks incl. gateways and chat)](runs/2026-11-17_062008Z_ui-smoke_bd2763f/RUN.md) |
| 2026-11-17 06:20:07 | shell | `bd2763f` | pass | [Tauri shell fmt and clippy with net.rs and keyring](runs/2026-11-17_062007Z_shell_bd2763f/RUN.md) |
| 2026-11-17 06:20:06 | core | `bd2763f` | pass | [Core fmt, clippy, tests with gateways/client/chats (31 tests)](runs/2026-11-17_062006Z_core_bd2763f/RUN.md) |
| 2026-11-17 05:51:29 | ui-smoke | `e428736` | pass | [Type check, unit tests, build and UI smoke with the tab-language check](runs/2026-11-17_055129Z_ui-smoke_e428736/RUN.md) |
| 2026-11-17 05:45:00 | windows | `e30cb3e` | pass | [Windows rebuild of the fix (e30cb3e) and manual click-through: 9/9](runs/2026-11-17_054500Z_windows_e30cb3e/RUN.md) |
| 2026-11-17 05:44:04 | ui-smoke | `e30cb3e` | pass | [UI smoke after the blank-window fix (freezePrototype off; test now applies the freeze when configured)](runs/2026-11-17_054404Z_ui-smoke_e30cb3e/RUN.md) |
| 2026-11-17 05:40:00 | windows | `ba63e5c` | fail | [Windows real-window check of ba63e5c: blank window (invalidates the earlier launch-only pass)](runs/2026-11-17_054000Z_windows_ba63e5c/RUN.md) |
| 2026-11-17 05:14:58 | windows | `ba63e5c` | pass | [Windows PC build and tests of phase I1 (core, webview, shell, release exe)](runs/2026-11-17_051458Z_windows_ba63e5c/RUN.md) |
| 2026-11-17 05:00:05 | docs | `70537dc` | pass | [Link check of README and docs](runs/2026-11-17_050005Z_docs_70537dc/RUN.md) |
| 2026-11-17 04:59:33 | security-scan | `fe528ba` | pass | [cargo audit of both Rust lockfiles (RustSec)](runs/2026-11-17_045933Z_security-scan_fe528ba/RUN.md) |
| 2026-11-17 04:58:08 | security-scan | `fe528ba` | pass | [npm audit of the webview dependencies (after the DOMPurify override)](runs/2026-11-17_045808Z_security-scan_fe528ba/RUN.md) |
| 2026-11-17 04:58:06 | ui-smoke | `fe528ba` | pass | [UI smoke: built webview in headless Chromium with a fake workspace, under the shipped CSP](runs/2026-11-17_045806Z_ui-smoke_fe528ba/RUN.md) |
| 2026-11-17 04:57:46 | webview | `fe528ba` | pass | [Webview: type check, unit tests, production build, patched DOMPurify in the bundle](runs/2026-11-17_045746Z_webview_fe528ba/RUN.md) |
| 2026-11-17 04:57:41 | shell | `fe528ba` | pass | [Tauri shell: fmt, clippy and debug build on Linux (WebKitGTK 2.52)](runs/2026-11-17_045741Z_shell_fe528ba/RUN.md) |
| 2026-11-17 04:57:40 | core | `fe528ba` | pass | [Rust core: fmt, clippy and unit tests (path confinement, saves, search)](runs/2026-11-17_045740Z_core_fe528ba/RUN.md) |
<!-- index:end -->
