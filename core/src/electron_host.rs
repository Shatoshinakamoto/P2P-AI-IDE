//! electron_host -- the optional Electron compatibility host for Node extensions (phase I6d,
//! docs/features/node-extensions-design.md section 11).
//!
//! Purpose: some VS Code extensions ship native modules (`.node` files) built with Node's classic C++ interface for
//! VS Code's desktop Electron. Such a module loads only in a runtime with the same module version (ABI): VS Code
//! 1.145's Electron 43.8.3 is ABI 148, while our server's own Node 24 is ABI 137, so the module fails to load and the
//! extension doesn't activate. Modules built on Node-API (N-API) are ABI-stable and work in both. When the person
//! turns compatibility on, this module downloads the exact Electron release VS Code 1.145.2 ships, checks it, unpacks
//! it into the app's data folder, and gives [`crate::node_host::start`] a launcher that runs our pinned server with
//! that Electron in Node mode (`ELECTRON_RUN_AS_NODE=1`), the way desktop VS Code runs its extension host. The
//! server's own native modules are all N-API, so they load in both (checked in the I6d experiment).
//!
//! Inputs and trust boundaries:
//!   * The download is untrusted network data: it must match a SHA-256 pinned in this file before it is unpacked.
//!   * The archive's entries are untrusted: each path is allow-listed and each byte counted before it is written.
//!   * Installed extensions' `.node` files are untrusted bytes: they are only searched for two fixed names, with
//!     caps on depth, entries and bytes; nothing is loaded or run by the scan.
//!
//! Security controls:
//!   * Off by default; the person turns it on, and the download happens only after they agreed to it (the shell
//!     passes `allow_download`). The switch is a 0600 file in the app's data folder (NIST CM-7, least function).
//!   * Integrity (NIST SI-7, SSDF PS.2, CWE-494): exact Electron version and file pinned per OS and CPU with their
//!     SHA-256 ([`PINS`]), taken from our own download and compared with Electron's published `SHASUMS256.txt`. No
//!     pin for this OS and CPU means nothing is offered (fail closed). The download uses the same https-only,
//!     allowed-hosts, public-address network seam as the Node host (`node_host::real_get`; CWE-918, CWE-601).
//!   * Archive (CWE-22, CWE-59, CWE-409): every path part allow-listed (`node_host::safe_parts`); links refused;
//!     entries, per-file and total bytes capped and counted on what is read; files created `create_new` through a
//!     cap-std handle on a fresh folder renamed into place only when complete (CWE-362).
//!   * No set-id helper (CWE-250): Electron's `chrome-sandbox` (set-uid root in a normal install) is skipped; Node mode
//!     never uses it, and every file gets owner-only permissions.
//!   * The server's environment stays the scrubbed allow-list (`node_host::ENV_ALLOW`, CWE-522) plus exactly one
//!     fixed variable, `ELECTRON_RUN_AS_NODE=1`. `NODE_OPTIONS` and `ELECTRON_*` from the app's own environment are
//!     never passed (they could load code into the host, CWE-94).
//!   * Errors are the Node host's fixed messages (CWE-209).
//!
//! Accepted risk: the same as Node extensions themselves. Native modules run with the person's rights, which is why
//! the host only starts in a trusted folder.
//!
//! Dependencies: zip, sha2, cap-std (already used by the core); `crate::node_host` for the network seam, the path
//! checks, the error kinds and the launcher.

use crate::node_host::{allowed_url, safe_parts, Get, HostError, Launcher};
use crate::store;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

/// The Electron release VS Code 1.145.2 ships (its `.npmrc` target), so extensions built for that VS Code load.
pub const ELECTRON_VERSION: &str = "43.8.3";
/// Electron 43.8.3's Node module version (`process.versions.modules`), for the docs and the UI text.
pub const ELECTRON_ABI: u32 = 148;
/// Our server's own Node 24.22.1 module version.
pub const SERVER_NODE_ABI: u32 = 137;

/// Largest download accepted (the Windows zip is about 145 MiB).
pub const MAX_DOWNLOAD: u64 = 200 << 20;
/// Largest single file (the Linux `electron` program is about 215 MiB).
pub const MAX_FILE: u64 = 300 << 20;
/// Largest unpacked size, counted on bytes read (CWE-409).
pub const MAX_TOTAL: u64 = 800 << 20;
/// Most archive entries (the zips have under 100).
pub const MAX_ENTRIES: usize = 2000;

/// One pinned Electron download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElectronPin {
    /// `std::env::consts::OS` value.
    pub os: &'static str,
    /// `std::env::consts::ARCH` value.
    pub arch: &'static str,
    /// The release file's https URL.
    pub url: &'static str,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: &'static str,
}

/// The pinned Electron builds: only for the platforms our server is pinned for (`node_host::PINS`).
///
/// Pinned 2026-11-22 from Electron's own GitHub release v43.8.3: each zip was downloaded and hashed here, and the hash
/// equals the line in the release's `SHASUMS256.txt` (HistoryTest run of the I6d experiment). To move to a new
/// Electron (after an editor library upgrade changes VS Code's `.npmrc` target), do the same for every platform.
pub const PINS: &[ElectronPin] = &[
    ElectronPin {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/electron/electron/releases/download/v43.8.3/electron-v43.8.3-linux-x64.zip",
        // SECURITY: the zip is accepted only with exactly this hash (NIST SI-7, CWE-494).
        sha256: "73d7d6fe120c998db75834c23c8ce7ce8390607c198b2e167a28acd7937bef22",
    },
    ElectronPin {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/electron/electron/releases/download/v43.8.3/electron-v43.8.3-win32-x64.zip",
        // SECURITY: the zip is accepted only with exactly this hash (NIST SI-7, CWE-494).
        sha256: "98efbd2d1c7bd44150c0aa800cd9ac5fefcefd2d44fd94958a95d6744d86c6de",
    },
];

fn lower_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The pin for an OS and CPU, if there is one with a well-formed hash and an allowed URL.
pub fn pin_for(pins: &[ElectronPin], os: &str, arch: &str) -> Option<ElectronPin> {
    pins.iter().find(|p| p.os == os && p.arch == arch).filter(|p| lower_hex64(p.sha256) && allowed_url(p.url)).copied()
}

/// The pin for this computer.
pub fn this_pin() -> Option<ElectronPin> {
    pin_for(PINS, std::env::consts::OS, std::env::consts::ARCH)
}

/// Downloads the pinned zip and checks its hash. Nothing is returned unless the hash matches (fail closed).
pub fn download(get: Get, pin: &ElectronPin) -> Result<Vec<u8>, HostError> {
    let bytes = get(pin.url, MAX_DOWNLOAD)?;
    // SECURITY: the pinned SHA-256 is the only thing that makes the download trusted (CWE-494, NIST SI-7).
    let digest: String = Sha256::digest(&bytes).iter().map(|x| format!("{x:02x}")).collect();
    if digest != pin.sha256 {
        return Err(HostError::HashMismatch);
    }
    Ok(bytes)
}

/// Names never unpacked: Electron's set-uid sandbox helper, unused in Node mode (CWE-250).
const SKIP: &[&str] = &["chrome-sandbox"];

/// Unpacks an Electron zip into the new folder `dest` with every check in the module notes.
pub fn unpack_zip(bytes: &[u8], dest: &Path) -> Result<(), HostError> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| HostError::BadArchive)?;
    if z.len() > MAX_ENTRIES {
        return Err(HostError::TooLarge);
    }
    // Check every name before writing anything.
    let mut plan = vec![];
    for i in 0..z.len() {
        let f = z.by_index_raw(i).map_err(|_| HostError::BadArchive)?;
        // SECURITY: links are refused, not skipped: a pinned Linux or Windows zip has none (CWE-59).
        if f.is_symlink() {
            return Err(HostError::BadArchive);
        }
        let name = f.name().map_err(|_| HostError::BadArchive)?.to_string();
        // SECURITY: an absolute name is refused before splitting (CWE-22).
        if name.starts_with('/') {
            return Err(HostError::BadArchive);
        }
        if let Some(parts) = safe_parts(&name)? {
            if parts.len() == 1 && SKIP.contains(&parts[0].as_str()) {
                continue;
            }
            #[cfg(unix)]
            let exec = f.unix_mode().is_some_and(|m| m & 0o100 != 0);
            #[cfg(not(unix))]
            let exec = false;
            plan.push((i, parts, exec));
        }
    }
    std::fs::create_dir(dest).map_err(|_| HostError::Io)?;
    let root = Dir::open_ambient_dir(dest, ambient_authority()).map_err(|_| HostError::Io)?;
    let mut total: u64 = 0;
    for (i, parts, exec) in plan {
        let mut f = z.by_index(i).map_err(|_| HostError::BadArchive)?;
        if f.is_dir() {
            let p: PathBuf = parts.iter().collect();
            root.create_dir_all(&p).map_err(|_| HostError::BadArchive)?;
            continue;
        }
        let (file, dirs) = parts.split_last().ok_or(HostError::BadArchive)?;
        let sub: PathBuf = dirs.iter().collect();
        if !dirs.is_empty() {
            root.create_dir_all(&sub).map_err(|_| HostError::BadArchive)?;
        }
        // SECURITY: create_new so an entry can never overwrite another or follow a link (CWE-59).
        let mut opts = cap_std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        let mut out = root.open_with(sub.join(file), &opts).map_err(|_| HostError::BadArchive)?;
        // SECURITY: count real bytes, not the archive's claimed sizes (CWE-409).
        let mut buf = [0u8; 64 * 1024];
        let mut this: u64 = 0;
        loop {
            let n = f.read(&mut buf).map_err(|_| HostError::BadArchive)?;
            if n == 0 {
                break;
            }
            this += n as u64;
            total += n as u64;
            if this > MAX_FILE || total > MAX_TOTAL {
                return Err(HostError::TooLarge);
            }
            out.write_all(&buf[..n]).map_err(|_| HostError::Io)?;
        }
        #[cfg(unix)]
        {
            use cap_std::fs::PermissionsExt;
            // SECURITY: owner-only, never set-id or group/other write (CWE-276, CWE-732).
            let m = if exec { 0o700 } else { 0o600 };
            out.set_permissions(cap_std::fs::Permissions::from_mode(m)).map_err(|_| HostError::Io)?;
        }
        #[cfg(not(unix))]
        let _ = exec;
    }
    Ok(())
}

/// The Electron program's file name on this OS.
pub fn exe_name() -> &'static str {
    if cfg!(windows) {
        "electron.exe"
    } else {
        "electron"
    }
}

/// The install folder for a pin: `<data>/electron-host/<version>-<os>-<arch>`.
pub fn install_dir(data: &Path, pin: &ElectronPin) -> PathBuf {
    data.join("electron-host").join(format!("{ELECTRON_VERSION}-{}-{}", pin.os, pin.arch))
}

/// The installed Electron program for `pin`, if it is there (a regular file, not a link).
pub fn installed_exe(data: &Path, pin: &ElectronPin) -> Option<PathBuf> {
    let p = install_dir(data, pin).join(exe_name());
    // SECURITY: symlink_metadata does not follow links (CWE-59).
    std::fs::symlink_metadata(&p).ok().filter(|m| m.file_type().is_file()).map(|_| p)
}

/// Downloads, checks and unpacks Electron for `pin` unless it is already there. Returns the program's path.
pub fn ensure_installed(get: Get, data: &Path, pin: &ElectronPin) -> Result<PathBuf, HostError> {
    if let Some(exe) = installed_exe(data, pin) {
        return Ok(exe);
    }
    let bytes = download(get, pin)?;
    let parent = data.join("electron-host");
    store::ensure_dir(&parent)?;
    let tmp = parent.join(format!(".unpack-{}", store::random_id()));
    let ok = unpack_zip(&bytes, &tmp).and_then(|_| {
        // SECURITY: the unpacked folder must hold the program as a regular file, or it is not used (fail closed).
        let m = std::fs::symlink_metadata(tmp.join(exe_name())).map_err(|_| HostError::BadArchive)?;
        if m.file_type().is_file() {
            Ok(())
        } else {
            Err(HostError::BadArchive)
        }
    });
    if let Err(e) = ok {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let dest = install_dir(data, pin);
    // SECURITY: only a fully unpacked and checked folder is moved into place (CWE-362).
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&tmp, &dest).map_err(|_| {
        let _ = std::fs::remove_dir_all(&tmp);
        HostError::Io
    })?;
    installed_exe(data, pin).ok_or(HostError::BadArchive)
}

fn switch_file(data: &Path) -> PathBuf {
    data.join("electron-host").join("enabled")
}

/// True when the person turned compatibility on. Off when the file is missing or has anything but `on`.
pub fn enabled(data: &Path) -> bool {
    store::read_capped(&switch_file(data), 16).is_ok_and(|b| b == b"on")
}

/// Turns compatibility on or off. Only from the person's click (the shell's command).
pub fn set_enabled(data: &Path, on: bool) -> Result<(), HostError> {
    store::ensure_dir(&data.join("electron-host"))?;
    if on {
        // SECURITY: written atomically with owner-only permissions (store::write_atomic; CWE-276, CWE-362).
        store::write_atomic(&switch_file(data), b"on")?;
    } else {
        store::remove(&switch_file(data))?;
    }
    Ok(())
}

/// The launcher that runs the server at `server_root` with Electron `exe` in Node mode.
pub fn launcher(exe: &Path, server_root: &Path) -> Launcher {
    Launcher {
        program: exe.to_path_buf(),
        args: vec![server_root.join("out").join("server-main.js").to_string_lossy().into_owned()],
        // SECURITY: the one fixed variable added to the scrubbed environment (module notes; CWE-94).
        env: vec![("ELECTRON_RUN_AS_NODE".to_string(), "1".to_string())],
    }
}

/// How a native module talks to its runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeKind {
    /// Node-API: ABI-stable, loads in Node and Electron alike.
    NodeApi,
    /// Node's classic C++ interface: loads only in the runtime (module version) it was built for.
    Classic,
    /// Neither name found (not a Node module, or packed in a way we don't read).
    Unknown,
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Classifies a `.node` file's bytes by the names it links to. A Node-API module uses `napi_*` functions; a classic
/// one registers through `node_module_register` (the name is in the import table of ELF, PE and Mach-O files).
pub fn classify(bytes: &[u8]) -> NativeKind {
    if contains(bytes, b"napi_module_register") || contains(bytes, b"napi_register_module_v1") {
        NativeKind::NodeApi
    } else if contains(bytes, b"node_module_register") {
        NativeKind::Classic
    } else {
        NativeKind::Unknown
    }
}

/// Largest `.node` file the scan reads.
pub const SCAN_MAX_FILE: u64 = 64 << 20;
/// Most folder entries the scan visits per extension.
pub const SCAN_MAX_ENTRIES: usize = 20_000;

/// True when the extension folder `dir` holds a classic native module (one that needs the runtime it was built
/// for). Read-only, capped walk; links are not followed (CWE-59, CWE-400).
pub fn has_classic_native(dir: &Path) -> bool {
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut seen = 0usize;
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > SCAN_MAX_ENTRIES {
                return false;
            }
            // SECURITY: file_type() does not follow links (CWE-59).
            let Ok(t) = e.file_type() else { continue };
            let p = e.path();
            if t.is_dir() && depth < 16 {
                stack.push((p, depth + 1));
            } else if t.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("node")) {
                if let Ok(b) = store::read_capped(&p, SCAN_MAX_FILE) {
                    if classify(&b) == NativeKind::Classic {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-electron-{name}-{}", store::random_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A zip built by hand; `link` entries are stored as Unix symbolic links.
    fn zip_of(entries: &[(&str, &[u8], u32, bool)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, mode, link) in entries {
            let o = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
            if *link {
                w.add_symlink(*name, "../../etc/passwd", o).unwrap();
            } else if name.ends_with('/') {
                w.add_directory(*name, o).unwrap();
            } else {
                w.start_file(*name, o).unwrap();
                w.write_all(data).unwrap();
            }
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn pins_are_well_formed_and_cover_the_server_platforms() {
        for p in PINS {
            assert!(pin_for(PINS, p.os, p.arch).is_some(), "{p:?}");
            assert!(p.url.contains(&format!("v{ELECTRON_VERSION}/")));
            // Every Electron pin has a server pin for the same platform, or the switch would offer nothing useful.
            assert!(crate::node_host::pin_for(crate::node_host::PINS, p.os, p.arch).is_some());
        }
        assert!(pin_for(PINS, "macos", "aarch64").is_none());
        // A bad hash or a host that is not allowed is refused (fail closed).
        let bad = [ElectronPin { os: "linux", arch: "x86_64", url: PINS[0].url, sha256: "" }];
        assert!(pin_for(&bad, "linux", "x86_64").is_none());
        let evil = [ElectronPin { url: "https://evil.example/electron.zip", ..PINS[0] }];
        assert!(pin_for(&evil, "linux", "x86_64").is_none());
    }

    #[test]
    fn download_checks_the_hash() {
        let body = b"electron".to_vec();
        let sha: String = Sha256::digest(&body).iter().map(|x| format!("{x:02x}")).collect();
        let sha: &'static str = Box::leak(sha.into_boxed_str());
        let pin = ElectronPin { sha256: sha, ..PINS[0] };
        let get = |_u: &str, _m: u64| -> Result<Vec<u8>, HostError> { Ok(b"electron".to_vec()) };
        assert_eq!(download(&get, &pin).unwrap(), body);
        let other = |_u: &str, _m: u64| -> Result<Vec<u8>, HostError> { Ok(b"tampered".to_vec()) };
        assert_eq!(download(&other, &pin), Err(HostError::HashMismatch));
    }

    #[test]
    fn unpack_keeps_files_skips_the_setuid_helper_and_refuses_bad_entries() {
        let d = tmp("unpack");
        let ok = zip_of(&[
            ("electron", b"ELF", 0o755, false),
            ("chrome-sandbox", b"SUID", 0o4755, false),
            ("locales/", b"", 0o755, false),
            ("locales/en-US.pak", b"pak", 0o644, false),
        ]);
        unpack_zip(&ok, &d.join("a")).unwrap();
        assert_eq!(std::fs::read(d.join("a/electron")).unwrap(), b"ELF");
        assert!(d.join("a/locales/en-US.pak").is_file());
        assert!(!d.join("a/chrome-sandbox").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(d.join("a/electron")).unwrap().permissions().mode() & 0o7777, 0o700);
            assert_eq!(std::fs::metadata(d.join("a/locales/en-US.pak")).unwrap().permissions().mode() & 0o7777, 0o600);
        }
        for (i, bad) in [
            zip_of(&[("../evil", b"x", 0o644, false)]),
            zip_of(&[("a/../../evil", b"x", 0o644, false)]),
            zip_of(&[("C:evil", b"x", 0o644, false)]),
            zip_of(&[("lib", b"", 0o777, true)]),
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(unpack_zip(bad, &d.join(format!("b{i}"))), Err(HostError::BadArchive), "case {i}");
        }
        assert!(!d.parent().unwrap().join("evil").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ensure_installed_downloads_once_and_needs_the_program() {
        let d = tmp("install");
        let good = zip_of(&[(exe_name(), b"prog", 0o755, false)]);
        let sha: String = Sha256::digest(&good).iter().map(|x| format!("{x:02x}")).collect();
        let pin = ElectronPin { sha256: Box::leak(sha.into_boxed_str()), ..PINS[0] };
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let get = |_u: &str, _m: u64| -> Result<Vec<u8>, HostError> {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(good.clone())
        };
        let exe = ensure_installed(&get, &d, &pin).unwrap();
        assert!(exe.ends_with(exe_name()));
        assert_eq!(ensure_installed(&get, &d, &pin).unwrap(), exe);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // A zip without the program is not installed.
        let d2 = tmp("install2");
        let empty = zip_of(&[("readme.txt", b"x", 0o644, false)]);
        let sha2: String = Sha256::digest(&empty).iter().map(|x| format!("{x:02x}")).collect();
        let pin2 = ElectronPin { sha256: Box::leak(sha2.into_boxed_str()), ..PINS[0] };
        let get2 = |_u: &str, _m: u64| -> Result<Vec<u8>, HostError> { Ok(empty.clone()) };
        assert_eq!(ensure_installed(&get2, &d2, &pin2), Err(HostError::BadArchive));
        assert!(installed_exe(&d2, &pin2).is_none());
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&d2);
    }

    #[test]
    fn switch_is_off_by_default_and_only_on_means_on() {
        let d = tmp("switch");
        assert!(!enabled(&d));
        set_enabled(&d, true).unwrap();
        assert!(enabled(&d));
        set_enabled(&d, false).unwrap();
        assert!(!enabled(&d));
        std::fs::write(d.join("electron-host/enabled"), b"yes").unwrap();
        assert!(!enabled(&d));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn launcher_adds_only_run_as_node() {
        let l = launcher(Path::new("/x/electron"), Path::new("/srv"));
        assert_eq!(l.program, PathBuf::from("/x/electron"));
        assert!(l.args[0].ends_with("server-main.js"));
        assert_eq!(l.env, vec![("ELECTRON_RUN_AS_NODE".to_string(), "1".to_string())]);
        // The scrubbed environment never carries code-loading variables from the app's own environment.
        let env = crate::node_host::scrubbed_env(
            [("NODE_OPTIONS", "--require /evil.js"), ("ELECTRON_RUN_AS_NODE", "1"), ("PATH", "/bin")]
                .into_iter()
                .map(|(a, b)| (a.to_string(), b.to_string())),
        );
        assert_eq!(env, vec![("PATH".to_string(), "/bin".to_string())]);
    }

    #[test]
    fn classify_and_scan_native_modules() {
        assert_eq!(classify(b"\x7fELF..napi_register_module_v1..node_module_register"), NativeKind::NodeApi);
        assert_eq!(classify(b"\x7fELF..napi_module_register.."), NativeKind::NodeApi);
        assert_eq!(classify(b"MZ..node_module_register.."), NativeKind::Classic);
        assert_eq!(classify(b"plain"), NativeKind::Unknown);
        let d = tmp("scan");
        std::fs::create_dir_all(d.join("napi/build/Release")).unwrap();
        std::fs::write(d.join("napi/build/Release/a.node"), b"napi_register_module_v1").unwrap();
        assert!(!has_classic_native(&d.join("napi")));
        std::fs::create_dir_all(d.join("classic/native")).unwrap();
        std::fs::write(d.join("classic/native/b.NODE"), b"..node_module_register..").unwrap();
        assert!(has_classic_native(&d.join("classic")));
        // A classic module reached only through a link is not counted (links are never followed).
        #[cfg(unix)]
        {
            std::fs::create_dir_all(d.join("linked")).unwrap();
            std::os::unix::fs::symlink(d.join("classic/native"), d.join("linked/native")).unwrap();
            assert!(!has_classic_native(&d.join("linked")));
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
