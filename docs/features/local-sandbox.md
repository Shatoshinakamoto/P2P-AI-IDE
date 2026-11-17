# Local sandbox on this computer (phase I5c)

* **Status:** Linux built 2026-11-17 after phase I5b was merged (PR #9), merged as PR #10. Windows (AppContainer and
  job object, a port of TandemForge's `hostrun/winsandbox.py`) built the same day.
  Requested by Pheonix on 2026-11-17: "include local os sandbox ported from tandemforge". This is a Rust port of
  TandemForge's host runner (`packages/python/src/tandemforge/hostrun/confine.py`, isolation `host-os`, at
  TandemForge 535d695) in **toolchain mode**: the recommended option on the 2026-11-17 decision card. It lets cargo,
  go, npm, make and Python tests run, which TandemForge's per-file execute allow list does not. If Pheonix picks the
  strict mode instead, that adds an allow list on top and is not a rewrite.
* **Code:** `core/src/localbox.rs` (snapshot, toolchain folders, namespaces, Landlock, seccomp, rlimits, run; the
  Windows `run`), `core/src/localbox_win.rs` (AppContainer profile, folder grants, job object, process start),
  `core/src/testparse.rs` (framework commands and test counts, as the gateway's), `app/src-tauri/src/run.rs`
  (`runners_list`, `run_on`), `app/src-tauri/src/agent.rs` (chat runs on the chosen runner),
  `app/src/components/{RunView,ChatView}.tsx`, `app/src/App.tsx`.
* **Tests:** `core/src/localbox.rs` (`local_runs_are_confined`: real Linux kernel; `local_runs_are_confined_on_windows`:
  real Windows), `core/src/localbox_win.rs` (command-line quoting, environment block), `core/src/testparse.rs`,
  `core/examples/local_toolchains.rs` (cargo, go, npm, make, unittest inside the sandbox), `app/tests/ui-smoke.mjs`.

## 1. Overview

**Run and test** offers a second place to run: **Local sandbox on this computer**. A run there never leaves the
computer. It runs on a copy of the folder, cannot reach the network, sees the home folder only where toolchains
live (read only), and its file changes are thrown away. The result looks the same as a gateway run: verdict, test
counts, failing tests and output. The chat's **Runs** use whichever runner is picked in Run and test, and the
approval card says where the run will happen.

The runner is offered only where every confinement piece works: Linux on x86_64 or aarch64 with Landlock and
unprivileged user namespaces, and Windows 10 (1703 or later) or 11, where a confined test start must succeed first.
Elsewhere (macOS) it is listed with the reason it is unavailable.

## 2. How it works

```mermaid
sequenceDiagram
  participant V as RunView / chat (after Allow)
  participant S as shell run.rs
  participant L as core localbox.rs
  participant T as trampoline (forked)
  participant J as job (PID 1 of new namespaces)
  V->>S: run_start(local-os, input)
  S->>L: check(input), run
  L->>L: copy folder to run-jobs/<id> (deny list, no links; node_modules/.venv linked read-only)
  L->>T: fork (std Command + pre_exec)
  T->>T: unshare user, net, ipc, pid, mount, uts, cgroup; uid/gid map to self
  T->>J: fork
  J->>J: private mounts, tmpfs /dev/shm, own /proc, rlimits, no_new_privs, Landlock, seccomp
  J->>J: execve(program, argv) with a cleared environment
  T-->>L: exit status of the job (126 = sandbox could not be set up)
  L-->>V: RunResult (runner local-os, place "this computer"), job folder removed
```

* **Snapshot.** The open folder is copied into `<data>/run-jobs/<random>/`, which is owner-only. The agent deny list
  applies, so `.env`, keys, `.git`, `.ssh`, `secrets/` and the like are not copied. Symlinks are not copied.
  Build folders (`target`, `dist`, `__pycache__`, caches) are skipped. `node_modules`, `.venv` and `venv` are
  linked read-only instead of copied. Limits: 20 000 files, 512 MiB, and files over 100 MiB are left out.
* **Per-folder cache.** `<data>/run-cache/<folder key>/` holds `target` (`CARGO_TARGET_DIR`), `go-build`
  (`GOCACHE`), `gopath`, `npm` and `xdg` caches, plus a `cargo-home` whose `registry` and `git` link to
  `~/.cargo` (read only). A second run reuses builds (verified: the second cargo run did not recompile).
* **Program.** The program is looked up on PATH: the copy's `.venv/bin` first, then the person's PATH entries that
  lie in system or toolchain folders (absolute, outside the open folder). A program given with a path is refused.
* **Frameworks.** The same command lines as the gateway, for example `cargo test --offline --quiet` and
  `python3 -m unittest discover -q` (`core/src/testparse.rs`). The offline settings are the same too, and the
  counts are parsed the same way.
* **Environment.** Cleared, then only: `PATH`; `HOME` and `TMPDIR` inside the job folder; `LANG=C.UTF-8`; `CI=true`;
  the cache variables above; `RUSTUP_HOME` and `GOMODCACHE` pointing at the person's read-only copies; and the
  framework's offline settings.
* **Time limit.** The run's 10 to 300 seconds of wall clock. At the limit the trampoline is killed; the job dies
  with it (`PR_SET_PDEATHSIG`), and with the job, its whole PID namespace.

### Windows (AppContainer and job object)

```mermaid
sequenceDiagram
  participant L as core localbox.rs (run)
  participant W as localbox_win.rs
  participant I as icacls.exe
  participant J as job (AppContainer, job object)
  L->>W: profile P2PAI.IDE.Run.<folder key> (no capabilities)
  L->>I: grant the cache (once) and the new, empty job folder: modify
  L->>L: copy the folder into the job folder (deny list, no links)
  L->>I: grant the program's toolchain folders: read and execute (recorded in run-grants.json)
  L->>W: run_confined(program.exe, args, environment)
  W->>J: CreateProcessW suspended (AppContainer, 3 handles only), assign to job, resume
  J-->>W: exit code, output through pipes
  W->>J: TerminateJobObject (whatever is left)
  L->>I: remove the recorded grants; empty the container's own folder
```

* **No network.** The profile has no capabilities, so the job cannot open internet, LAN or loopback connections.
* **Files.** An app container sees only what Windows gives every app container (reading the Windows and Program
  Files folders) and what is granted to its own SID. The IDE grants **modify** on the job folder and on the folder's
  cache, and **read and execute** on the toolchain group the program comes from (`.cargo\bin` with `.rustup`,
  `.cargo\registry` and `.cargo\git`; `go\bin` with `go\pkg\mod`; `.bun\bin`; `.deno\bin`) and on the program's
  own folder when it is not the Windows folder or a toolchain folder (for example a Python installed for one
  user). A Program Files folder is readable only when its installer allowed app containers (Go's does, Node.js's
  does not); the IDE tries the grant there and goes on without it when the person has no right to change it. Never the
  home folder itself or a drive root. Grants outside the IDE's data folder are written to `run-grants.json` before
  they are made and removed when the run ends, or at the next run after a crash.
* **Processes.** One job object per run: kill on close, at most 1024 processes, 8 GiB for the whole job, CPU time
  8 x the time limit, every UI restriction (no clipboard, desktop switch, global atoms, display or system
  settings, exit Windows, other programs' windows). At the end, or at the time limit, the whole job is terminated.
  No window opens (a hidden console shared by the job's programs).
* **Programs.** `.exe` files only, found on PATH by name (`.bat`, `.cmd` and scripts are refused). `npm` is started
  as `node.exe ...\node_modules\npm\bin\npm-cli.js`; `python3` means `python.exe` first (the python.org name); the Microsoft Store's
  app aliases (`WindowsApps`) are skipped.
* **Environment.** Built from scratch: the Windows folder variables (`SystemRoot`, `ComSpec`, `PATHEXT`, the
  Program Files folders, `ProgramData`, `LOCALAPPDATA`, which Windows requires to start an app container), `PATH`,
  `USERPROFILE`, `HOME` and `APPDATA` in the job folder, and the same cache variables as on Linux.
* **Between runs.** Windows points TEMP, TMP and LOCALAPPDATA of an app container at the container's own folder;
  it is emptied after every run.

### What the job can reach (Linux: Landlock, toolchain mode)

| Where | Access |
|---|---|
| `/usr`, `/lib*`, `/bin`, `/sbin`, `/etc`, `/opt`, `/nix/store`, `/snap` | read and execute |
| Toolchain folders under home: `.cargo/{bin,registry,git}`, `.rustup`, `go/{bin,pkg/mod}`, `sdk`, `.nvm`, `.volta`, `.pyenv`, `.local/{bin,lib}`, `.bun/bin`, `.deno/bin` (those that exist) | read and execute |
| The program's own folder (a toolchain elsewhere), never the home folder itself | read and execute |
| The project's `node_modules` / `.venv` (linked into the copy) | read and execute |
| The job folder (the copy, its `HOME` and `TMPDIR`) and the per-folder cache | read, write, execute |
| `/dev/null`, `/dev/zero`, `/dev/full`, `/dev/random`, `/dev/urandom`, `/dev/tty` | read and write |
| The job's own `/dev/shm` (tmpfs, 64 MiB) and its own `/proc` | write (no execute) / read |
| Everything else, including the rest of the home folder, `/tmp`, other users' files | nothing |
| TCP bind and connect (Landlock ABI 4+) | refused |
| Signals and abstract Unix sockets outside the job (ABI 6+) | refused |

## 3. How it is secure

Threat model (STRIDE, brief). Assets: the person's files, secrets, other programs and the network. Attackers: the
project's own code (a cloned repository's tests or build scripts), a prompt-injected model asking for a run, hostile
output. Every run is untrusted code.

| Threat | Control | Reference |
|---|---|---|
| Reading secrets (SSH and cloud keys, browser profiles, `.env`) | Landlock allow-list: home hidden except toolchain folders, chosen to exclude `.cargo/credentials.toml` and `.local/share/keyrings`; deny list on the copy | CWE-200, CWE-522, CWE-22; NIST AC-3, AC-6 |
| Changing the person's files | writes only to the job folder and the IDE's cache; the real folder is untouched (verified) | CWE-284, CWE-732 |
| Network exfiltration or attacks | empty network namespace (loopback down), Landlock TCP deny, seccomp `socket` only for IPv4/IPv6 (verified: connect fails) | NIST SC-7; CWE-918 |
| Talking to local services (Docker, D-Bus sockets) | seccomp refuses `socket(AF_UNIX)`; Landlock scopes abstract sockets; new IPC namespace | CWE-668 |
| Escaping the sandbox or gaining privileges | `no_new_privs`; user namespace with the person's own uid (no capabilities outside); seccomp refuses ptrace, mount, unshare/setns, new namespaces in clone, clone3, kernel modules, kexec, bpf, perf, userfaultfd, io_uring, keyring, TIOCSTI, x32 calls, other architectures (killed) | CWE-269, CWE-250, CWE-693 |
| Leftover processes, fork bombs, resource exhaustion | job is PID 1 of its PID namespace, so everything ends with it (verified: a background `sleep` is gone); `PR_SET_PDEATHSIG`; rlimits (8 GiB data, 1 GiB file size, 4096 files, 1024 processes, CPU time = 8 x the time limit, no core dumps); wall-clock kill | CWE-400, CWE-404, CWE-770 |
| Shared memory of other programs | a private tmpfs on `/dev/shm` in the job's mount namespace | CWE-668, NIST SC-4 |
| Planted programs and environment tricks | program from PATH entries outside the folder; cleared environment; no shell between the IDE and the program | CWE-426, CWE-427, CWE-526, CWE-78 |
| Copy tricks (symlinks, huge folders) | links not followed when copying; counts and sizes capped; job folder created fresh and owner-only | CWE-59, CWE-377, CWE-400 |
| Half-working sandbox | fail closed: any setup step that fails ends the job with 126 before the program starts, and the runner is offered only when Landlock and user namespaces work | NIST SI-6 |
| Windows: reading files, network, other programs | AppContainer with no capabilities (no network, loopback included); grants only to its own SID, recorded and removed after the run; job object kill on close, process, memory, CPU and UI limits; `.exe` only, inherited handles limited to the three pipes; container folder emptied after each run | NIST AC-6, SC-7, SC-4; CWE-78, CWE-88, CWE-403, CWE-404, CWE-459, CWE-732 |
| Hostile output | cleaned (control characters), capped (64 KiB per stream), parsed without regular expressions, rendered as text | CWE-150, CWE-1333, CWE-79 |

Writing code to these standards does not make the IDE "STIG-compliant"; formal compliance needs an assessment.

## 4. How to use it

1. Open a folder. On Linux, check that unprivileged user namespaces are on. Most desktop distributions have them
   on; on Ubuntu 24.04+ AppArmor may restrict them, see Troubleshooting. On Windows nothing needs to be set up
   (no admin rights); Developer Mode lets Cargo use the crates already on the computer (see section 6).
2. Press the flask icon (**Run and test**), choose **Local sandbox on this computer**, pick **Tests** and a
   framework, and press **Run ... tests**.
3. For the chat, keep that runner selected and turn **Runs** on; the card says "in the local sandbox on this
   computer".

## 5. Dependencies

`libc` 0.2.191 (system calls), now a direct dependency of the core on Unix. It was already in both lockfiles
through other crates, so no new code is downloaded. Checked with cargo audit on 2026-11-17: 0 vulnerabilities (run linked in section 8).
The kernel needs Landlock (ABI 1+; ABI 4+ for the TCP rule, 6+ for the scopes) and user namespaces.

Windows: `windows-sys` 0.61.2 (Microsoft's raw Win32 bindings, only the features the sandbox calls), now a direct
dependency of the core on Windows. It was already in both lockfiles through other crates, so no new code is
downloaded. `icacls.exe` from the Windows system folder (never looked up on PATH).

## 6. Limitations and known risks

* **Toolchain mode (accepted risk, pending Pheonix's choice):** the job may execute any program in the system and
  toolchain folders, including shells. The boundary is the file, network and system-call confinement, not a list
  of programs.
* System folders such as `/etc` are readable (as for any program). Secrets kept there by an administrator are not
  hidden.
* No loopback network: tests that start a local server or database fail.
* `socket(AF_UNIX)` is refused. `socketpair` and pipes work, so build tools and test runners that use them are fine.
* Cargo works offline from crates already unpacked in `~/.cargo/registry`; a crate that was only downloaded but
  never unpacked cannot be unpacked inside the sandbox. Build once outside the IDE first.
* Python packages in a user site under `~/.local/lib` are readable; virtual environments under the project
  (`.venv`) are used first.
* macOS (Seatbelt) is not planned until there is a Mac to test on.
* **Windows, toolchain mode:** the job may start other programs, including `cmd.exe` (npm scripts need it), and can
  write and run files in its job folder and cache (cargo and go run the test programs they build). TandemForge's
  strict child-process policy would stop cargo and npm.
* **Windows grants change folder permissions for a moment:** during a run, the toolchain folders the program comes
  from (for example `.rustup`) carry an extra read-and-execute entry for the sandbox's own SID; it is removed when
  the run ends. A large toolchain folder makes the first seconds of a run slower.
* **Windows, dependency folders are copied**, not linked (links need extra rights), so a large `node_modules` can
  hit the copy limits.
* **Windows, Cargo with dependencies** needs the person's crates linked into the cache, which needs Developer Mode
  (symbolic links). Without it, offline Cargo builds work only for projects without dependencies.
* Windows can read its own and the Program Files folders from any app container; secrets kept there are not hidden.
* The person's own uid runs the job (in its own user namespace). A kernel bug in namespaces, Landlock or seccomp is
  out of scope; keep the kernel updated.

## 7. Troubleshooting

| Message | Cause and fix |
|---|---|
| "user namespaces are turned off on this computer" | enable unprivileged user namespaces (`sysctl kernel.unprivileged_userns_clone=1` on Debian-based kernels, or on Ubuntu 24.04+ allow them for the IDE through an AppArmor profile); use the gateway sandbox meanwhile |
| "this Linux kernel has no Landlock" | a kernel 5.13+ with Landlock enabled (`lsm=...,landlock`) is needed |
| "the local sandbox is not built for this system yet" | macOS: use the gateway sandbox |
| "the Windows sandbox (AppContainer) could not be started here" | Windows older than 10 version 1703, or app containers are blocked by policy; use the gateway sandbox |
| "only .exe programs can be started on Windows" | `.bat`, `.cmd` and scripts are refused; run the interpreter instead, for example `node script.js` |
| "a folder could not be shared with the sandbox" | `icacls` could not add the grant (for example a folder on a drive without permissions, such as FAT32 or exFAT); move the toolchain or the IDE's data to an NTFS drive |
| Windows: npm or node fails with `MODULE_NOT_FOUND` (or another program cannot read its own files) | the program lives in a Program Files folder whose installer did not allow app containers (Node.js does this), and without admin rights the IDE cannot add the grant. An administrator can allow every app container to read that folder once: `icacls "C:\Program Files\nodejs" /grant "*S-1-15-2-1:(OI)(CI)(RX)"` (S-1-15-2-1 is ALL APPLICATION PACKAGES, read and execute only); or install Node.js for one user (nvm, Volta, or the zip in a folder of your own), which the IDE can share itself |
| "the program's folder cannot be shared with the sandbox" | the program lies directly in the home folder or a drive root; install it in its own folder |
| "the program was not found on this computer" | install it, or put its folder on PATH (a toolchain folder or a system folder) |
| "give the program by name ..." | type `python3`, not `/usr/bin/python3` |
| "the folder is too large for a local run" | over 20 000 files or 512 MiB after skipping build folders |
| Result "the sandbox could not be set up" (exit 126) | a confinement step failed; nothing ran |
| Cargo: "failed to unpack" or "no matching package" offline | build once outside the IDE so the crates are unpacked in `~/.cargo/registry` |

## 8. Test evidence

* [Core: fmt, clippy, 68 unit + 5 + 2 tests incl. `local_runs_are_confined` on the real kernel (no network, home hidden, real folder untouched, background process gone)](../../HistoryTest/runs/2026-11-17_131847Z_core_e980a8c/RUN.md)
* [Real toolchains inside the sandbox: cargo twice (cache reused), go, npm, make, unittest: 6 passed, 0 failed, 0 skipped](../../HistoryTest/runs/2026-11-17_131900Z_local-toolchains_e980a8c/RUN.md)
* [Tauri shell: fmt and clippy with `runners_list` and `run_on`](../../HistoryTest/runs/2026-11-17_131926Z_shell_e980a8c/RUN.md)
* [Webview: UI smoke 77/77 with the runner picker and the local sandbox card text](../../HistoryTest/runs/2026-11-17_131928Z_ui-smoke_e980a8c/RUN.md)
* [Dependency audit: cargo audit (core 88, shell 539 crates) and npm audit, 0 vulnerabilities](../../HistoryTest/runs/2026-11-17_131937Z_security-scan_e980a8c/RUN.md)
* [Windows PC: the Linux code is left out and everything else builds; core 62 + 5 + 2 tests, webview, audit, shell clippy, release binary](../../HistoryTest/runs/2026-11-17_132300Z_windows_a8b439e/RUN.md)
