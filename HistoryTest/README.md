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
| 2026-11-22 07:19:29 | node-host-real | `0c3d711` | pass | [Core downloads the pinned Windows Node host from the public pre-release, checks it, starts it, token enforced](runs/2026-11-22_071929Z_node-host-real_0c3d711/RUN.md) |
| 2026-11-22 07:19:04 | node-host-build | `87b8c86` | pass | [Windows x64 Node host server build from VS Code 1.145.2 on the PC](runs/2026-11-22_071904Z_node-host-build_87b8c86/RUN.md) |
| 2026-11-22 06:16:12 | ui | `dab272f` | pass | [UI smoke after pinning our own Node host build](runs/2026-11-22_061612Z_ui_dab272f/RUN.md) |
| 2026-11-22 06:15:50 | core | `dab272f` | pass | [Core tests after pinning our own Node host build](runs/2026-11-22_061550Z_core_dab272f/RUN.md) |
| 2026-11-22 06:15:44 | node-host-pin | `dab272f` | pass | [Pinned Linux x64 Node host downloads anonymously from the public pre-release with the pinned SHA-256](runs/2026-11-22_061544Z_node-host-pin_dab272f/RUN.md) |
| 2026-11-22 05:37:59 | node-host-build | `4096b2e` | pass | [Node extension host server built from VS Code 1.145.2, Linux x64](runs/2026-11-22_053759Z_node-host-build_4096b2e/RUN.md) |
| 2026-11-22 05:37:20 | ui-node-host-real | `832a0a7` | pass | [Editor with a real Node host built from VS Code 1.145.2 (Linux x64), fresh logs only](runs/2026-11-22_053720Z_ui-node-host-real_832a0a7/RUN.md) |
| 2026-11-22 05:36:24 | ui-node-host-real | `4096b2e` | pass | [Editor with a real Node host built from VS Code 1.145.2 (Linux x64)](runs/2026-11-22_053624Z_ui-node-host-real_4096b2e/RUN.md) |
| 2026-11-22 04:29:52 | node-host | `c727219` | partial | [Real Node extension in the desktop app on the PC: 3 bugs fixed, version mismatch found, version check verified](runs/2026-11-22_042952Z_node-host_c727219/RUN.md) |
| 2026-11-22 04:26:00 | ui | `c727219` | pass | [UI smoke with a mismatched Node host](runs/2026-11-22_042600Z_ui_c727219/RUN.md) |
| 2026-11-22 04:26:00 | app-unit | `c727219` | pass | [App unit tests incl. the VS Code version match rule](runs/2026-11-22_042600Z_app-unit_c727219/RUN.md) |
| 2026-11-22 04:25:49 | core | `c727219` | pass | [Core unit tests after the I6c-3 version check](runs/2026-11-22_042549Z_core_c727219/RUN.md) |
| 2026-11-22 01:59:16 | core | `506f165` | pass | [Phase I6c-3 core unit tests with the real pins (plus clippy on all targets)](runs/2026-11-22_015916Z_core_506f165/RUN.md) |
| 2026-11-22 01:59:08 | node-host | `506f165` | pass | [Phase I6c-3: the real pinned VSCodium 1.140.06880 server downloads, starts and needs its token (PC, Windows x64)](runs/2026-11-22_015908Z_node-host_506f165/RUN.md) |
| 2026-11-22 01:46:48 | security-scan | `10da8d4` | pass | [Phase I6c-2 static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices](runs/2026-11-22_014648Z_security-scan_10da8d4/RUN.md) |
| 2026-11-22 01:46:47 | app-unit | `10da8d4` | pass | [Phase I6c-2 app unit tests](runs/2026-11-22_014647Z_app-unit_10da8d4/RUN.md) |
| 2026-11-22 01:46:26 | ui | `10da8d4` | pass | [Phase I6c-2 UI smoke test: Node extensions panel, trust, download, connection to the host (headless Chromium, shipped CS](runs/2026-11-22_014626Z_ui_10da8d4/RUN.md) |
| 2026-11-22 01:46:15 | core | `10da8d4` | pass | [Phase I6c-2 core unit tests (Node extensions install, staged for the host, pinned commit)](runs/2026-11-22_014615Z_core_10da8d4/RUN.md) |
| 2026-11-22 00:42:43 | core | `48f004d` | pass | [Phase I6c-1 core unit tests after the Windows trust-test fix](runs/2026-11-22_004243Z_core_48f004d/RUN.md) |
| 2026-11-21 23:29:48 | security-scan | `f363048` | pass | [Phase I6c-1 static checks (re-run after notices update)](runs/2026-11-21_232948Z_security-scan_f363048/RUN.md) |
| 2026-11-21 23:29:14 | security-scan | `3e0d986` | fail | [Phase I6c-1 static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices](runs/2026-11-21_232914Z_security-scan_3e0d986/RUN.md) |
| 2026-11-21 23:28:57 | core | `3e0d986` | pass | [Phase I6c-1 Node extension host core: unit tests](runs/2026-11-21_232857Z_core_3e0d986/RUN.md) |
| 2026-11-21 23:18:29 | security-scan | `caf40d4` | pass | [Phase I6b internet consent static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices](runs/2026-11-21_231829Z_security-scan_caf40d4/RUN.md) |
| 2026-11-21 23:18:29 | app-unit | `caf40d4` | pass | [Phase I6b internet consent: app unit tests](runs/2026-11-21_231829Z_app-unit_caf40d4/RUN.md) |
| 2026-11-21 23:18:13 | ui | `caf40d4` | pass | [Phase I6b UI smoke test: internet for web extensions asked once per extension (headless Chromium, shipped CSP)](runs/2026-11-21_231813Z_ui_caf40d4/RUN.md) |
| 2026-11-21 23:18:02 | core | `caf40d4` | pass | [Phase I6b internet consent: core unit tests (ext_net request checks, network grant)](runs/2026-11-21_231802Z_core_caf40d4/RUN.md) |
| 2026-11-21 21:50:28 | security-scan | `fd65ba7` | pass | [Phase I6b static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices](runs/2026-11-21_215028Z_security-scan_fd65ba7/RUN.md) |
| 2026-11-21 21:50:28 | app-unit | `fd65ba7` | pass | [Phase I6b app unit tests](runs/2026-11-21_215028Z_app-unit_fd65ba7/RUN.md) |
| 2026-11-21 21:50:16 | ui | `fd65ba7` | pass | [Phase I6b UI smoke test with a web extension in the worker host (headless Chromium, shipped CSP)](runs/2026-11-21_215016Z_ui_fd65ba7/RUN.md) |
| 2026-11-21 21:50:05 | core | `fd65ba7` | pass | [Phase I6b core unit tests (web extensions install, Node and proposed-API extensions refused)](runs/2026-11-21_215005Z_core_fd65ba7/RUN.md) |
| 2026-11-21 20:56:39 | security-scan | `1e284ce` | pass | [Phase I7 static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices](runs/2026-11-21_205639Z_security-scan_1e284ce/RUN.md) |
| 2026-11-21 20:56:30 | tf-engine-e2e | `1e284ce` | pass | [Phase I7 end to end: IDE core against a real TandemForge backend (fake Ollama)](runs/2026-11-21_205630Z_tf-engine-e2e_1e284ce/RUN.md) |
| 2026-11-21 20:56:04 | app-unit | `1e284ce` | pass | [Phase I7 app unit tests](runs/2026-11-21_205604Z_app-unit_1e284ce/RUN.md) |
| 2026-11-21 20:55:57 | ui | `1e284ce` | pass | [Phase I7 UI smoke test with TandemForge pairing (headless Chromium, shipped CSP)](runs/2026-11-21_205557Z_ui_1e284ce/RUN.md) |
| 2026-11-21 20:55:45 | core | `1e284ce` | pass | [Phase I7 core unit tests (tandemforge module, TandemForge gateway kind)](runs/2026-11-21_205545Z_core_1e284ce/RUN.md) |
| 2026-11-21 17:35:22 | security-scan | `848b60e` | pass | [Phase I6a static checks: tsc, rustfmt, clippy, cargo audit, npm audit gate, licence notices](runs/2026-11-21_173522Z_security-scan_848b60e/RUN.md) |
| 2026-11-21 17:34:18 | app-unit | `848b60e` | pass | [App unit tests (paths, language ids)](runs/2026-11-21_173418Z_app-unit_848b60e/RUN.md) |
| 2026-11-21 17:34:12 | ui | `848b60e` | pass | [UI smoke test under the shipped CSP, with the new Extensions view checks](runs/2026-11-21_173412Z_ui_848b60e/RUN.md) |
| 2026-11-21 17:33:57 | ovsx-cached | `848b60e` | pass | [Real install path on Open VSX files cached with curl](runs/2026-11-21_173357Z_ovsx-cached_848b60e/RUN.md) |
| 2026-11-21 17:33:47 | core | `848b60e` | pass | [Core unit tests incl. 9 new Open VSX extension tests](runs/2026-11-21_173347Z_core_848b60e/RUN.md) |
| 2026-11-18 01:11:29 | security-scan | `aee787c` | pass | [npm audit gate on app/ and its unit tests before merging PR 15](runs/2026-11-18_011129Z_security-scan_aee787c/RUN.md) |
| 2026-11-17 23:52:09 | security-scan | `dc6d647` | pass | [npm audit gate: unit tests and live gate on app/](runs/2026-11-17_235209Z_security-scan_dc6d647/RUN.md) |
| 2026-11-17 23:29:34 | security-scan | `64acea3` | partial | [npm audit for GHSA-fc5q-39xc-vjpf (picomatch ReDoS)](runs/2026-11-17_232934Z_security-scan_64acea3/RUN.md) |
| 2026-11-17 22:52:40 | ui | `64acea3` | pass | [UI smoke test after adding the docs screenshot script](runs/2026-11-17_225240Z_ui_64acea3/RUN.md) |
| 2026-11-17 22:52:31 | docs-shots | `64acea3` | pass | [IDE documentation screenshots (10) from the built webview with the ui-smoke fake bridge](runs/2026-11-17_225231Z_docs-shots_64acea3/RUN.md) |
| 2026-11-17 19:38:21 | installers | `377af6f` | pass | [Licence gate after regenerating THIRD-PARTY-NOTICES.txt (I5c/I5d crates)](runs/2026-11-17_193821Z_installers_377af6f/RUN.md) |
| 2026-11-17 14:08:46 | windows-containers | `8bba2b4` | pass | [Container runner on the PC with Docker Desktop 29.9.2 at 8bba2b4: core tests and real containers](runs/2026-11-17_140846Z_windows-containers_8bba2b4/RUN.md) |
| 2026-11-17 14:07:53 | security-scan | `104f232` | pass | [cargo audit and npm audit (I5d)](runs/2026-11-17_140753Z_security-scan_104f232/RUN.md) |
| 2026-11-17 14:07:43 | ui-smoke | `104f232` | pass | [Webview tests, build and UI smoke with container runner (I5d)](runs/2026-11-17_140743Z_ui-smoke_104f232/RUN.md) |
| 2026-11-17 14:07:41 | windows-cross | `104f232` | pass | [Core clippy for the Windows target (I5d)](runs/2026-11-17_140741Z_windows-cross_104f232/RUN.md) |
| 2026-11-17 14:07:39 | shell | `104f232` | pass | [Tauri shell fmt and clippy (I5d)](runs/2026-11-17_140739Z_shell_104f232/RUN.md) |
| 2026-11-17 14:07:06 | local-containers | `104f232` | pass | [Real Docker container runs (I5d)](runs/2026-11-17_140706Z_local-containers_104f232/RUN.md) |
| 2026-11-17 14:06:53 | core | `104f232` | pass | [Core fmt, clippy and tests (I5d)](runs/2026-11-17_140653Z_core_104f232/RUN.md) |
| 2026-11-17 13:47:17 | security-scan | `dbbc5cd` | pass | [cargo audit (core, shell) and npm audit](runs/2026-11-17_134717Z_security-scan_dbbc5cd/RUN.md) |
| 2026-11-17 13:47:13 | shell | `dbbc5cd` | pass | [Tauri shell fmt and clippy with the updated lockfile](runs/2026-11-17_134713Z_shell_dbbc5cd/RUN.md) |
| 2026-11-17 13:47:12 | windows-cross | `dbbc5cd` | pass | [Cross clippy of the core for x86_64-pc-windows-msvc (Windows sandbox code)](runs/2026-11-17_134712Z_windows-cross_dbbc5cd/RUN.md) |
| 2026-11-17 13:47:06 | local-toolchains | `dbbc5cd` | pass | [Real toolchains in the Linux sandbox with the corrected cache check](runs/2026-11-17_134706Z_local-toolchains_dbbc5cd/RUN.md) |
| 2026-11-17 13:46:51 | core | `dbbc5cd` | pass | [Core on Linux with the Windows sandbox added (Linux code unchanged in behaviour)](runs/2026-11-17_134651Z_core_dbbc5cd/RUN.md) |
| 2026-11-17 13:46:40 | windows-sandbox | `dbbc5cd` | partial | [Windows local sandbox (AppContainer) on the PC at dbbc5cd: confinement test and real toolchains](runs/2026-11-17_134640Z_windows-sandbox_dbbc5cd/RUN.md) |
| 2026-11-17 13:42:23 | windows-sandbox | `8819a48` | fail | [Second Windows sandbox run on the PC at 8819a48 (excerpt only)](runs/2026-11-17_134223Z_windows-sandbox_8819a48/RUN.md) |
| 2026-11-17 13:34:00 | windows-sandbox | `5b8a053` | fail | [First real run of the Windows local sandbox (AppContainer) on the PC at 5b8a053](runs/2026-11-17_133400Z_windows-sandbox_5b8a053/RUN.md) |
| 2026-11-17 13:23:00 | windows | `a8b439e` | pass | [Windows build and tests of phase I5c (local sandbox, Linux code left out) at a8b439e](runs/2026-11-17_132300Z_windows_a8b439e/RUN.md) |
| 2026-11-17 13:19:37 | security-scan | `e980a8c` | pass | [cargo audit (core, shell) and npm audit](runs/2026-11-17_131937Z_security-scan_e980a8c/RUN.md) |
| 2026-11-17 13:19:28 | ui-smoke | `e980a8c` | pass | [Webview UI smoke with the runner picker and local sandbox card text](runs/2026-11-17_131928Z_ui-smoke_e980a8c/RUN.md) |
| 2026-11-17 13:19:26 | shell | `e980a8c` | pass | [Tauri shell: fmt and clippy with runners_list and run_on](runs/2026-11-17_131926Z_shell_e980a8c/RUN.md) |
| 2026-11-17 13:19:00 | local-toolchains | `e980a8c` | pass | [Real toolchains in the local sandbox (cargo x2, go, npm, make, unittest)](runs/2026-11-17_131900Z_local-toolchains_e980a8c/RUN.md) |
| 2026-11-17 13:18:47 | core | `e980a8c` | pass | [Core: fmt, clippy, tests incl. real-kernel local sandbox confinement](runs/2026-11-17_131847Z_core_e980a8c/RUN.md) |
| 2026-11-17 13:11:35 | live-gateway | `a6170be` | pass | [Live run and test after merging main (cap-std workspace) into PR #9](runs/2026-11-17_131135Z_live-gateway_a6170be/RUN.md) |
| 2026-11-17 13:03:06 | release | `a687f4e` | pass | [IDE PR 6 (installers) with main merged: config parses, merge has no conflict markers](runs/2026-11-17_130306Z_release_a687f4e/RUN.md) |
| 2026-11-17 13:01:13 | windows | `391278c` | pass | [Windows build and tests of phase I5b (Run and test) at 391278c](runs/2026-11-17_130113Z_windows_391278c/RUN.md) |
| 2026-11-17 12:59:53 | live-gateway | `618354d` | pass | [Live run and test against a real P2P-AI gateway, node and sandbox (live_runs)](runs/2026-11-17_125953Z_live-gateway_618354d/RUN.md) |
| 2026-11-17 12:59:44 | security-scan | `618354d` | pass | [cargo audit and npm audit for phase I5b](runs/2026-11-17_125944Z_security-scan_618354d/RUN.md) |
| 2026-11-17 12:59:34 | ui-smoke | `618354d` | pass | [Webview unit tests, build and UI smoke 75 checks incl. Run and test and the approval card](runs/2026-11-17_125934Z_ui-smoke_618354d/RUN.md) |
| 2026-11-17 12:59:32 | shell | `618354d` | pass | [Tauri shell fmt and clippy with run_start and agent_decide](runs/2026-11-17_125932Z_shell_618354d/RUN.md) |
| 2026-11-17 12:59:30 | core | `618354d` | pass | [Core fmt, clippy and tests for phase I5b (runner, agent run tools)](runs/2026-11-17_125930Z_core_618354d/RUN.md) |
| 2026-11-17 12:52:33 | security-scan | `6ebd731` | pass | [OSV lookup of the crates cap-std adds](runs/2026-11-17_125233Z_security-scan_6ebd731/RUN.md) |
| 2026-11-17 12:52:26 | shell | `6ebd731` | invalid | [INVALID (The cloud container has no GTK/WebKit development libraries (gdk-sys build script failed), so the Tauri shell cannot build here. No public function of core changed, so the shell's calls are u](runs/2026-11-17_125226Z_shell_6ebd731/RUN.md) |
| 2026-11-17 12:52:23 | core | `6ebd731` | pass | [Core tests, fmt and clippy after moving workspace file access to cap-std handles](runs/2026-11-17_125223Z_core_6ebd731/RUN.md) |
| 2026-11-17 09:15:18 | windows | `71c6f12` | pass | [Windows core re-run after the real-git test fix (71c6f12)](runs/2026-11-17_091518Z_windows_71c6f12/RUN.md) |
| 2026-11-17 09:13:14 | core | `71c6f12` | pass | [Core fmt, clippy and tests after the Windows-safe real-git test fix](runs/2026-11-17_091314Z_core_71c6f12/RUN.md) |
| 2026-11-17 09:08:45 | windows | `3a1a8bd` | fail | [Windows build and tests of phase I5a (Source Control) at 3a1a8bd](runs/2026-11-17_090845Z_windows_3a1a8bd/RUN.md) |
| 2026-11-17 09:07:57 | security-scan | `86e1c4f` | pass | [cargo audit and npm audit for I5a](runs/2026-11-17_090757Z_security-scan_86e1c4f/RUN.md) |
| 2026-11-17 09:07:46 | ui-smoke | `86e1c4f` | pass | [Webview: UI smoke with the Source Control view](runs/2026-11-17_090746Z_ui-smoke_86e1c4f/RUN.md) |
| 2026-11-17 09:07:45 | shell | `86e1c4f` | pass | [Tauri shell: fmt and clippy with the source control commands](runs/2026-11-17_090745Z_shell_86e1c4f/RUN.md) |
| 2026-11-17 09:07:43 | core | `86e1c4f` | pass | [Core: fmt, clippy and tests with I5a source control](runs/2026-11-17_090743Z_core_86e1c4f/RUN.md) |
| 2026-11-17 08:59:43 | windows | `bfae51f` | pass | [Windows build and tests of phase I4 (bfae51f)](runs/2026-11-17_085943Z_windows_bfae51f/RUN.md) |
| 2026-11-17 08:55:09 | installers | `1b95d91` | pass | [Linux .deb/.rpm bundle of the IDE with the new bundle settings](runs/2026-11-17_085509Z_installers_1b95d91/RUN.md) |
| 2026-11-17 08:55:07 | installers | `1b95d91` | pass | [Licence gate: IDE](runs/2026-11-17_085507Z_installers_1b95d91/RUN.md) |
| 2026-11-17 08:52:25 | live-gateway | `bfae51f` | pass | [Live: IDE core against a real Go gateway, node and sandbox (Autopilot mission)](runs/2026-11-17_085225Z_live-gateway_bfae51f/RUN.md) |
| 2026-11-17 08:23:59 | security-scan | `7e32c2d` | pass | [cargo audit and npm audit for I4](runs/2026-11-17_082359Z_security-scan_7e32c2d/RUN.md) |
| 2026-11-17 08:23:40 | ui-smoke | `7e32c2d` | pass | [Webview: UI smoke with the Missions view](runs/2026-11-17_082340Z_ui-smoke_7e32c2d/RUN.md) |
| 2026-11-17 08:23:40 | shell | `7e32c2d` | pass | [Tauri shell: fmt and clippy with the missions commands](runs/2026-11-17_082340Z_shell_7e32c2d/RUN.md) |
| 2026-11-17 08:23:36 | core | `7e32c2d` | pass | [Core: fmt, clippy and tests with I4 projects sync](runs/2026-11-17_082336Z_core_7e32c2d/RUN.md) |
| 2026-11-17 07:46:10 | windows | `0a3a48f` | partial | [Windows click-through of I2 keychain and I3 chat tools against a local model](runs/2026-11-17_074610Z_windows_0a3a48f/RUN.md) |
| 2026-11-17 07:45:53 | windows | `0a3a48f` | pass | [Windows build and tests of phase I3 (f3cde42, then 0a3a48f with the CRLF fix)](runs/2026-11-17_074553Z_windows_0a3a48f/RUN.md) |
| 2026-11-17 07:13:48 | core | `0a3a48f` | pass | [Core fmt, clippy and tests with the CRLF edit fix](runs/2026-11-17_071348Z_core_0a3a48f/RUN.md) |
| 2026-11-17 06:52:49 | security-scan | `e919fcd` | pass | [cargo audit and npm audit after phase I3 (no new dependencies)](runs/2026-11-17_065249Z_security-scan_e919fcd/RUN.md) |
| 2026-11-17 06:52:40 | ui-smoke | `e919fcd` | pass | [Webview: type check, unit tests, build, UI smoke with tool steps and diff review](runs/2026-11-17_065240Z_ui-smoke_e919fcd/RUN.md) |
| 2026-11-17 06:52:39 | shell | `e919fcd` | pass | [Tauri shell: fmt and clippy with the agent commands](runs/2026-11-17_065239Z_shell_e919fcd/RUN.md) |
| 2026-11-17 06:52:36 | core | `e919fcd` | pass | [Core: fmt, clippy, tests incl. agent tools and pending changes (phase I3)](runs/2026-11-17_065236Z_core_e919fcd/RUN.md) |
| 2026-11-17 06:38:32 | security-scan | `545bdf4` | pass | [cargo audit of both lockfiles after adding ed25519-dalek, base64, getrandom](runs/2026-11-17_063832Z_security-scan_545bdf4/RUN.md) |
| 2026-11-17 06:38:23 | ui-smoke | `545bdf4` | pass | [Webview: type check, unit tests, build, UI smoke with the public gateway list](runs/2026-11-17_063823Z_ui-smoke_545bdf4/RUN.md) |
| 2026-11-17 06:38:21 | shell | `545bdf4` | pass | [Tauri shell: fmt and clippy with the public list commands](runs/2026-11-17_063821Z_shell_545bdf4/RUN.md) |
| 2026-11-17 06:38:19 | core | `545bdf4` | pass | [Core: fmt, clippy, tests incl. shared advert vectors (phase I2b)](runs/2026-11-17_063819Z_core_545bdf4/RUN.md) |
| 2026-11-17 06:20:58 | branding | `ea76006` | pass | [IDE icons verified (run in the P2P-AI repository)](runs/2026-11-17_062058Z_branding_ea76006/RUN.md) |
| 2026-11-17 06:20:16 | security-scan | `bd2763f` | pass | [cargo audit of both lockfiles after adding ureq, serde_json, keyring](runs/2026-11-17_062016Z_security-scan_bd2763f/RUN.md) |
| 2026-11-17 06:20:08 | ui-smoke | `bd2763f` | pass | [Webview type check, unit tests, build and UI smoke (30 checks incl. gateways and chat)](runs/2026-11-17_062008Z_ui-smoke_bd2763f/RUN.md) |
| 2026-11-17 06:20:07 | shell | `bd2763f` | pass | [Tauri shell fmt and clippy with net.rs and keyring](runs/2026-11-17_062007Z_shell_bd2763f/RUN.md) |
| 2026-11-17 06:20:06 | core | `bd2763f` | pass | [Core fmt, clippy, tests with gateways/client/chats (31 tests)](runs/2026-11-17_062006Z_core_bd2763f/RUN.md) |
| 2026-11-17 06:18:00 | windows | `71a1e22` | pass | [Windows build and tests of phase I2a (core 29 tests, webview, shell, release exe)](runs/2026-11-17_061800Z_windows_71a1e22/RUN.md) |
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
