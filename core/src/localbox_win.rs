//! localbox_win.rs -- the Windows half of the local OS sandbox (phase I5c, docs/features/local-sandbox.md).
//!
//! Purpose: run one program for [`super::run`] inside an **AppContainer** and a **Job Object**. A Rust port of
//! TandemForge's `tandemforge.hostrun.winsandbox` (isolation `host-os` on Windows), in toolchain mode: the job may
//! start other programs (cargo starts rustc and the test binary, `npm test` starts `cmd.exe` and node), which
//! TandemForge's child-process policy forbids.
//!
//! How it works:
//!   1. **Profile.** One AppContainer profile per open folder (`P2PAI.IDE.Run.<24 hex>`), created with no
//!      capabilities: no `internetClient`, `internetClientServer` or `privateNetworkClientServer`, so outbound, LAN
//!      and loopback connections all fail (NIST SC-7, CWE-918). Its own SID is `S-1-15-2-...`.
//!   2. **Grants.** The container reaches only what Windows gives every app container (reading the Windows and
//!      Program Files folders) plus what is granted to its SID with `icacls` (argument list, no shell): modify on
//!      the job folder and the folder's cache, read and execute on the toolchain folders and the program's own
//!      folder. Grants outside the IDE's data folder are recorded in `run-grants.json` before they are made and
//!      removed when the run ends, or at the next run after a crash (CWE-732, NIST AC-6).
//!   3. **Process.** `CreateProcessW` with `STARTUPINFOEX`: the security-capabilities attribute (the container)
//!      and an explicit inherited-handle list holding only the three pipe handles (CWE-403). Created suspended,
//!      put in the job, then resumed, so it never runs outside the job. No window (CREATE_NO_WINDOW).
//!   4. **Job object.** Kill on close, die on unhandled exception, at most 1024 processes, 8 GiB for the whole job,
//!      a CPU time limit, and every UI restriction (no clipboard, desktop switch, global atoms, display or system
//!      settings, exit Windows, foreign USER handles) (CWE-400, CWE-770). At the end the whole job is terminated,
//!      so nothing the program started outlives the run (CWE-404).
//!   5. **Only `.exe` programs.** `.bat` and `.cmd` files are run by `cmd.exe` with its own argument parsing (the
//!      "BatBadBut" class), so they are refused; `npm` is started as `node.exe npm-cli.js` instead (CWE-78, CWE-88).
//!   6. **Clean up.** Windows points TEMP, TMP and LOCALAPPDATA of a contained process at the container's own
//!      folder; it is emptied after every run so no run leaves data for the next one (CWE-459, NIST SC-4).
//!
//! Inputs and trust boundaries: the program, arguments and environment were prepared by `super::run` from a
//! request checked by `runner::check`; the job itself is untrusted.
//!
//! Dependencies: windows-sys 0.61 (raw Win32 bindings from Microsoft; already in the lockfile through other
//! crates) and `icacls.exe` from the Windows system folder.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, SetHandleInformation, GENERIC_READ, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
    WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName, GetAppContainerFolderPath,
};
use windows_sys::Win32::Security::{FreeSid, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_JOB_TIME, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, InitializeProcThreadAttributeList, ResumeThread,
    TerminateProcess, UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

/// Profile names start with this; the rest is the first 24 hex characters of the folder key.
const PROFILE_PREFIX: &str = "P2PAI.IDE.Run.";
/// Where the grants outside the IDE's data folder are recorded until they are removed.
const GRANTS_FILE: &str = "run-grants.json";
/// HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS).
const HR_ALREADY_EXISTS: i32 = 0x800700B7_u32 as i32;
/// Every JOB_OBJECT_UILIMIT_* flag.
const UILIMIT_ALL: u32 = 0xFF;
/// At most this many processes in a job at once (as `RLIMIT_NPROC` on Linux).
const MAX_PROCESSES: u32 = 1024;
/// Memory for the whole job (as `RLIMIT_DATA` on Linux).
const JOB_MEMORY: usize = 8 << 30;

/// Rights for `icacls`: M = modify (read, write, execute, delete), RX = read and execute.
pub(super) const RIGHTS_MODIFY: &str = "M";
pub(super) const RIGHTS_EXEC: &str = "RX";

/// What one confined program run gave back.
pub(super) struct Outcome {
    pub exit_code: Option<i64>,
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub cut: bool,
}

/// UTF-16 with a closing NUL, for the W functions.
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// A Windows string returned by the system, read up to its NUL.
///
/// SAFETY: `p` must point at a NUL-terminated UTF-16 string.
unsafe fn from_wide(p: *const u16) -> String {
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

/// The AppContainer profile name for a folder key: stable, at most 64 characters, letters, digits and dots only.
pub(super) fn profile_name(folder_key: &str) -> String {
    let short: String = folder_key.chars().filter(char::is_ascii_hexdigit).take(24).collect();
    format!("{PROFILE_PREFIX}{short}")
}

/// One argument quoted the way the Microsoft C runtime splits a command line again (CWE-88).
///
/// Rules: an argument without spaces, tabs or quotes is kept as it is (and an empty one becomes `""`); otherwise it
/// is wrapped in quotes, each quote is escaped with a backslash, and backslashes are doubled only where they come
/// before a quote (inside or at the closing quote).
pub(super) fn quote_arg(a: &str, out: &mut String) {
    if !a.is_empty() && !a.contains([' ', '\t', '\n', '\x0b', '"']) {
        out.push_str(a);
        return;
    }
    out.push('"');
    let mut backslashes = 0usize;
    for c in a.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
}

/// The whole command line: the program path in quotes (it cannot hold a quote), then each argument quoted.
///
/// Errors: NUL anywhere, or a quote in the program path (CWE-158, CWE-88).
pub(super) fn command_line(program: &Path, args: &[String]) -> Result<String, &'static str> {
    let p = program.to_string_lossy();
    if p.contains(['"', '\0']) || args.iter().any(|a| a.contains('\0')) {
        return Err("the command line is invalid");
    }
    let mut s = format!("\"{p}\"");
    for a in args {
        s.push(' ');
        quote_arg(a, &mut s);
    }
    Ok(s)
}

/// A `CREATE_UNICODE_ENVIRONMENT` block: `NAME=value\0` sorted by name (case-insensitive), then `\0`.
///
/// Errors: an empty name, a name with `=` or NUL, or a value with NUL (CWE-158).
pub(super) fn env_block(env: &[(String, String)]) -> Result<Vec<u16>, &'static str> {
    let mut items: Vec<&(String, String)> = env.iter().collect();
    if items.iter().any(|(k, v)| k.is_empty() || k.contains(['=', '\0']) || v.contains('\0')) {
        return Err("the job environment is invalid");
    }
    items.sort_by_key(|(k, _)| k.to_uppercase());
    let mut out = Vec::new();
    for (k, v) in items {
        out.extend(format!("{k}={v}").encode_utf16());
        out.push(0);
    }
    out.push(0);
    Ok(out)
}

/// Creates (or reuses) the profile and returns its SID (to be freed with `FreeSid`) and its text form.
///
/// Security: the profile has no capabilities (no network, no libraries, no devices; NIST AC-6). Only a SID of the
/// AppContainer form `S-1-15-2-` is accepted (CWE-20).
fn ensure_profile(name: &str) -> Result<(PSID, String), String> {
    let n = wide(OsStr::new(name));
    let display = wide(OsStr::new("P2P-AI IDE run"));
    let desc = wide(OsStr::new("P2P-AI IDE local sandbox for test and command runs"));
    let mut sid: PSID = std::ptr::null_mut();
    // SAFETY: valid NUL-terminated strings; no capabilities (null, 0); `sid` receives an allocated SID.
    let mut hr = unsafe {
        CreateAppContainerProfile(n.as_ptr(), display.as_ptr(), desc.as_ptr(), std::ptr::null(), 0, &mut sid)
    };
    if hr == HR_ALREADY_EXISTS {
        // SAFETY: as above.
        hr = unsafe { DeriveAppContainerSidFromAppContainerName(n.as_ptr(), &mut sid) };
    }
    if hr != 0 || sid.is_null() {
        return Err("the sandbox profile could not be created".into());
    }
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: `sid` is a valid SID; `text` receives a LocalAlloc'd string, freed below.
    let ok = unsafe { ConvertSidToStringSidW(sid, &mut text) };
    if ok == 0 || text.is_null() {
        // SAFETY: the SID came from the calls above.
        unsafe { FreeSid(sid) };
        return Err("the sandbox profile could not be read".into());
    }
    // SAFETY: `text` is NUL-terminated; freed once.
    let s = unsafe {
        let s = from_wide(text);
        LocalFree(text as _);
        s
    };
    if !s.starts_with("S-1-15-2-") || !s.chars().all(|c| c.is_ascii_digit() || c == '-' || c == 'S') {
        // SAFETY: as above.
        unsafe { FreeSid(sid) };
        return Err("the sandbox profile is not valid".into());
    }
    Ok((sid, s))
}

/// The SID text of a profile (created when missing).
pub(super) fn profile_sid(name: &str) -> Result<String, String> {
    let (sid, s) = ensure_profile(name)?;
    // SAFETY: the SID came from `ensure_profile` and is freed once.
    unsafe { FreeSid(sid) };
    Ok(s)
}

/// `%SystemRoot%\System32\icacls.exe` (never looked up on PATH: CWE-426).
fn icacls() -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).filter(|p| p.is_absolute());
    root.unwrap_or_else(|| PathBuf::from(r"C:\Windows")).join("System32").join("icacls.exe")
}

/// Adds an inherited ACL entry for the container's SID to `path` (`icacls <path> /grant *SID:(OI)(CI)(<rights>)`).
///
/// Security: an argument list, no shell (CWE-78); the SID was checked to be an AppContainer SID; `rights` is one of
/// the two constants above.
pub(super) fn grant(path: &Path, sid: &str, rights: &str) -> Result<(), String> {
    let ok = std::process::Command::new(icacls())
        .arg(path)
        .arg("/grant")
        .arg(format!("*{sid}:(OI)(CI)({rights})"))
        .args(["/q", "/c"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        Ok(())
    } else {
        Err("a folder could not be shared with the sandbox".into())
    }
}

/// Removes every ACL entry of `sid` from `path` (and, through inheritance, below it).
fn revoke(path: &Path, sid: &str) {
    let _ = std::process::Command::new(icacls())
        .arg(path)
        .arg("/remove:g")
        .arg(format!("*{sid}"))
        .args(["/q", "/c"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// The grants a run made outside the IDE's data folder, recorded so a crash cannot leave them (CWE-732).
pub(super) struct Grants {
    state: PathBuf,
    done: Vec<(PathBuf, String)>,
}

impl Grants {
    /// Removes what an earlier run recorded and did not get to remove (after a crash), then starts empty.
    pub(super) fn start(data_dir: &Path) -> Grants {
        let state = data_dir.join(GRANTS_FILE);
        revoke_recorded(&state);
        Grants { state, done: vec![] }
    }

    /// Records, then grants `rights` on `path` to `sid`.
    pub(super) fn add(&mut self, path: &Path, sid: &str, rights: &str) -> Result<(), String> {
        self.done.push((path.to_path_buf(), sid.to_string()));
        let list: Vec<serde_json::Value> =
            self.done.iter().map(|(p, s)| serde_json::json!({"path": p, "sid": s})).collect();
        // SECURITY: written before the grant, so a crash in between is still undone at the next run.
        std::fs::write(&self.state, serde_json::to_vec(&list).unwrap_or_default())
            .map_err(|_| "could not record a folder shared with the sandbox")?;
        grant(path, sid, rights)
    }
}

impl Drop for Grants {
    fn drop(&mut self) {
        revoke_recorded(&self.state);
    }
}

/// Revokes every grant listed in `state` and deletes it.
fn revoke_recorded(state: &Path) {
    let Ok(b) = crate::store::read_capped(state, 256 << 10) else { return };
    if let Ok(serde_json::Value::Array(list)) = serde_json::from_slice::<serde_json::Value>(&b) {
        for g in list {
            let (Some(p), Some(s)) = (g["path"].as_str(), g["sid"].as_str()) else { continue };
            // SECURITY: only AppContainer SIDs are ever revoked, so a changed file cannot strip the person's
            // own access (CWE-20).
            if s.starts_with("S-1-15-2-") && s.chars().all(|c| c.is_ascii_digit() || c == '-' || c == 'S') {
                revoke(Path::new(p), s);
            }
        }
    }
    let _ = std::fs::remove_file(state);
}

/// Deletes everything inside `root` without following links or junctions; keeps the folders themselves.
fn clear_tree(root: &Path) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        if m.file_type().is_symlink() {
            // SECURITY: the link itself goes, never what it points at (CWE-59).
            let _ = std::fs::remove_file(&p).or_else(|_| std::fs::remove_dir(&p));
        } else if m.is_dir() {
            clear_tree(&p);
        } else {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// Empties the container's own folder (its TEMP, TMP and LOCALAPPDATA) after a run (CWE-459, NIST SC-4).
pub(super) fn clear_container_folder(sid: &str) {
    let s = wide(OsStr::new(sid));
    let mut out: *mut u16 = std::ptr::null_mut();
    // SAFETY: valid SID string; `out` receives a CoTaskMemAlloc'd path, freed below.
    let hr = unsafe { GetAppContainerFolderPath(s.as_ptr(), &mut out) };
    if hr != 0 || out.is_null() {
        return;
    }
    // SAFETY: NUL-terminated; freed once.
    let path = unsafe {
        let p = from_wide(out);
        CoTaskMemFree(out as _);
        p
    };
    let p = PathBuf::from(path);
    if p.is_absolute() && p.is_dir() {
        clear_tree(&p);
    }
}

/// Closes a handle when dropped.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: a handle this module opened, closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// A pipe whose write end the child inherits and whose read end stays with the IDE.
fn pipe() -> Result<(Owned, Owned), String> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let (mut r, mut w): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: out-pointers to local handles.
    if unsafe { CreatePipe(&mut r, &mut w, &sa, 0) } == 0 {
        return Err("the sandbox could not be set up".into());
    }
    let (r, w) = (Owned(r), Owned(w));
    // SECURITY: our read end is not inheritable, so the job holds only its write end (CWE-403).
    // SAFETY: a valid handle from CreatePipe.
    if unsafe { SetHandleInformation(r.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err("the sandbox could not be set up".into());
    }
    Ok((r, w))
}

/// Reads at most `MAX_STREAM` bytes from a pipe in a thread (the rest is drained).
fn reader(h: Owned) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
    let raw = h.0 as RawHandle;
    std::mem::forget(h);
    // SAFETY: the File takes over the handle (closed when the File drops); nothing else closes it.
    let f = unsafe { std::fs::File::from_raw_handle(raw) };
    std::thread::spawn(move || super::read_capped(f))
}

/// Starts `program` with `args` in the folder's AppContainer and a new job object, waits up to `timeout`, and
/// returns what it printed.
///
/// * `profile` -- from [`profile_name`]; `env` -- the whole environment (nothing is inherited); `cwd` -- the
///   folder to run in (inside the job folder); `cpu_s` -- CPU time for the whole job.
///
/// Errors (fixed text): any confinement step failed, so nothing ran (the process starts suspended and is resumed
/// only inside the job).
pub(super) fn run_confined(
    profile: &str,
    program: &Path,
    args: &[String],
    env: &[(String, String)],
    cwd: &Path,
    timeout: Duration,
    cpu_s: u64,
) -> Result<Outcome, String> {
    // SECURITY: only .exe files are started directly (no .bat/.cmd parsing by cmd.exe: CWE-78, CWE-88).
    if !program.is_absolute() || !program.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return Err("only .exe programs can be started on Windows".into());
    }
    let mut cmdline: Vec<u16> = command_line(program, args)?.encode_utf16().chain(std::iter::once(0)).collect();
    let block = env_block(env)?;
    let app = wide(program.as_os_str());
    let dir = wide(cwd.as_os_str());
    let (sid, _sid_text) = ensure_profile(profile)?;
    struct Sid(PSID);
    impl Drop for Sid {
        fn drop(&mut self) {
            // SAFETY: from ensure_profile, freed once.
            unsafe { FreeSid(self.0) };
        }
    }
    let sid = Sid(sid);

    // The job object and its limits.
    // SAFETY: no attributes, no name.
    let job = Owned(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
    if job.0.is_null() {
        return Err("the sandbox could not be set up".into());
    }
    // SAFETY: a plain-data struct; all-zero is a valid start.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_JOB_MEMORY
        | JOB_OBJECT_LIMIT_JOB_TIME;
    info.BasicLimitInformation.ActiveProcessLimit = MAX_PROCESSES;
    // CPU time in 100 ns units for the whole job (CWE-400).
    info.BasicLimitInformation.PerJobUserTimeLimit =
        i64::try_from(cpu_s.saturating_mul(10_000_000)).unwrap_or(i64::MAX);
    info.JobMemoryLimit = JOB_MEMORY;
    let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS { UIRestrictionsClass: UILIMIT_ALL };
    // SAFETY: pointers to the local structs with their exact sizes.
    let set = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &info as *const _ as _,
            std::mem::size_of_val(&info) as u32,
        ) != 0
            && SetInformationJobObject(
                job.0,
                JobObjectBasicUIRestrictions,
                &ui as *const _ as _,
                std::mem::size_of_val(&ui) as u32,
            ) != 0
    };
    if !set {
        return Err("the sandbox limits could not be set".into());
    }

    // Standard handles: NUL for input, pipes for output.
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let nul = wide(OsStr::new("NUL"));
    // SAFETY: a valid name; inheritable handle for the child's stdin.
    let stdin = Owned(unsafe {
        CreateFileW(
            nul.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &sa,
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    });
    if stdin.0 == INVALID_HANDLE_VALUE {
        return Err("the sandbox could not be set up".into());
    }
    let (out_r, out_w) = pipe()?;
    let (err_r, err_w) = pipe()?;

    // The attribute list: the AppContainer and the handle list.
    let mut size = 0usize;
    // SAFETY: the size query (null list) is the documented first call.
    unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut size) };
    if size == 0 {
        return Err("the sandbox could not be set up".into());
    }
    let mut attr_buf = vec![0u8; size];
    let attrs = attr_buf.as_mut_ptr() as _;
    // SAFETY: a buffer of the size the system asked for.
    if unsafe { InitializeProcThreadAttributeList(attrs, 2, 0, &mut size) } == 0 {
        return Err("the sandbox could not be set up".into());
    }
    struct AttrList(*mut core::ffi::c_void);
    impl Drop for AttrList {
        fn drop(&mut self) {
            // SAFETY: initialised above, deleted once (the buffer outlives this guard).
            unsafe { DeleteProcThreadAttributeList(self.0) };
        }
    }
    let _attr_guard = AttrList(attrs);
    // SECURITY: no capabilities at all: no network, no libraries, no devices (NIST AC-6, SC-7).
    let caps = SECURITY_CAPABILITIES {
        AppContainerSid: sid.0,
        Capabilities: std::ptr::null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    // SECURITY: only these three handles are inherited, nothing else the IDE has open (CWE-403).
    let handles: [HANDLE; 3] = [stdin.0, out_w.0, err_w.0];
    // SAFETY: the values live until CreateProcessW returns (they are locals of this function).
    let ok = unsafe {
        UpdateProcThreadAttribute(
            attrs,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            &caps as *const _ as _,
            std::mem::size_of_val(&caps),
            std::ptr::null_mut(),
            std::ptr::null(),
        ) != 0
            && UpdateProcThreadAttribute(
                attrs,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr() as _,
                std::mem::size_of_val(&handles),
                std::ptr::null_mut(),
                std::ptr::null(),
            ) != 0
    };
    if !ok {
        return Err("the sandbox could not be set up".into());
    }
    // SAFETY: plain-data structs; all-zero is a valid start.
    let mut si: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = stdin.0;
    si.StartupInfo.hStdOutput = out_w.0;
    si.StartupInfo.hStdError = err_w.0;
    si.lpAttributeList = attrs;
    // SAFETY: as above.
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // CREATE_NO_WINDOW: a hidden console that the job's own console programs (rustc, node, cmd) share, so no
    // window ever opens on the person's desktop.
    let flags = EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW;
    // SAFETY: every pointer is to a live, NUL-terminated buffer or struct of this function.
    let started = unsafe {
        CreateProcessW(
            app.as_ptr(),
            cmdline.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            flags,
            block.as_ptr() as _,
            dir.as_ptr(),
            &si.StartupInfo,
            &mut pi,
        )
    };
    if started == 0 {
        return Err("the program could not be started in the sandbox".into());
    }
    let process = Owned(pi.hProcess);
    let thread = Owned(pi.hThread);
    // SECURITY: in the job before its first instruction runs (it was created suspended).
    // SAFETY: valid handles from CreateProcessW.
    if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
        // SAFETY: as above.
        unsafe { TerminateProcess(process.0, 1) };
        return Err("the sandbox could not be set up".into());
    }
    // The child has its own copies now; ours must close so the readers see the end of the output.
    drop((stdin, out_w, err_w));
    let out = reader(out_r);
    let err = reader(err_r);
    let t = Instant::now();
    // SAFETY: a valid thread handle, still suspended.
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        // SAFETY: as above.
        unsafe { TerminateJobObject(job.0, 1) };
        return Err("the sandbox could not be set up".into());
    }
    drop(thread);
    let mut timed_out = false;
    loop {
        // SAFETY: a valid process handle.
        if unsafe { WaitForSingleObject(process.0, 50) } == WAIT_OBJECT_0 {
            break;
        }
        if t.elapsed() >= timeout {
            timed_out = true;
            break;
        }
    }
    // SECURITY: everything the program started ends with it (CWE-404); at the time limit the program too.
    // SAFETY: a valid job handle.
    unsafe { TerminateJobObject(job.0, 1) };
    let mut code = 0u32;
    // SAFETY: a valid process handle; `code` receives its exit code.
    let exit_code = if unsafe { GetExitCodeProcess(process.0, &mut code) } != 0 && !timed_out {
        Some(i64::from(code as i32))
    } else {
        None
    };
    let (so, c1) = out.join().unwrap_or_default();
    let (se, c2) = err.join().unwrap_or_default();
    drop(job);
    Ok(Outcome { exit_code, timed_out, stdout: so, stderr: se, cut: c1 || c2 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_quoted_as_the_c_runtime_splits_them() {
        let q = |a: &str| {
            let mut s = String::new();
            quote_arg(a, &mut s);
            s
        };
        assert_eq!(q("plain"), "plain");
        assert_eq!(q(""), "\"\"");
        assert_eq!(q("a b"), "\"a b\"");
        assert_eq!(q("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(q(r"C:\dir\"), r"C:\dir\");
        assert_eq!(q(r"C:\my dir\"), r#""C:\my dir\\""#);
        assert_eq!(q(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(q("x & del y"), "\"x & del y\"");
        assert!(command_line(Path::new(r"C:\a\b.exe"), &["\0".into()]).is_err());
        assert!(command_line(Path::new("C:\\a\"b.exe"), &[]).is_err());
        assert_eq!(
            command_line(Path::new(r"C:\a b\c.exe"), &["x".into(), "y z".into()]).unwrap(),
            "\"C:\\a b\\c.exe\" x \"y z\""
        );
    }

    #[test]
    fn the_environment_block_is_sorted_and_checked() {
        let b = env_block(&[("b".into(), "2".into()), ("A".into(), "1".into())]).unwrap();
        let s = String::from_utf16(&b).unwrap();
        assert_eq!(s, "A=1\0b=2\0\0");
        assert!(env_block(&[("".into(), "x".into())]).is_err());
        assert!(env_block(&[("a=b".into(), "x".into())]).is_err());
        assert!(env_block(&[("a".into(), "x\0y".into())]).is_err());
        assert_eq!(profile_name("0123456789abcdef0123456789abcdef"), "P2PAI.IDE.Run.0123456789abcdef01234567");
    }
}
