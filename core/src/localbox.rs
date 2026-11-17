//! localbox.rs -- the local OS sandbox runner (phase I5c, docs/features/run-and-test.md).
//!
//! Purpose: run the project's tests or one program on this computer, confined by the operating system, with the
//! same request and result as the gateway runner ([`crate::runner`]). A port of TandemForge's host runner
//! (`tandemforge.hostrun.confine`, isolation `host-os`) to Rust, in "toolchain mode" (Pheonix's I5 choice: local
//! runs must work for cargo, go, npm, make and Python tests, so the job may execute programs from the system
//! folders, the detected toolchain folders and its own copy, instead of TandemForge's per-file allow list).
//!
//! How a run works (Linux):
//!   1. A **snapshot** of the folder is copied into a fresh owner-only job folder (the agent deny list applies:
//!      `.env`, keys, `.git` ... are not copied). `node_modules` and `.venv` folders are linked read-only instead
//!      of copied. Build output goes to a per-folder cache (`target`, Go and npm caches) owned by the IDE.
//!   2. The program is found on PATH (absolute entries outside the open folder) or in the copy's `.venv`.
//!   3. The IDE forks; the child (the "trampoline") enters new **user, network, IPC, PID, UTS, cgroup and mount
//!      namespaces** (own uid mapped to itself, so no capabilities outside), then forks the job, which is PID 1
//!      of the new PID namespace: when it ends, the kernel ends everything it started. In the job: a private
//!      tmpfs on `/dev/shm` and a fresh `/proc`, **rlimits**, `no_new_privs`, **Landlock** (read and execute only
//!      the system folders, the toolchain folders and the linked folders; write only the job folder and the
//!      cache; no TCP bind or connect; no signals or abstract sockets outside the job), and **seccomp** (no
//!      ptrace, mount, new namespaces, kernel modules, bpf, io_uring, keyring, TIOCSTI, and `socket` only for
//!      IPv4/IPv6 inside the empty network namespace). Then `execve`.
//!   4. Output is read with caps; the run is killed at its time limit; the job folder is removed.
//!
//! Inputs and trust boundaries: the request was checked by `runner::check` (webview or model, untrusted); the
//! project's code is untrusted and runs only after confinement; its output is untrusted text (cleaned, capped).
//!
//! Security controls (with TandemForge's references): namespaces and an empty network namespace (NIST SC-7,
//! SC-39); Landlock allow-list of folders (CWE-22, CWE-284, NIST AC-3, AC-6); seccomp (CWE-269, CWE-668); rlimits
//! and a wall-clock kill (CWE-400, CWE-770); no shell between the IDE and the program, argv only (CWE-78); a
//! cleared environment (CWE-426, CWE-427, CWE-526); files copied without following links (CWE-59); secrets never
//! copied (CWE-200). Fail closed: any confinement step that fails ends the job with exit code 126 before the
//! program starts, and the runner is offered only when [`available`] says the kernel has every piece.
//!
//! Accepted risks (toolchain mode, documented in run-and-test.md): the job may execute any program in the system
//! and toolchain folders, including shells; reads of the system folders (`/etc`, `/usr` ...) are allowed;
//! loopback networking is off too, so tests that open a local server fail.
//!
//! Dependencies: libc (system calls; already in the lockfile through other crates), the core's workspace, agent
//! (deny list), runner and testparse modules.

use crate::runner::MAX_STREAM;
use crate::runner::{RunRequest, RunResult};
use crate::workspace::EntryKind;
use crate::workspace::Workspace;
use std::path::Path;
use std::path::PathBuf;

/// The runner's name in requests and results.
pub const RUNNER: &str = "local-os";
/// Most files and bytes copied into a snapshot, and the largest file.
const MAX_COPY_FILES: usize = 20_000;
const MAX_COPY_BYTES: u64 = 512 << 20;
const MAX_COPY_FILE: u64 = 100 << 20;
/// Folders linked read-only into the snapshot instead of copied (dependencies the tests need).
const LINK_DIRS: [&str; 3] = ["node_modules", ".venv", "venv"];
/// Folders never copied (build output and caches; the per-folder cache takes their place).
const SKIP_DIRS: [&str; 6] = [".git", "target", "dist", "__pycache__", ".pytest_cache", ".mypy_cache"];

/// Whether this runner can be offered here, and why not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Availability {
    pub available: bool,
    /// Fixed words when not available.
    pub reason: String,
}

/// A copy of the folder for one run, removed when dropped.
pub(crate) struct JobDir {
    pub(crate) path: PathBuf,
}

impl Drop for JobDir {
    fn drop(&mut self) {
        // SECURITY: remove_dir_all does not follow symlinks, so the read-only links to node_modules or .venv are
        // removed as links and their targets are untouched (CWE-59).
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// What a snapshot holds: the folders the job may read through links.
pub(crate) struct Snapshot {
    /// Canonical paths of linked folders (read and execute in the sandbox). Always empty on Windows (copied).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    linked: Vec<PathBuf>,
    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    /// A `.venv/bin` in the copy (Python frameworks use its interpreter first).
    venv_bin: Option<PathBuf>,
}

/// Copies the folder's files into `dest`: the agent deny list applies, links are not followed and build folders are
/// skipped. Dependency folders (`node_modules`, `.venv`) are linked read-only when `link_deps` is true (the Linux
/// local sandbox, which mounts nothing) and copied otherwise (Windows, and containers, where a host link would
/// point nowhere).
///
/// Security: every path comes from the workspace listing (confined to the root) and passes `agent::denied`, so
/// secrets never enter the job folder (CWE-200); symlinks in the project are not copied (CWE-59); counts and sizes
/// are capped (CWE-400).
pub(crate) fn snapshot(ws: &Workspace, dest: &Path, link_deps: bool) -> Result<Snapshot, String> {
    #[cfg(windows)]
    let _ = link_deps;
    let mut snap = Snapshot { linked: vec![], venv_bin: None };
    let (mut files, mut bytes) = (0usize, 0u64);
    let mut stack: Vec<String> = vec![String::new()];
    while let Some(dir_rel) = stack.pop() {
        let Ok(listing) = ws.list_dir(&dir_rel) else { continue };
        for e in listing.entries {
            if crate::agent::denied(&e.rel) {
                continue;
            }
            let target = dest.join(&e.rel);
            match e.kind {
                #[cfg(unix)]
                EntryKind::Dir if link_deps && LINK_DIRS.contains(&e.name.as_str()) => {
                    let Ok(real) = ws.root().join(&e.rel).canonicalize() else { continue };
                    // SECURITY: only a folder that really is inside the open folder is linked (CWE-22).
                    if !real.starts_with(ws.root()) {
                        continue;
                    }
                    std::os::unix::fs::symlink(&real, &target).map_err(|_| "could not prepare the run folder")?;
                    if e.name != "node_modules" && real.join("bin").is_dir() && dir_rel.is_empty() {
                        snap.venv_bin = Some(target.join("bin"));
                    }
                    snap.linked.push(real);
                }
                // Windows: links need extra rights there, so dependency folders are copied like any folder (within
                // the same caps); a virtual environment's programs are in `Scripts`.
                #[cfg(windows)]
                EntryKind::Dir if LINK_DIRS.contains(&e.name.as_str()) => {
                    std::fs::create_dir(&target).map_err(|_| "could not prepare the run folder")?;
                    if e.name != "node_modules" && dir_rel.is_empty() && ws.root().join(&e.rel).join("Scripts").is_dir()
                    {
                        snap.venv_bin = Some(target.join("Scripts"));
                    }
                    stack.push(e.rel);
                }
                EntryKind::Dir if SKIP_DIRS.contains(&e.name.as_str()) => {}
                EntryKind::Dir => {
                    std::fs::create_dir(&target).map_err(|_| "could not prepare the run folder")?;
                    stack.push(e.rel);
                }
                EntryKind::File => {
                    files += 1;
                    bytes += e.size;
                    if files > MAX_COPY_FILES || bytes > MAX_COPY_BYTES {
                        return Err("the folder is too large for a local run (20000 files, 512 MiB)".into());
                    }
                    if e.size > MAX_COPY_FILE {
                        continue;
                    }
                    let src = ws.root().join(&e.rel);
                    // SECURITY: copy only a regular file, checked without following links (CWE-59).
                    match std::fs::symlink_metadata(&src) {
                        Ok(m) if m.file_type().is_file() => {
                            std::fs::copy(&src, &target).map_err(|_| "could not copy the folder for the run")?;
                        }
                        _ => continue,
                    }
                }
                _ => {}
            }
        }
    }
    Ok(snap)
}

#[cfg(target_os = "linux")]
/// Folders below the home folder that hold toolchains (read and execute in the sandbox). Not the whole `.cargo`
/// or `.local`: `.cargo/credentials.toml` and `.local/share/keyrings` hold secrets (CWE-200, CWE-522).
const HOME_TOOLCHAINS: [&str; 14] = [
    ".cargo/bin",
    ".cargo/registry",
    ".cargo/git",
    ".rustup",
    "go/bin",
    "go/pkg/mod",
    "sdk",
    ".nvm",
    ".volta",
    ".pyenv",
    ".local/bin",
    ".local/lib",
    ".bun/bin",
    ".deno/bin",
];

#[cfg(target_os = "linux")]
/// Folders every program needs to read and run (libraries, interpreters, locale, certificates).
const SYSTEM_DIRS: [&str; 11] =
    ["/usr", "/lib", "/lib32", "/lib64", "/libx32", "/bin", "/sbin", "/etc", "/opt", "/nix/store", "/snap"];

#[cfg(target_os = "linux")]
/// Finds `name` on PATH: absolute entries only, never inside the open folder or the job folder, an existing file.
///
/// Security: a program planted in the project cannot be picked up by its name (CWE-426, CWE-427).
fn find_program(name: &str, path_var: &str, avoid: &[&Path]) -> Option<PathBuf> {
    if name.contains('/') {
        return None;
    }
    for dir in std::env::split_paths(path_var) {
        if !dir.is_absolute() || avoid.iter().any(|a| dir.starts_with(a)) {
            continue;
        }
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// The per-folder cache: build output and package caches survive between runs of the same folder.
pub(crate) fn cache_dir(data_dir: &Path, ws: &Workspace) -> Result<PathBuf, String> {
    let key = crate::projects::folder_key(ws.root());
    let short: String = key.chars().filter(char::is_ascii_alphanumeric).take(24).collect();
    let d = data_dir.join("run-cache").join(short);
    for sub in ["target", "go-build", "gopath", "npm", "xdg", "cargo-home"] {
        std::fs::create_dir_all(d.join(sub)).map_err(|_| "could not prepare the run cache")?;
    }
    restrict(&d);
    Ok(d)
}

#[cfg(windows)]
/// Windows: the IDE's data folder is below `%APPDATA%`, whose inherited ACL already allows only the person, the
/// system and administrators (CWE-276); the sandbox's own SID is added by `win::grant` where a job needs it.
fn restrict(_p: &Path) {}

#[cfg(unix)]
/// Makes a folder owner-only (0700). SECURITY: other users cannot read or plant files in it (CWE-276, CWE-732).
fn restrict(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
}

/// Reads at most `MAX_STREAM` bytes and drains the rest, so the program never blocks on a full pipe.
pub(crate) fn read_capped(mut r: impl std::io::Read) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut buf = [0u8; 16 << 10];
    let mut cut = false;
    loop {
        match r.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let room = MAX_STREAM.saturating_sub(out.len());
                out.extend_from_slice(&buf[..n.min(room)]);
                cut |= n > room;
            }
        }
    }
    (out, cut)
}

/// Removes control characters other than newline and tab (CWE-150).
pub(crate) fn clean(b: &[u8]) -> String {
    String::from_utf8_lossy(b).chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect()
}

#[cfg(target_os = "linux")]
mod linux {
    //! The confinement itself. Everything the forked children do is prepared here before `fork` and only uses
    //! system calls afterwards (no allocation, no locks), as `pre_exec` requires.

    use std::ffi::CString;
    use std::path::{Path, PathBuf};

    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_RULE_PATH_BENEATH: i32 = 1;
    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REFER: u64 = 1 << 13;
    const FS_TRUNCATE: u64 = 1 << 14;
    const FS_IOCTL_DEV: u64 = 1 << 15;
    const NET_BIND_TCP: u64 = 1 << 0;
    const NET_CONNECT_TCP: u64 = 1 << 1;
    const SCOPE_ABSTRACT_UNIX: u64 = 1 << 0;
    const SCOPE_SIGNAL: u64 = 1 << 1;

    /// `struct landlock_ruleset_attr`.
    #[repr(C)]
    struct RulesetAttr {
        fs: u64,
        net: u64,
        scoped: u64,
    }

    /// `struct landlock_path_beneath_attr` (packed in the kernel ABI).
    #[repr(C, packed)]
    struct PathBeneath {
        allowed: u64,
        parent_fd: i32,
    }

    /// The kernel's Landlock ABI version, or 0.
    pub fn landlock_abi() -> i64 {
        // SAFETY: a version query with a null attribute and size 0, as the Landlock documentation describes.
        let r = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<u8>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        r.max(0)
    }

    fn fs_rights(abi: i64) -> u64 {
        // Every access right of ABI 1 (bits 0-12), then the later ones.
        let mut r = (1u64 << 13) - 1;
        if abi >= 2 {
            r |= FS_REFER;
        }
        if abi >= 3 {
            r |= FS_TRUNCATE;
        }
        if abi >= 5 {
            r |= FS_IOCTL_DEV;
        }
        r
    }

    /// One Landlock rule, ready for the child: the path and the rights.
    pub struct Rule {
        path: CString,
        rights: u64,
        dir: bool,
        optional: bool,
    }

    /// Everything the children need, prepared before `fork`.
    pub struct Plan {
        abi: i64,
        rules: Vec<Rule>,
        uid_map: Vec<u8>,
        gid_map: Vec<u8>,
        seccomp: Vec<libc::sock_filter>,
        /// Seconds of CPU time and other limits.
        cpu_s: u64,
    }

    impl Plan {
        /// Builds the plan. `read_exec` folders may be read and run, `rw` folders read and changed (no execute
        /// there is not possible: build output must run, so they get execute too).
        pub fn new(read_exec: &[PathBuf], rw: &[PathBuf], cpu_s: u64) -> Result<Plan, &'static str> {
            let abi = landlock_abi();
            if abi < 1 {
                return Err("Landlock is not available in this kernel");
            }
            let all = fs_rights(abi);
            let read_exec_rights = FS_EXECUTE | FS_READ_FILE | FS_READ_DIR;
            let file_rights = FS_EXECUTE | FS_WRITE_FILE | FS_READ_FILE | if abi >= 3 { FS_TRUNCATE } else { 0 };
            let c =
                |p: &Path| CString::new(p.as_os_str().as_encoded_bytes()).map_err(|_| "a folder name is not usable");
            let mut rules = vec![];
            for d in super::SYSTEM_DIRS {
                rules.push(Rule { path: c(Path::new(d))?, rights: read_exec_rights, dir: true, optional: true });
            }
            for d in read_exec {
                rules.push(Rule { path: c(d)?, rights: read_exec_rights, dir: true, optional: false });
            }
            for d in rw {
                rules.push(Rule { path: c(d)?, rights: all, dir: true, optional: false });
            }
            for f in ["/dev/null", "/dev/zero", "/dev/full", "/dev/random", "/dev/urandom", "/dev/tty"] {
                rules.push(Rule {
                    path: c(Path::new(f))?,
                    rights: (FS_READ_FILE | FS_WRITE_FILE) & file_rights,
                    dir: false,
                    optional: true,
                });
            }
            // The job's own /dev/shm tmpfs and /proc (mounted fresh inside its namespaces; never the host's: the
            // mounts fail closed below if they cannot be made, and then these rules are not reachable).
            rules.push(Rule { path: c(Path::new("/dev/shm"))?, rights: all & !FS_EXECUTE, dir: true, optional: true });
            rules.push(Rule {
                path: c(Path::new("/proc"))?,
                rights: FS_READ_FILE | FS_READ_DIR,
                dir: true,
                optional: true,
            });
            // SECURITY: the job runs as the person's own uid inside its user namespace, never root (CWE-250).
            let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
            Ok(Plan {
                abi,
                rules,
                uid_map: format!("{uid} {uid} 1").into_bytes(),
                gid_map: format!("{gid} {gid} 1").into_bytes(),
                seccomp: seccomp_program()?,
                cpu_s,
            })
        }
    }

    // --- seccomp ------------------------------------------------------------------------------------------

    #[cfg(target_arch = "x86_64")]
    const AUDIT_ARCH: u32 = 0xC000_003E;
    #[cfg(target_arch = "aarch64")]
    const AUDIT_ARCH: u32 = 0xC000_00B7;
    const RET_ALLOW: u32 = 0x7FFF_0000;
    const RET_KILL_PROCESS: u32 = 0x8000_0000;
    const fn ret_errno(e: i32) -> u32 {
        0x0005_0000 | (e as u32 & 0xFFFF)
    }
    const LD_W_ABS: u16 = 0x20;
    const JEQ_K: u16 = 0x15;
    const JSET_K: u16 = 0x45;
    const JGE_K: u16 = 0x35;
    const RET_K: u16 = 0x06;
    const TIOCSTI: u32 = 0x5412;
    const NS_FLAGS: u32 = (libc::CLONE_NEWNS
        | libc::CLONE_NEWCGROUP
        | libc::CLONE_NEWUTS
        | libc::CLONE_NEWIPC
        | libc::CLONE_NEWUSER
        | libc::CLONE_NEWPID
        | libc::CLONE_NEWNET) as u32;

    /// System calls refused with EPERM: ones a build or test never needs that widen what a process can reach
    /// (TandemForge's list; CWE-269, CWE-668).
    fn denied_syscalls() -> Vec<i64> {
        let mut v = vec![
            libc::SYS_ptrace,
            libc::SYS_process_vm_readv,
            libc::SYS_process_vm_writev,
            libc::SYS_kexec_load,
            libc::SYS_kexec_file_load,
            libc::SYS_init_module,
            libc::SYS_finit_module,
            libc::SYS_delete_module,
            libc::SYS_bpf,
            libc::SYS_perf_event_open,
            libc::SYS_userfaultfd,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_keyctl,
            libc::SYS_add_key,
            libc::SYS_request_key,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_mount,
            libc::SYS_umount2,
            libc::SYS_pivot_root,
            libc::SYS_chroot,
            libc::SYS_open_by_handle_at,
            libc::SYS_name_to_handle_at,
            libc::SYS_open_tree,
            libc::SYS_move_mount,
            libc::SYS_fsopen,
            libc::SYS_fsconfig,
            libc::SYS_fsmount,
            libc::SYS_fspick,
            libc::SYS_mount_setattr,
            libc::SYS_quotactl,
            libc::SYS_swapon,
            libc::SYS_swapoff,
            libc::SYS_reboot,
            libc::SYS_acct,
            libc::SYS_vhangup,
        ];
        #[cfg(target_arch = "x86_64")]
        v.extend([libc::SYS_iopl, libc::SYS_ioperm, libc::SYS_uselib]);
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The BPF program: kill other architectures (and x32), EPERM the denied calls, ENOSYS clone3 (libc falls
    /// back to clone, whose flags can be checked), no new namespaces in clone, `socket` only for IPv4/IPv6
    /// (inside the empty network namespace; no Unix sockets such as the Docker or D-Bus socket), no TIOCSTI.
    fn seccomp_program() -> Result<Vec<libc::sock_filter>, &'static str> {
        enum J {
            Kill,
            Eperm,
            Enosys,
            Clone,
            Socket,
            Ioctl,
            Allow,
            Next,
        }
        // (code, jt, jf, k) with symbolic targets, then resolved.
        let mut p: Vec<(u16, J, J, u32)> = vec![];
        let mut labels: Vec<(&str, usize)> = vec![];
        p.push((LD_W_ABS, J::Next, J::Next, 4)); // arch
        p.push((JEQ_K, J::Next, J::Kill, AUDIT_ARCH));
        p.push((LD_W_ABS, J::Next, J::Next, 0)); // nr
        #[cfg(target_arch = "x86_64")]
        p.push((JGE_K, J::Kill, J::Next, 0x4000_0000)); // x32 ABI
        for n in denied_syscalls() {
            p.push((JEQ_K, J::Eperm, J::Next, n as u32));
        }
        p.push((JEQ_K, J::Enosys, J::Next, libc::SYS_clone3 as u32));
        p.push((JEQ_K, J::Clone, J::Next, libc::SYS_clone as u32));
        p.push((JEQ_K, J::Socket, J::Next, libc::SYS_socket as u32));
        p.push((JEQ_K, J::Ioctl, J::Next, libc::SYS_ioctl as u32));
        p.push((RET_K, J::Next, J::Next, RET_ALLOW));
        labels.push(("clone", p.len()));
        p.push((LD_W_ABS, J::Next, J::Next, 16)); // arg0 (flags)
        p.push((JSET_K, J::Eperm, J::Next, NS_FLAGS));
        p.push((RET_K, J::Next, J::Next, RET_ALLOW));
        labels.push(("socket", p.len()));
        p.push((LD_W_ABS, J::Next, J::Next, 16)); // arg0 (family)
        p.push((JEQ_K, J::Allow, J::Next, libc::AF_INET as u32));
        p.push((JEQ_K, J::Allow, J::Next, libc::AF_INET6 as u32));
        p.push((RET_K, J::Next, J::Next, ret_errno(libc::EACCES)));
        labels.push(("ioctl", p.len()));
        p.push((LD_W_ABS, J::Next, J::Next, 24)); // arg1 (request, low 32 bits)
        p.push((JEQ_K, J::Eperm, J::Next, TIOCSTI));
        labels.push(("allow", p.len()));
        p.push((RET_K, J::Next, J::Next, RET_ALLOW));
        labels.push(("eperm", p.len()));
        p.push((RET_K, J::Next, J::Next, ret_errno(libc::EPERM)));
        labels.push(("enosys", p.len()));
        p.push((RET_K, J::Next, J::Next, ret_errno(libc::ENOSYS)));
        labels.push(("kill", p.len()));
        p.push((RET_K, J::Next, J::Next, RET_KILL_PROCESS));
        let at = |n: &str| labels.iter().find(|l| l.0 == n).map(|l| l.1).unwrap_or(0);
        let mut out = vec![];
        for (i, (code, jt, jf, k)) in p.iter().enumerate() {
            let rel = |j: &J| -> Result<u8, &'static str> {
                let target = match j {
                    J::Next => return Ok(0),
                    J::Kill => at("kill"),
                    J::Eperm => at("eperm"),
                    J::Enosys => at("enosys"),
                    J::Clone => at("clone"),
                    J::Socket => at("socket"),
                    J::Ioctl => at("ioctl"),
                    J::Allow => at("allow"),
                };
                u8::try_from(target - (i + 1)).map_err(|_| "seccomp program jump out of range")
            };
            out.push(libc::sock_filter { code: *code, jt: rel(jt)?, jf: rel(jf)?, k: *k });
        }
        Ok(out)
    }

    // --- the children ----------------------------------------------------------------------------------------

    fn write_file(path: &[u8], data: &[u8]) -> bool {
        // SAFETY: `path` is NUL-terminated (a literal below); open/write/close are async-signal-safe.
        unsafe {
            let fd = libc::open(path.as_ptr().cast(), libc::O_WRONLY | libc::O_CLOEXEC);
            if fd < 0 {
                return false;
            }
            let ok = libc::write(fd, data.as_ptr().cast(), data.len()) == data.len() as isize;
            libc::close(fd);
            ok
        }
    }

    fn lower(res: libc::__rlimit_resource_t, v: u64) {
        // SAFETY: getrlimit/setrlimit on a stack struct; async-signal-safe.
        unsafe {
            let mut cur = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            libc::getrlimit(res, &mut cur);
            let v = if cur.rlim_max == libc::RLIM_INFINITY { v } else { v.min(cur.rlim_max) };
            let l = libc::rlimit { rlim_cur: v, rlim_max: v };
            libc::setrlimit(res, &l);
        }
    }

    /// Runs in the job (PID 1 of its namespace) just before `execve`. Fail closed: `_exit(126)`.
    fn confine_job(plan: &Plan) {
        // SAFETY: only system calls on prepared, NUL-terminated data; this process is single-threaded (forked).
        unsafe {
            let fail = || -> ! { libc::_exit(126) };
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong, 0, 0, 0);
            // A private mount tree: mounts below stay inside the job's mount namespace.
            if libc::mount(
                c"none".as_ptr(),
                c"/".as_ptr(),
                std::ptr::null(),
                libc::MS_REC | libc::MS_PRIVATE,
                std::ptr::null(),
            ) != 0
            {
                fail();
            }
            // SECURITY: a fresh /dev/shm, never the person's shared one (CWE-668), bounded (CWE-400).
            if libc::mount(
                c"p2pide-shm".as_ptr(),
                c"/dev/shm".as_ptr(),
                c"tmpfs".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                c"size=64m,nr_inodes=4096,mode=1777".as_ptr().cast(),
            ) != 0
            {
                fail();
            }
            // A /proc of the job's own PID namespace (other processes are not visible). Optional: when the
            // kernel refuses it (a masked /proc in some containers), /proc stays the host's and Landlock's /proc
            // rule is replaced by nothing below.
            let own_proc = libc::mount(
                c"proc".as_ptr(),
                c"/proc".as_ptr(),
                c"proc".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                std::ptr::null(),
            ) == 0;
            let gb = 1u64 << 30;
            lower(libc::RLIMIT_DATA, 8 * gb);
            lower(libc::RLIMIT_FSIZE, gb);
            lower(libc::RLIMIT_NOFILE, 4096);
            lower(libc::RLIMIT_CORE, 0);
            lower(libc::RLIMIT_CPU, plan.cpu_s);
            lower(libc::RLIMIT_NPROC, 1024);
            // --- Landlock ---
            let mut attr = RulesetAttr { fs: fs_rights(plan.abi), net: 0, scoped: 0 };
            let mut size = 8usize;
            if plan.abi >= 4 {
                // SECURITY: TCP bind and connect handled and never allowed (NIST SC-7).
                attr.net = NET_BIND_TCP | NET_CONNECT_TCP;
                size = 16;
            }
            if plan.abi >= 6 {
                // SECURITY: no signals to, or abstract sockets of, processes outside the job.
                attr.scoped = SCOPE_ABSTRACT_UNIX | SCOPE_SIGNAL;
                size = 24;
            }
            let rs = libc::syscall(libc::SYS_landlock_create_ruleset, &attr as *const RulesetAttr, size, 0u32);
            if rs < 0 {
                fail();
            }
            for r in &plan.rules {
                if !own_proc && r.path.as_bytes() == b"/proc" {
                    continue;
                }
                let mut flags = libc::O_PATH | libc::O_CLOEXEC;
                if r.dir {
                    flags |= libc::O_DIRECTORY;
                }
                let fd = libc::open(r.path.as_ptr(), flags);
                if fd < 0 {
                    if r.optional {
                        continue;
                    }
                    fail();
                }
                let pb = PathBeneath { allowed: r.rights & attr.fs, parent_fd: fd };
                let ok = libc::syscall(
                    libc::SYS_landlock_add_rule,
                    rs as i32,
                    LANDLOCK_RULE_PATH_BENEATH,
                    &pb as *const PathBeneath,
                    0u32,
                );
                libc::close(fd);
                if ok < 0 {
                    fail();
                }
            }
            // SECURITY: no_new_privs (needed by Landlock and seccomp; also stops setuid gains, CWE-250).
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                fail();
            }
            if libc::syscall(libc::SYS_landlock_restrict_self, rs as i32, 0u32) != 0 {
                fail();
            }
            libc::close(rs as i32);
            // --- seccomp ---
            let prog = libc::sock_fprog { len: plan.seccomp.len() as u16, filter: plan.seccomp.as_ptr() as *mut _ };
            if libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &prog as *const libc::sock_fprog, 0, 0) != 0
            {
                fail();
            }
        }
    }

    /// The `pre_exec` hook: enter the namespaces, fork the job, and turn this process into its waiting parent.
    ///
    /// Returns `Ok` only in the job (which then execs); the trampoline never returns (it `_exit`s with the job's
    /// status). Fails closed: a namespace that cannot be made ends the trampoline with 126.
    pub fn trampoline(plan: &Plan) -> std::io::Result<()> {
        // SAFETY: called by std in the forked child before execve; only system calls on prepared data.
        unsafe {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong, 0, 0, 0);
            let flags = libc::CLONE_NEWUSER
                | libc::CLONE_NEWNET
                | libc::CLONE_NEWIPC
                | libc::CLONE_NEWPID
                | libc::CLONE_NEWNS
                | libc::CLONE_NEWUTS
                | libc::CLONE_NEWCGROUP;
            if libc::unshare(flags) != 0 {
                libc::_exit(126);
            }
            if !write_file(b"/proc/self/setgroups\0", b"deny")
                || !write_file(b"/proc/self/uid_map\0", &plan.uid_map)
                || !write_file(b"/proc/self/gid_map\0", &plan.gid_map)
            {
                libc::_exit(126);
            }
            let pid = libc::fork();
            if pid < 0 {
                libc::_exit(126);
            }
            if pid == 0 {
                // The job: PID 1 of the new PID namespace.
                confine_job(plan);
                return Ok(());
            }
            // The trampoline: hold no descriptor but the standard ones (std's exec-error pipe must close when the
            // job execs, or the IDE would wait for the job's end to learn that it started).
            libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0u32);
            let mut status = 0;
            loop {
                let r = libc::waitpid(pid, &mut status, 0);
                if r == pid {
                    break;
                }
                if r < 0 && *libc::__errno_location() != libc::EINTR {
                    libc::_exit(126);
                }
            }
            if libc::WIFEXITED(status) {
                libc::_exit(libc::WEXITSTATUS(status));
            }
            libc::_exit(128 + libc::WTERMSIG(status));
        }
    }

    /// True when user namespaces can be made here (checked in a short-lived child).
    pub fn userns_ok() -> bool {
        // SAFETY: fork a child that only calls unshare and _exit; the parent waits for it.
        unsafe {
            let pid = libc::fork();
            if pid == 0 {
                let ok =
                    libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNET | libc::CLONE_NEWPID | libc::CLONE_NEWNS)
                        == 0;
                libc::_exit(if ok { 0 } else { 1 });
            }
            if pid < 0 {
                return false;
            }
            let mut st = 0;
            libc::waitpid(pid, &mut st, 0) == pid && libc::WIFEXITED(st) && libc::WEXITSTATUS(st) == 0
        }
    }
}

/// Whether the local OS sandbox can run here: Linux on x86_64 or aarch64 with Landlock and user namespaces.
/// Fail closed: when any piece is missing the runner is not offered (NIST SI-6).
pub fn available() -> Availability {
    #[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        if linux::landlock_abi() < 1 {
            return Availability { available: false, reason: "this Linux kernel has no Landlock".into() };
        }
        if !linux::userns_ok() {
            return Availability { available: false, reason: "user namespaces are turned off on this computer".into() };
        }
        Availability { available: true, reason: String::new() }
    }
    #[cfg(windows)]
    {
        // SECURITY: fail closed: a real confined run (cmd.exe exits with 7) must work before the runner is
        // offered; checked once per start of the IDE (NIST SI-6).
        static PROBE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let ok = *PROBE.get_or_init(|| {
            let root = std::env::var_os("SystemRoot").map(PathBuf::from).filter(|p| p.is_absolute());
            let root = root.unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
            let env = win_base_env();
            win::run_confined(
                "P2PAI.IDE.Probe",
                &root.join("System32").join("cmd.exe"),
                &["/d".into(), "/c".into(), "exit 7".into()],
                &env,
                &root,
                std::time::Duration::from_secs(20),
                20,
            )
            .is_ok_and(|o| o.exit_code == Some(7))
        });
        if ok {
            Availability { available: true, reason: String::new() }
        } else {
            Availability {
                available: false,
                reason: "the Windows sandbox (AppContainer) could not be started here".into(),
            }
        }
    }
    #[cfg(not(any(windows, all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))))]
    {
        Availability { available: false, reason: "the local sandbox is not built for this system yet".into() }
    }
}

/// Runs a checked request in the local OS sandbox.
///
/// * `data_dir` -- the IDE's data folder (job folders and the per-folder cache live below it).
/// * `stage` -- called with `copying` and `running`.
///
/// Errors are fixed messages. The result's `place` is `this computer`.
#[cfg(target_os = "linux")]
pub fn run(
    ws: &Workspace,
    req: &RunRequest,
    data_dir: &Path,
    stage: &mut dyn FnMut(&'static str),
) -> Result<RunResult, String> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let a = available();
    if !a.available {
        return Err(a.reason);
    }
    stage("copying");
    let cache = cache_dir(data_dir, ws)?;
    let jobs = data_dir.join("run-jobs");
    std::fs::create_dir_all(&jobs).map_err(|_| "could not prepare the run folder")?;
    restrict(&jobs);
    let job = JobDir { path: jobs.join(crate::store::new_id()) };
    // SECURITY: a new folder (create_dir fails if it exists), owner-only (CWE-377, CWE-732).
    std::fs::create_dir(&job.path).map_err(|_| "could not prepare the run folder")?;
    restrict(&job.path);
    let snap = snapshot(ws, &job.path, true)?;
    let home = job.path.join(".p2pide-home");
    let tmp = job.path.join(".p2pide-tmp");
    for d in [&home, &tmp] {
        std::fs::create_dir(d).map_err(|_| "could not prepare the run folder")?;
    }

    // The program and its arguments.
    let (argv, extra_env): (Vec<String>, Vec<(&str, &str)>) = if req.tool == "run_tests" {
        let (a, e) = crate::testparse::framework_command(&req.framework).ok_or("unknown test framework")?;
        (a.iter().map(|s| s.to_string()).collect(), e)
    } else {
        (req.argv.clone(), vec![])
    };
    let user_home = std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute());
    let mut toolchains: Vec<PathBuf> = vec![];
    if let Some(h) = &user_home {
        for t in HOME_TOOLCHAINS {
            if let Ok(p) = h.join(t).canonicalize() {
                if p.is_dir() && p.starts_with(h) && p != *h {
                    toolchains.push(p);
                }
            }
        }
    }
    // PATH for the job: the copy's .venv first, then the person's PATH entries that are system or toolchain
    // folders (absolute, outside the open folder), then the usual system folders.
    let user_path = std::env::var("PATH").unwrap_or_default();
    let mut path_dirs: Vec<PathBuf> = vec![];
    if let Some(v) = &snap.venv_bin {
        path_dirs.push(v.clone());
    }
    for d in std::env::split_paths(&user_path) {
        let ok_place = d.is_absolute()
            && !d.starts_with(ws.root())
            && (super_dirs_contain(&d)
                || toolchains.iter().any(|t| d.starts_with(t))
                || d.canonicalize().is_ok_and(|c| toolchains.iter().any(|t| c.starts_with(t))));
        if ok_place && !path_dirs.contains(&d) {
            path_dirs.push(d);
        }
    }
    for d in ["/usr/local/bin", "/usr/bin", "/bin"] {
        let d = PathBuf::from(d);
        if !path_dirs.contains(&d) {
            path_dirs.push(d);
        }
    }
    let job_path = std::env::join_paths(&path_dirs).map_err(|_| "a PATH folder is not usable")?;
    let program = if argv[0].contains('/') {
        return Err("give the program by name (it is looked up on PATH), not by path".into());
    } else {
        find_program(&argv[0], &job_path.to_string_lossy(), &[ws.root()])
            .or_else(|| snap.venv_bin.as_ref().map(|v| v.join(&argv[0])).filter(|p| p.is_file()))
            .ok_or("the program was not found on this computer")?
    };

    // What the job may reach.
    let mut read_exec = toolchains.clone();
    read_exec.extend(snap.linked.iter().cloned());
    if let Ok(real) = program.canonicalize() {
        if let Some(dir) = real.parent() {
            // The program's own folder (a toolchain outside the known ones). Never the home folder itself.
            if user_home.as_deref() != Some(dir) && !read_exec.iter().any(|r| dir.starts_with(r)) {
                read_exec.push(dir.to_path_buf());
            }
        }
    }
    let rw = vec![job.path.clone(), cache.clone()];
    let cpu_s = u64::from(req.timeout_s) * 8;
    let plan = linux::Plan::new(&read_exec, &rw, cpu_s)?;

    let cwd = if req.cwd.is_empty() { job.path.clone() } else { job.path.join(&req.cwd) };
    if !cwd.is_dir() {
        return Err("the folder to run in is not in the copy".into());
    }
    let mut cmd = Command::new(&program);
    cmd.args(&argv[1..]).current_dir(&cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // SECURITY: a cleared environment (CWE-526, CWE-426): only what tools need, pointing into the job and cache.
    cmd.env_clear();
    let cargo_home = cache.join("cargo-home");
    if let Some(h) = &user_home {
        // Cargo locks and writes below CARGO_HOME; the person's registry is linked read-only into the cache's
        // own CARGO_HOME, so builds work offline without write access to ~/.cargo.
        for sub in ["registry", "git"] {
            let link = cargo_home.join(sub);
            if std::fs::symlink_metadata(&link).is_err() && h.join(".cargo").join(sub).is_dir() {
                let _ = std::os::unix::fs::symlink(h.join(".cargo").join(sub), &link);
            }
        }
        if h.join(".rustup").is_dir() {
            cmd.env("RUSTUP_HOME", h.join(".rustup"));
        }
        if h.join("go/pkg/mod").is_dir() {
            cmd.env("GOMODCACHE", h.join("go/pkg/mod"));
        }
    }
    cmd.env("PATH", &job_path)
        .env("HOME", &home)
        .env("TMPDIR", &tmp)
        .env("LANG", "C.UTF-8")
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_TARGET_DIR", cache.join("target"))
        .env("GOCACHE", cache.join("go-build"))
        .env("GOPATH", cache.join("gopath"))
        .env("GOFLAGS", "-mod=mod")
        .env("npm_config_cache", cache.join("npm"))
        .env("XDG_CACHE_HOME", cache.join("xdg"))
        .env("CI", "true");
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    // SAFETY: the hook only runs system calls on data prepared above (see `linux::trampoline`).
    unsafe {
        cmd.pre_exec(move || linux::trampoline(&plan));
    }
    stage("running");
    let t = Instant::now();
    let mut child = cmd.spawn().map_err(|_| "the program could not be started in the sandbox")?;
    let out = child.stdout.take().map(|o| std::thread::spawn(move || read_capped(o)));
    let err = child.stderr.take().map(|e| std::thread::spawn(move || read_capped(e)));
    let limit = Duration::from_secs(u64::from(req.timeout_s));
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if t.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                // SECURITY: killing the trampoline kills the job (PDEATHSIG) and with it its whole PID namespace.
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            Err(_) => break None,
        }
    };
    let (so, c1) = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let (se, c2) = err.and_then(|h| h.join().ok()).unwrap_or_default();
    let exit_code = status.and_then(|s| s.code()).map(i64::from);
    let stdout = clean(&so);
    let stderr = clean(&se);
    let tests = if req.tool == "run_tests" { crate::testparse::parse(&req.framework, &stdout, &stderr) } else { None };
    Ok(RunResult {
        runner: RUNNER.into(),
        place: "this computer".into(),
        summary: req.summary(),
        exit_code,
        timed_out,
        duration_ms: t.elapsed().as_millis() as i64,
        stdout,
        stderr,
        truncated: c1 || c2,
        tests,
        error: if exit_code == Some(126) && !timed_out {
            "the sandbox could not be set up".into()
        } else {
            String::new()
        },
    })
}

#[cfg(windows)]
#[path = "localbox_win.rs"]
mod win;

#[cfg(windows)]
/// The variables Windows programs expect, copied from the IDE's own environment (no secrets: folder names and
/// machine facts only). `LOCALAPPDATA` is required: CreateProcessW refuses an AppContainer start without it.
fn win_base_env() -> Vec<(String, String)> {
    const KEEP: [&str; 16] = [
        "SystemRoot",
        "windir",
        "SystemDrive",
        "ComSpec",
        "PATHEXT",
        "OS",
        "NUMBER_OF_PROCESSORS",
        "PROCESSOR_ARCHITECTURE",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "CommonProgramW6432",
        "ProgramData",
        "LOCALAPPDATA",
    ];
    KEEP.iter().filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v))).collect()
}

#[cfg(windows)]
/// `p` without the `\\?\` prefix that `canonicalize` adds, when it is a plain drive path.
/// Also used by the container runner: Docker Desktop does not accept `\\?\` paths in `--volume`.
pub(crate) fn plain(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

#[cfg(windows)]
/// Lower-case text of a path, for prefix checks (Windows paths are case-insensitive).
fn low(p: &Path) -> String {
    let mut s = p.to_string_lossy().to_lowercase().replace('/', "\\");
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}

#[cfg(windows)]
/// True when `p` is `root` or below it (case-insensitive).
fn under(p: &Path, root: &Path) -> bool {
    let (p, r) = (low(p), low(root));
    p == r || p.starts_with(&format!("{r}\\"))
}

#[cfg(windows)]
/// Toolchain folders below the home folder, in groups: a group is shared with the sandbox when the program lies in
/// its first folder. Not the whole `.cargo`: `.cargo\credentials.toml` holds a secret (CWE-200, CWE-522).
const WIN_TOOLCHAINS: [&[&str]; 4] = [
    &[".cargo\\bin", ".rustup", ".cargo\\registry", ".cargo\\git"],
    &["go\\bin", "go\\pkg\\mod"],
    &[".bun\\bin"],
    &[".deno\\bin"],
];

#[cfg(windows)]
/// Finds `name` (an `.exe`, the extension optional) in `dirs`. `npm` and `npx` become `node.exe <cli.js>`, since
/// their `.cmd` launchers are refused; `python3` means `python.exe` first, then `python3.exe`.
///
/// Returns the program and the arguments to put before the request's own.
fn win_program(name: &str, dirs: &[PathBuf]) -> Result<(PathBuf, Vec<String>), String> {
    // SECURITY: a name only, never a path (CWE-426, CWE-427).
    if name.contains(['/', '\\', ':']) || name.starts_with('.') {
        return Err("give the program by name (it is looked up on PATH), not by path".into());
    }
    let find = |n: &str| dirs.iter().map(|d| d.join(n)).find(|p| p.is_file());
    let lower = name.to_lowercase();
    let stem = match Path::new(&lower).extension().map(|e| e.to_string_lossy().to_string()) {
        None => lower.clone(),
        Some(e) if e == "exe" => lower.trim_end_matches(".exe").to_string(),
        // SECURITY: .bat, .cmd, .com, .ps1 and scripts are not started (CWE-78, CWE-88).
        Some(_) => return Err("only .exe programs can be started on Windows".into()),
    };
    if stem == "npm" || stem == "npx" {
        let node = find("node.exe").ok_or("the program was not found on this computer")?;
        let dir = node.parent().map(Path::to_path_buf).unwrap_or_default();
        let cli = dir.join("node_modules").join("npm").join("bin").join(format!("{stem}-cli.js"));
        if !cli.is_file() {
            return Err("the program was not found on this computer".into());
        }
        return Ok((node, vec![cli.to_string_lossy().to_string()]));
    }
    // `python3`: the python.org installer names its program python.exe (python3.exe is usually a Store alias or
    // an MSYS2 build), so python.exe is tried first.
    if stem == "python3" {
        if let Some(p) = find("python.exe") {
            return Ok((p, vec![]));
        }
    }
    if let Some(p) = find(&format!("{stem}.exe")) {
        return Ok((p, vec![]));
    }
    Err("the program was not found on this computer".into())
}

/// Runs a checked request in the Windows sandbox (AppContainer and job object; see `localbox_win.rs`).
///
/// Same contract as the Linux `run`: errors are fixed messages and the result's `place` is `this computer`.
#[cfg(windows)]
pub fn run(
    ws: &Workspace,
    req: &RunRequest,
    data_dir: &Path,
    stage: &mut dyn FnMut(&'static str),
) -> Result<RunResult, String> {
    use std::time::{Duration, Instant};

    let a = available();
    if !a.available {
        return Err(a.reason);
    }
    stage("copying");
    let profile = win::profile_name(&crate::projects::folder_key(ws.root()));
    let sid = win::profile_sid(&profile)?;
    // SECURITY: grants outside the IDE's data folder are recorded and removed when this run ends (CWE-732).
    let mut grants = win::Grants::start(data_dir);
    let cache = cache_dir(data_dir, ws)?;
    // The cache is the folder's own and only its sandbox SID is given it; granted once (inheritance then covers
    // what builds write there). The marker only saves time: if a job deletes it, the grant is made again.
    let marker = cache.join(".sandbox-shared");
    if !marker.is_file() {
        win::grant(&cache, &sid, win::RIGHTS_MODIFY)?;
        let _ = std::fs::write(&marker, b"");
    }
    let jobs = data_dir.join("run-jobs");
    std::fs::create_dir_all(&jobs).map_err(|_| "could not prepare the run folder")?;
    let job = JobDir { path: jobs.join(crate::store::random_id()) };
    // SECURITY: a new folder (create_dir fails if it exists), shared with this sandbox only while it is still
    // empty, so the grant is quick and the copied files inherit it (CWE-377, CWE-732).
    std::fs::create_dir(&job.path).map_err(|_| "could not prepare the run folder")?;
    win::grant(&job.path, &sid, win::RIGHTS_MODIFY)?;
    let snap = snapshot(ws, &job.path, true)?;
    let home = job.path.join(".p2pide-home");
    let tmp = job.path.join(".p2pide-tmp");
    for d in [home.clone(), tmp.clone(), home.join("AppData").join("Roaming")] {
        std::fs::create_dir_all(&d).map_err(|_| "could not prepare the run folder")?;
    }

    let (argv, extra_env): (Vec<String>, Vec<(&str, &str)>) = if req.tool == "run_tests" {
        let (a, e) = crate::testparse::framework_command(&req.framework).ok_or("unknown test framework")?;
        (a.iter().map(|s| s.to_string()).collect(), e)
    } else {
        (req.argv.clone(), vec![])
    };
    let user_home = std::env::var_os("USERPROFILE").map(PathBuf::from).filter(|h| h.is_absolute());
    let program_files: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|k| std::env::var_os(k).map(PathBuf::from))
        .filter(|p| p.is_absolute())
        .collect();
    let groups: Vec<Vec<PathBuf>> = WIN_TOOLCHAINS
        .iter()
        .map(|g| match &user_home {
            Some(h) => g.iter().map(|t| h.join(t)).filter(|p| p.is_dir()).collect(),
            None => vec![],
        })
        .collect();
    // PATH: the copy's .venv\Scripts first, then the person's PATH folders that are system or toolchain folders
    // or hold a program (absolute, outside the open folder, never the Store's app aliases).
    let mut path_dirs: Vec<PathBuf> = vec![];
    if let Some(v) = &snap.venv_bin {
        path_dirs.push(v.clone());
    }
    for d in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let ok = d.is_absolute()
            && !under(&d, &plain(ws.root().to_path_buf()))
            && !low(&d).contains("\\windowsapps")
            && d.is_dir()
            && !path_dirs.iter().any(|p| low(p) == low(&d));
        if ok {
            path_dirs.push(d);
        }
    }
    let (program, mut pre) = win_program(&argv[0], &path_dirs)?;
    let mut args: Vec<String> = std::mem::take(&mut pre);
    args.extend(argv[1..].iter().cloned());

    // What the job may reach besides its folders: the toolchain group the program is in, and the program's own
    // folder when it is not the Windows folder or a toolchain folder. Never the home folder itself or a drive root.
    // The real folder (through links such as an nvm or scoop "current" link) is used, since Windows checks that.
    let prog_dir = program
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(|d| plain(d.to_path_buf())))
        .ok_or("the program was not found on this computer")?;
    let mut share: Vec<PathBuf> = vec![];
    for g in &groups {
        if g.first().is_some_and(|first| under(&prog_dir, first)) {
            share.extend(g.iter().cloned());
        }
    }
    if program.file_name().is_some_and(|n| n.eq_ignore_ascii_case("go.exe")) {
        if let Some(m) = groups.get(1).and_then(|g| g.iter().find(|p| low(p).ends_with("\\pkg\\mod"))) {
            share.push(m.clone());
        }
    }
    for s in &share {
        grants.add(s, &sid, win::RIGHTS_EXEC)?;
    }
    // Every app container may read the Windows folder. Program Files folders are readable only when their
    // installer allowed app containers (Go does; Node.js does not): the grant is tried, and when the person may
    // not change that folder's permissions (no admin rights) the run goes on and may fail to read it (see
    // Troubleshooting).
    let windows_dir = std::env::var_os("SystemRoot").map(PathBuf::from).filter(|p| p.is_absolute());
    let in_windows = windows_dir.is_some_and(|w| under(&prog_dir, &w));
    let in_program_files = program_files.iter().any(|r| under(&prog_dir, r));
    let in_shared = share.iter().any(|s| under(&prog_dir, s)) || under(&prog_dir, &job.path);
    if !in_windows && !in_shared {
        let is_home = user_home.as_ref().is_some_and(|h| under(h, &prog_dir));
        if is_home || prog_dir.components().count() < 3 {
            return Err("the program's folder cannot be shared with the sandbox".into());
        }
        if in_program_files {
            let _ = grants.add(&prog_dir, &sid, win::RIGHTS_EXEC);
        } else {
            grants.add(&prog_dir, &sid, win::RIGHTS_EXEC)?;
        }
    }

    let cwd = if req.cwd.is_empty() { job.path.clone() } else { job.path.join(&req.cwd) };
    if !cwd.is_dir() {
        return Err("the folder to run in is not in the copy".into());
    }
    let cargo_home = cache.join("cargo-home");
    if let Some(h) = &user_home {
        // Cargo writes its locks below CARGO_HOME; the person's registry is linked into the cache's own one
        // (read only through the grant above). Links need Developer Mode on Windows; without it, offline builds
        // work only for projects without dependencies (documented).
        for sub in ["registry", "git"] {
            let link = cargo_home.join(sub);
            if std::fs::symlink_metadata(&link).is_err() && h.join(".cargo").join(sub).is_dir() {
                let _ = std::os::windows::fs::symlink_dir(h.join(".cargo").join(sub), &link);
            }
        }
    }
    // SECURITY: the whole environment is built here; nothing of the IDE's is inherited (CWE-526, CWE-426).
    let mut env = win_base_env();
    let s = |p: &Path| p.to_string_lossy().to_string();
    let job_path = std::env::join_paths(&path_dirs).map_err(|_| "a PATH folder is not usable")?;
    env.extend([
        ("PATH".to_string(), job_path.to_string_lossy().to_string()),
        ("USERPROFILE".into(), s(&home)),
        ("HOME".into(), s(&home)),
        ("APPDATA".into(), s(&home.join("AppData").join("Roaming"))),
        ("TEMP".into(), s(&tmp)),
        ("TMP".into(), s(&tmp)),
        ("CARGO_HOME".into(), s(&cargo_home)),
        ("CARGO_TARGET_DIR".into(), s(&cache.join("target"))),
        ("GOCACHE".into(), s(&cache.join("go-build"))),
        ("GOPATH".into(), s(&cache.join("gopath"))),
        ("GOFLAGS".into(), "-mod=mod".into()),
        ("npm_config_cache".into(), s(&cache.join("npm"))),
        ("CI".into(), "true".into()),
    ]);
    if let Some(h) = &user_home {
        if h.join(".rustup").is_dir() {
            env.push(("RUSTUP_HOME".into(), s(&h.join(".rustup"))));
        }
        if h.join("go").join("pkg").join("mod").is_dir() {
            env.push(("GOMODCACHE".into(), s(&h.join("go").join("pkg").join("mod"))));
        }
    }
    for (k, v) in extra_env {
        env.push((k.to_string(), v.to_string()));
    }
    stage("running");
    let t = Instant::now();
    let limit = Duration::from_secs(u64::from(req.timeout_s));
    let outcome = win::run_confined(&profile, &program, &args, &env, &cwd, limit, u64::from(req.timeout_s) * 8);
    win::clear_container_folder(&sid);
    drop(grants);
    let o = outcome?;
    let stdout = clean(&o.stdout);
    let stderr = clean(&o.stderr);
    let tests = if req.tool == "run_tests" { crate::testparse::parse(&req.framework, &stdout, &stderr) } else { None };
    Ok(RunResult {
        runner: RUNNER.into(),
        place: "this computer".into(),
        summary: req.summary(),
        exit_code: o.exit_code,
        timed_out: o.timed_out,
        duration_ms: t.elapsed().as_millis() as i64,
        stdout,
        stderr,
        truncated: o.cut,
        tests,
        error: String::new(),
    })
}

/// Not built for this system yet (macOS later).
#[cfg(not(any(target_os = "linux", windows)))]
pub fn run(
    _ws: &Workspace,
    _req: &RunRequest,
    _data_dir: &Path,
    _stage: &mut dyn FnMut(&'static str),
) -> Result<RunResult, String> {
    Err(available().reason)
}

#[cfg(target_os = "linux")]
/// True for a folder under one of the system folders.
fn super_dirs_contain(d: &Path) -> bool {
    SYSTEM_DIRS.iter().any(|s| d.starts_with(s))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::runner::{check, RunInput};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-lb-{tag}-{}", crate::store::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn project() -> (Workspace, PathBuf) {
        let d = tmp("ws");
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::create_dir_all(d.join("tests")).unwrap();
        std::fs::write(d.join("src/calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
        std::fs::write(
            d.join("tests/test_calc.py"),
            "import sys, unittest\nsys.path.insert(0, 'src')\nfrom calc import add\n\n\nclass T(unittest.TestCase):\n    \
             def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n\n    def test_zero(self):\n        \
             self.assertEqual(add(0, 0), 0)\n",
        )
        .unwrap();
        std::fs::write(d.join("tests/__init__.py"), "").unwrap();
        std::fs::write(d.join(".env"), "API_KEY=local-do-not-copy\n").unwrap();
        (Workspace::open(&d).unwrap(), d)
    }

    fn py(ws: &Workspace, code: &str, timeout: Option<u32>) -> RunInput {
        let _ = ws;
        RunInput {
            kind: "command".into(),
            argv: vec!["python3".into(), "-c".into(), code.into()],
            timeout_s: timeout,
            ..Default::default()
        }
    }

    /// The sandbox end to end with the real kernel, when this computer has python3, Landlock and user namespaces.
    #[test]
    fn local_runs_are_confined() {
        if !available().available || find_program("python3", &std::env::var("PATH").unwrap_or_default(), &[]).is_none()
        {
            eprintln!("skipped: no local sandbox or python3 here");
            return;
        }
        let (w, d) = project();
        let data = tmp("data");
        let run = |input: RunInput| run(&w, &check(&w, &input).unwrap(), &data, &mut |_| {}).unwrap();

        // Tests: one of two fails, and the copy's change does not touch the real folder.
        let r = run(RunInput { kind: "tests".into(), framework: "unittest".into(), ..Default::default() });
        let t = r.tests.clone().unwrap_or_default();
        assert_eq!((t.passed, t.failed), (1, 1), "{}\n{}", r.stdout, r.stderr);
        assert_eq!(r.runner, RUNNER);

        // A secret outside the granted folders cannot be read; .env was not copied.
        let secret_dir = tmp("secret");
        std::fs::write(secret_dir.join("key"), "s3cret").unwrap();
        let code = format!(
            "import os\nfor p in [{:?}, '.env', os.path.expanduser('~/.ssh')]:\n    \
             try:\n        open(p).read(); print('READ', p)\n    except OSError as e:\n        print('no', type(e).__name__)\n",
            secret_dir.join("key").to_string_lossy()
        );
        let r = run(py(&w, &code, None));
        assert!(!r.stdout.contains("READ"), "SECURITY: read outside the sandbox: {}{}", r.stdout, r.stderr);

        // Writes only inside the job; nothing reaches the real folder or /tmp.
        let code = "import os\nfor p in ['/tmp/p2pide-escape', 'src/calc.py']:\n    \
            try:\n        open(p, 'w').write('x'); print('WROTE', p)\n    except OSError as e:\n        print('no', p)\n";
        let r = run(py(&w, code, None));
        assert!(r.stdout.contains("WROTE src/calc.py") && !r.stdout.contains("WROTE /tmp"), "{}{}", r.stdout, r.stderr);
        assert!(!Path::new("/tmp/p2pide-escape").exists());
        assert!(
            std::fs::read_to_string(d.join("src/calc.py")).unwrap().contains("a - b"),
            "the real file is untouched"
        );

        // SECURITY: no network, no Unix sockets, no ptrace (NIST SC-7, CWE-668, CWE-269).
        let code = "import socket, ctypes\ntry:\n    socket.create_connection(('1.1.1.1', 53), 2); print('NET')\n\
            except OSError: print('no net')\ntry:\n    socket.socket(socket.AF_UNIX); print('UNIX')\nexcept OSError: print('no unix')\n\
            print('ptrace', ctypes.CDLL(None, use_errno=True).ptrace(0, 0, 0, 0))\n";
        let r = run(py(&w, code, None));
        assert!(
            r.stdout.contains("no net") && r.stdout.contains("no unix") && r.stdout.contains("ptrace -1"),
            "{}{}",
            r.stdout,
            r.stderr
        );

        // The job is PID 1 of its own namespace; a process it leaves behind ends with it.
        let code = "import os, subprocess\nprint('pid', os.getpid())\nsubprocess.Popen(['sleep', '97'])\n";
        let r = run(py(&w, code, None));
        assert!(r.stdout.contains("pid 1"), "{}{}", r.stdout, r.stderr);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let left = std::fs::read_dir("/proc")
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| std::fs::read(e.path().join("cmdline")).is_ok_and(|c| c == b"sleep\x0097\x00"));
        assert!(!left, "SECURITY: a process outlived its run (CWE-404)");

        // The time limit stops a run.
        let r = run(py(&w, "import time; time.sleep(60)", Some(10)));
        assert!(r.timed_out && !r.ok());
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&secret_dir);
    }

    #[test]
    fn the_seccomp_program_is_well_formed() {
        if !available().available {
            return;
        }
        let p = linux::Plan::new(&[], &[], 10);
        assert!(p.is_ok());
    }
}

#[cfg(all(test, windows))]
mod win_tests {
    use super::*;
    use crate::runner::{check, RunInput};

    fn cmd(line: &str, timeout: Option<u32>) -> RunInput {
        RunInput {
            kind: "command".into(),
            argv: vec!["cmd".into(), "/d".into(), "/c".into(), line.into()],
            timeout_s: timeout,
            ..Default::default()
        }
    }

    /// The Windows sandbox end to end on the real system: AppContainer, job object and folder grants. Uses only
    /// programs from the Windows folder (cmd.exe, curl.exe), which every app container may run.
    #[test]
    fn local_runs_are_confined_on_windows() {
        let a = available();
        if !a.available {
            eprintln!("skipped: {}", a.reason);
            return;
        }
        let base = std::env::temp_dir().join(format!("p2pide-lbw-{}", crate::store::random_id()));
        let (proj, data, outside) = (base.join("ws"), base.join("data"), base.join("outside"));
        for d in [&proj, &data, &outside] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(proj.join("hello.txt"), "hello from the copy\n").unwrap();
        // A busy loop that never ends by itself (ping cannot be used to wait: the sandbox has no loopback).
        std::fs::write(proj.join("loop.cmd"), "@for /l %%i in (1,0,2) do @rem\r\n").unwrap();
        std::fs::write(proj.join(".env"), "API_KEY=local-do-not-copy\n").unwrap();
        let secret = outside.join("secret.txt");
        std::fs::write(&secret, "outside-secret\n").unwrap();
        let ws = Workspace::open(&proj).unwrap();
        let go = |input: RunInput| {
            let req = check(&ws, &input).unwrap();
            run(&ws, &req, &data, &mut |_| {}).unwrap()
        };

        // Reads and writes inside the copy work; the deny list kept .env out.
        let r = go(cmd(
            "type hello.txt && echo made> made.txt && type made.txt && if exist .env (echo ENV) else (echo NOENV)",
            None,
        ));
        assert_eq!(r.exit_code, Some(0), "{r:?}");
        assert!(
            r.stdout.contains("hello from the copy") && r.stdout.contains("made") && r.stdout.contains("NOENV"),
            "{r:?}"
        );
        assert_eq!(r.runner, "local-os");
        // The real folder is untouched.
        assert!(!proj.join("made.txt").exists());

        // A file outside the granted folders cannot be read (the person's own files stay hidden).
        let r = go(cmd(&format!("type \"{}\"", secret.display()), None));
        assert_ne!(r.exit_code, Some(0), "{r:?}");
        assert!(!r.stdout.contains("outside-secret"), "{r:?}");

        // Writing outside is refused too.
        let r = go(cmd(&format!("echo x> \"{}\"", outside.join("planted.txt").display()), None));
        assert!(!outside.join("planted.txt").exists(), "{r:?}");

        // No network: not the internet, not this computer's loopback.
        let curl = std::env::var("SystemRoot").map(|r| Path::new(&r).join("System32").join("curl.exe")).unwrap();
        if curl.is_file() {
            for url in ["http://1.1.1.1/", "http://127.0.0.1:9/"] {
                let r = go(RunInput {
                    kind: "command".into(),
                    argv: vec!["curl".into(), "-s".into(), "-m".into(), "5".into(), url.into()],
                    ..Default::default()
                });
                assert_ne!(r.exit_code, Some(0), "{url}: {r:?}");
            }
        }

        // A background program does not outlive the run, and the run does not wait for it.
        let t = std::time::Instant::now();
        let r = go(cmd("start /b cmd /d /c loop.cmd & echo started", None));
        assert!(r.stdout.contains("started"), "{r:?}");
        assert!(t.elapsed().as_secs() < 30, "the run waited for the background program");

        // The time limit ends the run.
        let r = go(cmd("loop.cmd", Some(10)));
        assert!(r.timed_out, "{r:?}");

        // .bat and .cmd programs are refused; a path is refused.
        for p in ["build.cmd", "x.bat", r"C:\Windows\System32\cmd.exe"] {
            let req =
                check(&ws, &RunInput { kind: "command".into(), argv: vec![p.into()], ..Default::default() }).unwrap();
            assert!(run(&ws, &req, &data, &mut |_| {}).is_err(), "{p}");
        }

        // No grant outside the data folder is left behind.
        assert!(!data.join("run-grants.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
