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

#[cfg(target_os = "linux")]
use crate::runner::MAX_STREAM;
use crate::runner::{RunRequest, RunResult};
#[cfg(target_os = "linux")]
use crate::workspace::EntryKind;
use crate::workspace::Workspace;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

/// The runner's name in requests and results.
pub const RUNNER: &str = "local-os";
/// Most files and bytes copied into a snapshot, and the largest file.
#[cfg(target_os = "linux")]
const MAX_COPY_FILES: usize = 20_000;
#[cfg(target_os = "linux")]
const MAX_COPY_BYTES: u64 = 512 << 20;
#[cfg(target_os = "linux")]
const MAX_COPY_FILE: u64 = 100 << 20;
/// Folders linked read-only into the snapshot instead of copied (dependencies the tests need).
#[cfg(target_os = "linux")]
const LINK_DIRS: [&str; 3] = ["node_modules", ".venv", "venv"];
/// Folders never copied (build output and caches; the per-folder cache takes their place).
#[cfg(target_os = "linux")]
const SKIP_DIRS: [&str; 6] = [".git", "target", "dist", "__pycache__", ".pytest_cache", ".mypy_cache"];

/// Whether this runner can be offered here, and why not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Availability {
    pub available: bool,
    /// Fixed words when not available.
    pub reason: String,
}

#[cfg(target_os = "linux")]
/// A copy of the folder for one run, removed when dropped.
struct JobDir {
    path: PathBuf,
}

#[cfg(target_os = "linux")]
impl Drop for JobDir {
    fn drop(&mut self) {
        // SECURITY: remove_dir_all does not follow symlinks, so the read-only links to node_modules or .venv are
        // removed as links and their targets are untouched (CWE-59).
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(target_os = "linux")]
/// What a snapshot holds: the folders the job may read through links.
struct Snapshot {
    /// Canonical paths of linked folders (read and execute in the sandbox).
    linked: Vec<PathBuf>,
    /// A `.venv/bin` in the copy (Python frameworks use its interpreter first).
    venv_bin: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
/// Copies the folder's files into `dest`: the agent deny list applies, links are not followed, build folders are
/// skipped and dependency folders are linked read-only.
///
/// Security: every path comes from the workspace listing (confined to the root) and passes `agent::denied`, so
/// secrets never enter the job folder (CWE-200); symlinks in the project are not copied (CWE-59); counts and sizes
/// are capped (CWE-400).
fn snapshot(ws: &Workspace, dest: &Path) -> Result<Snapshot, String> {
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
                EntryKind::Dir if LINK_DIRS.contains(&e.name.as_str()) => {
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

#[cfg(target_os = "linux")]
/// The per-folder cache: build output and package caches survive between runs of the same folder.
fn cache_dir(data_dir: &Path, ws: &Workspace) -> Result<PathBuf, String> {
    let key = crate::projects::folder_key(ws.root());
    let short: String = key.chars().filter(char::is_ascii_alphanumeric).take(24).collect();
    let d = data_dir.join("run-cache").join(short);
    for sub in ["target", "go-build", "gopath", "npm", "xdg", "cargo-home"] {
        std::fs::create_dir_all(d.join(sub)).map_err(|_| "could not prepare the run cache")?;
    }
    restrict(&d);
    Ok(d)
}

#[cfg(target_os = "linux")]
/// Makes a folder owner-only (0700). SECURITY: other users cannot read or plant files in it (CWE-276, CWE-732).
fn restrict(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
}

#[cfg(target_os = "linux")]
/// Reads at most `MAX_STREAM` bytes and drains the rest, so the program never blocks on a full pipe.
fn read_capped(mut r: impl std::io::Read) -> (Vec<u8>, bool) {
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

#[cfg(target_os = "linux")]
/// Removes control characters other than newline and tab (CWE-150).
fn clean(b: &[u8]) -> String {
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
    #[cfg(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64"))))]
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
    let snap = snapshot(ws, &job.path)?;
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

/// Not built for this system yet (Windows comes next; macOS later).
#[cfg(not(target_os = "linux"))]
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
