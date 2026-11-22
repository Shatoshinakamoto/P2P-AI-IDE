//! node_host -- the Node extension host sidecar (phase I6c-1, ADR-029 D1; docs/features/node-extensions-design.md).
//!
//! Purpose: Node extensions (a `main` entry, Node APIs) can't run in the editor's sandboxed web worker. The editor
//! library runs them only through a VS Code server ("remote extension host"). This module gets the MIT-licensed
//! VSCodium server the first time a trusted folder needs it, checks it, unpacks it into the app's own data folder,
//! and starts and stops it as a local sidecar for one trusted folder. The shell (app/src-tauri) calls it; the UI then
//! connects to the server on 127.0.0.1 with the connection token.
//!
//! Inputs and trust boundaries:
//!   * The download is untrusted network data: it must match a SHA-256 pinned in this file before it is unpacked.
//!   * The archive's entries are untrusted: each path is allow-listed and each byte counted before it is written.
//!   * The folder path comes from the shell (the person picked it); it must be in the trust list the person built.
//!   * The server's output is untrusted text: only one line shape ("listening on <port>") is read from it.
//!
//! Security controls:
//!   * Integrity (NIST SI-7, SSDF PS.2, CWE-494): the exact version and file are pinned per OS and CPU with their
//!     SHA-256 ([`PINS`]); an OS or CPU with no pin, or a pin with an empty hash, gets nothing (fail closed). https
//!     only, to the GitHub release hosts in [`ALLOWED_HOSTS`], public addresses only, no proxy from the environment
//!     (CWE-918), redirects followed by hand to allowed hosts only (CWE-601), size capped (CWE-400).
//!   * Archive (CWE-22, CWE-59, CWE-409): every path part allow-listed; regular files and folders only (symbolic
//!     links are skipped, hard links and devices refused); at most [`MAX_ENTRIES`] entries and [`MAX_TOTAL`] bytes
//!     counted on what is read; files created `create_new` through a cap-std handle on a fresh folder, which is then
//!     renamed into place, so a half unpack is never used.
//!   * Workspace trust (CWE-829): [`TrustStore`] holds the folders the person trusted; [`start`] refuses any other.
//!   * Local access (CWE-306, CWE-1327): the server listens on 127.0.0.1 only, on a port the OS picks, and needs a
//!     192-bit connection token from the OS CSPRNG (CWE-330). The token is passed in a 0600 file, not on the command
//!     line where other users' process lists could see it (CWE-214), and it is never logged (CWE-532).
//!   * No secrets for extensions (CWE-522, NIST AC-6): the server's environment is cleared and rebuilt from
//!     [`ENV_ALLOW`]; gateway keys stay in the OS keychain and the core.
//!   * Errors are fixed messages by kind (CWE-209). The shell audits downloads, trust changes, starts and stops.
//!
//! Accepted risk: Node extensions run with the person's own rights, as in VS Code. That is why the host only starts
//! in a folder the person trusted, and only extensions the person installed from Open VSX are loaded.
//!
//! Dependencies: ureq, sha2, getrandom, cap-std and flate2 (already used), and `tar` 0.4 (new; the maintained
//! tar-rs crate, MIT/Apache-2.0, read-only use: entries are inspected and written by this module, never with the
//! crate's own `unpack`).

use crate::store::{self, StoreError};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Hosts the server download may come from: GitHub releases and the CDN its downloads redirect to.
pub const ALLOWED_HOSTS: [&str; 3] =
    ["github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com"];

/// Largest download accepted (the servers are about 90 MB packed).
pub const MAX_DOWNLOAD: u64 = 200 << 20;
/// Largest unpacked size, counted on bytes read (CWE-409).
pub const MAX_TOTAL: u64 = 600 << 20;
/// Largest single file.
pub const MAX_FILE: u64 = 200 << 20;
/// Most archive entries.
pub const MAX_ENTRIES: usize = 60_000;
/// How long the server may take to say which port it listens on.
pub const START_TIMEOUT: Duration = Duration::from_secs(30);

/// The VS Code version the pinned server is built from. The editor must be built on the same VS Code minor
/// version: VS Code's editor-to-extension-host messages are numbered, and the numbering changes between minor
/// versions, so a mismatched pair connects but silently never starts an extension (found on the PC in phase
/// I6c-3: an editor on 1.145 and the VSCodium 1.140 server pinned then). The editor checks this before it connects
/// (app/src/lib/vscodeVersion.ts).
pub const VSCODE_VERSION: &str = "1.145.2";

/// One pinned server download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    /// `std::env::consts::OS` value: `linux`, `macos` or `windows`.
    pub os: &'static str,
    /// `std::env::consts::ARCH` value: `x86_64` or `aarch64`.
    pub arch: &'static str,
    /// Server build version, for example `1.145.2`.
    pub version: &'static str,
    /// The release file's https URL.
    pub url: &'static str,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: &'static str,
    /// The server's VS Code commit (40 lowercase hex, from its `product.json`). The editor must announce the same
    /// commit, or the server refuses the connection as a version mismatch.
    pub commit: &'static str,
}

/// Our server build's version: the VS Code release tag it is built from (`scripts/build_node_host.py`).
pub const SERVER_BUILD: &str = "1.145.2";
/// Its VS Code commit, checked by the build script against the clone and the built `product.json`. It is also
/// the editor library's commit, so the server accepts the editor's connection.
pub const SERVER_COMMIT: &str = "c4453f8ea430795f941c1b2ff6c9321135a74a02";

/// The pinned servers: our own builds of VS Code 1.145.2's MIT server (REH), published on the IDE repo's
/// pre-release `node-host-1.145.2` (decided by Pheonix 2026-11-22: build our own, host it there).
///
/// Pinned 2026-11-22: Linux x64 was built in the cloud (HistoryTest `2026-11-22_053759Z_node-host-build_4096b2e`),
/// Windows x64 natively on the PC; each file was uploaded, downloaded back anonymously from the release and hashed
/// again (same SHA-256). Platforms with no builder yet (macOS, Linux on ARM) get nothing (fail closed, CWE-494):
/// the app says Node extensions are not available there. To add or
/// move a build, publish the archive and its `.sha256`, download it back, hash it, and update the constants.
pub const PINS: &[Pin] = &[Pin {
    os: "linux",
    arch: "x86_64",
    version: SERVER_BUILD,
    url: "https://github.com/Shatoshinakamoto/P2P-AI-IDE/releases/download/node-host-1.145.2/p2pide-node-host-linux-x64-1.145.2.tar.gz",
    // SECURITY: the archive is accepted only with exactly this hash (NIST SI-7, CWE-494).
    sha256: "75deb573a1841099c9b17bffe5680a6f32e244439480ab3c25579aa4de38d80e",
    commit: SERVER_COMMIT,
},
Pin {
    os: "windows",
    arch: "x86_64",
    version: SERVER_BUILD,
    url: "https://github.com/Shatoshinakamoto/P2P-AI-IDE/releases/download/node-host-1.145.2/p2pide-node-host-win32-x64-1.145.2.tar.gz",
    // SECURITY: the archive is accepted only with exactly this hash (NIST SI-7, CWE-494).
    sha256: "fdbef6ee11aca9e18573815783cc5e536c1275ac3544e00bce76c6c9fadfd22e",
    commit: SERVER_COMMIT,
}];

/// Fixed error kinds; [`HostError::message`] is safe to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostError {
    /// No pinned server for this OS and CPU.
    NotPinned,
    /// The download failed or came from a host that is not allowed.
    Network,
    /// The download or the archive is larger than allowed.
    TooLarge,
    /// The download does not match its pinned hash.
    HashMismatch,
    /// The archive is damaged or has an entry that could leave its folder.
    BadArchive,
    /// The folder is not trusted.
    NotTrusted,
    /// The server is not installed yet.
    NotInstalled,
    /// The server did not start or did not say its port in time.
    StartFailed,
    /// Bad input from the caller.
    BadInput,
    /// A file-system failure.
    Io,
    /// The folder is not on a local drive (a network share), which the editor can't name for the server yet.
    FolderNotLocal,
}

impl HostError {
    /// A short message with no path or OS detail (CWE-209).
    pub fn message(&self) -> &'static str {
        match self {
            HostError::NotPinned => "there is no Node extension host for this computer yet",
            HostError::Network => "could not download the Node extension host",
            HostError::TooLarge => "the Node extension host download is too large",
            HostError::HashMismatch => "the Node extension host download failed its check and was not used",
            HostError::BadArchive => "the Node extension host download is damaged",
            HostError::NotTrusted => "Node extensions run only in a folder you trust",
            HostError::NotInstalled => "the Node extension host is not downloaded yet",
            HostError::StartFailed => "the Node extension host did not start",
            HostError::BadInput => "bad request",
            HostError::Io => "could not read or write the app's data folder",
            HostError::FolderNotLocal => {
                "Node extensions need a folder on a local drive; network folders are not supported yet"
            }
        }
    }
}

impl From<StoreError> for HostError {
    fn from(_: StoreError) -> Self {
        HostError::Io
    }
}

/// The pin for an OS and CPU, if there is one with a well-formed hash and an allowed URL.
pub fn pin_for(pins: &[Pin], os: &str, arch: &str) -> Option<Pin> {
    pins.iter()
        .find(|p| p.os == os && p.arch == arch)
        .filter(|p| lower_hex(p.sha256, 64) && lower_hex(p.commit, 40))
        .filter(|p| allowed_url(p.url))
        .copied()
}

fn lower_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn host_of(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    // SECURITY: no user info and no port, so "github.com@evil" or "github.com:1" cannot pass (CWE-601).
    if host.contains('@') || host.contains(':') {
        return None;
    }
    Some(host)
}

/// True when `url` is https on one of [`ALLOWED_HOSTS`].
pub fn allowed_url(url: &str) -> bool {
    host_of(url).is_some_and(|h| ALLOWED_HOSTS.iter().any(|a| a.eq_ignore_ascii_case(h)))
}

/// The network seam: GET a URL, at most `max` bytes. Tests pass a fake; [`real_get`] is the real one.
pub type Get<'a> = &'a (dyn Fn(&str, u64) -> Result<Vec<u8>, HostError> + Sync);

/// The real network seam: https only, public addresses only, up to 3 redirects and only to allowed hosts.
pub fn real_get() -> impl Fn(&str, u64) -> Result<Vec<u8>, HostError> + Sync {
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .https_only(true)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(600)))
        .user_agent("p2p-ai-ide/0.1")
        .build();
    let agent = ureq::Agent::with_parts(
        config,
        ureq::unversioned::transport::DefaultConnector::default(),
        crate::discovery::PublicOnly,
    );
    move |url: &str, max: u64| {
        let mut target = url.to_string();
        for _ in 0..4 {
            // SECURITY: every hop, including redirects, must be an allowed https host (CWE-601, CWE-918).
            if !allowed_url(&target) {
                return Err(HostError::Network);
            }
            let resp = agent.get(&target).call().map_err(|_| HostError::Network)?;
            let status = resp.status().as_u16();
            if (300..400).contains(&status) {
                let loc = resp.headers().get("location").and_then(|v| v.to_str().ok()).ok_or(HostError::Network)?;
                target = loc.to_string();
                continue;
            }
            if status != 200 {
                return Err(HostError::Network);
            }
            let mut buf = vec![];
            resp.into_body().into_reader().take(max + 1).read_to_end(&mut buf).map_err(|_| HostError::Network)?;
            if buf.len() as u64 > max {
                return Err(HostError::TooLarge);
            }
            return Ok(buf);
        }
        Err(HostError::Network)
    }
}

/// Downloads the pinned file and checks its hash. Nothing is returned unless the hash matches (fail closed).
pub fn download(get: Get, pin: &Pin) -> Result<Vec<u8>, HostError> {
    let bytes = get(pin.url, MAX_DOWNLOAD)?;
    // SECURITY: the pinned SHA-256 is the only thing that makes the download trusted (CWE-494, NIST SI-7).
    if hex(&Sha256::digest(&bytes)) != pin.sha256 {
        return Err(HostError::HashMismatch);
    }
    Ok(bytes)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Splits an archive path into safe parts; `None` for the archive root itself.
pub(crate) fn safe_parts(name: &str) -> Result<Option<Vec<String>>, HostError> {
    let name = name.strip_prefix("./").unwrap_or(name).trim_end_matches('/');
    if name.is_empty() || name == "." {
        return Ok(None);
    }
    let mut parts = vec![];
    for p in name.split('/') {
        // SECURITY: allow-list every part (CWE-22): no empty, ".", ".." parts, no absolute or drive paths, no
        // backslash, colon or control characters, and no Windows reserved trailing dot or space.
        if p.is_empty()
            || p == "."
            || p == ".."
            || p.len() > 255
            || p.ends_with('.')
            || p.ends_with(' ')
            || p.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        {
            return Err(HostError::BadArchive);
        }
        parts.push(p.to_string());
    }
    if parts.len() > 48 {
        return Err(HostError::BadArchive);
    }
    Ok(Some(parts))
}

/// Unpacks a `.tar.gz` into the new folder `dest` with every check in the module notes.
pub fn unpack_tgz(bytes: &[u8], dest: &Path) -> Result<(), HostError> {
    std::fs::create_dir(dest).map_err(|_| HostError::Io)?;
    let root = Dir::open_ambient_dir(dest, ambient_authority()).map_err(|_| HostError::Io)?;
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut ar = tar::Archive::new(gz);
    let mut total: u64 = 0;
    let mut count = 0usize;
    for entry in ar.entries().map_err(|_| HostError::BadArchive)? {
        let mut e = entry.map_err(|_| HostError::BadArchive)?;
        count += 1;
        if count > MAX_ENTRIES {
            return Err(HostError::TooLarge);
        }
        let kind = e.header().entry_type();
        let raw = e.path_bytes().into_owned();
        let name = std::str::from_utf8(&raw).map_err(|_| HostError::BadArchive)?;
        // SECURITY: an absolute name is refused before splitting (CWE-22).
        if name.starts_with('/') {
            return Err(HostError::BadArchive);
        }
        let parts = safe_parts(name)?;
        match kind {
            tar::EntryType::Directory => {
                if let Some(parts) = parts {
                    let p: PathBuf = parts.iter().collect();
                    root.create_dir_all(&p).map_err(|_| HostError::BadArchive)?;
                }
            }
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                let parts = parts.ok_or(HostError::BadArchive)?;
                let (file, dirs) = parts.split_last().ok_or(HostError::BadArchive)?;
                let sub: PathBuf = dirs.iter().collect();
                if !dirs.is_empty() {
                    root.create_dir_all(&sub).map_err(|_| HostError::BadArchive)?;
                }
                let mode = e.header().mode().unwrap_or(0o644);
                // SECURITY: create_new so an entry can never overwrite another or follow a link (CWE-59).
                let mut opts = cap_std::fs::OpenOptions::new();
                opts.write(true).create_new(true);
                let mut out = root.open_with(sub.join(file), &opts).map_err(|_| HostError::BadArchive)?;
                // SECURITY: count real bytes, not the header's claimed size (CWE-409).
                let mut buf = [0u8; 64 * 1024];
                let mut this: u64 = 0;
                loop {
                    let n = e.read(&mut buf).map_err(|_| HostError::BadArchive)?;
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
                    // SECURITY: keep only the owner's execute bit for programs (node, scripts); never set-id or
                    // group/other write (CWE-276, CWE-732).
                    let m = if mode & 0o100 != 0 { 0o700 } else { 0o600 };
                    out.set_permissions(cap_std::fs::Permissions::from_mode(m)).map_err(|_| HostError::Io)?;
                }
                #[cfg(not(unix))]
                let _ = mode;
            }
            // Symbolic links (for example node_modules/.bin) are not needed to run the server: skipped, never made.
            tar::EntryType::Symlink => {}
            // PAX and GNU long-name headers are read by the tar crate itself.
            tar::EntryType::XGlobalHeader | tar::EntryType::XHeader | tar::EntryType::GNULongName => {}
            // SECURITY: hard links, devices and FIFOs are refused (CWE-59, CWE-73).
            _ => return Err(HostError::BadArchive),
        }
    }
    Ok(())
}

/// The folder that holds the server files: the one with `out/server-main.js`, at the top or one level down.
pub fn server_root(dir: &Path) -> Option<PathBuf> {
    let has = |d: &Path| d.join("out").join("server-main.js").is_file();
    if has(dir) {
        return Some(dir.to_path_buf());
    }
    let mut found = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        // SECURITY: real folders only, no links (CWE-59).
        if e.file_type().ok()?.is_dir() && has(&p) {
            if found.is_some() {
                return None;
            }
            found = Some(p);
        }
    }
    found
}

/// The server's install folder for a pin: `<data>/node-host/<version>-<os>-<arch>`.
pub fn install_dir(data: &Path, pin: &Pin) -> PathBuf {
    data.join("node-host").join(format!("{}-{}-{}", pin.version, pin.os, pin.arch))
}

/// Downloads, checks and unpacks the server for `pin` unless it is already there. Returns the server root.
pub fn ensure_installed(get: Get, data: &Path, pin: &Pin) -> Result<PathBuf, HostError> {
    let dest = install_dir(data, pin);
    if let Some(root) = server_root(&dest) {
        return Ok(root);
    }
    let bytes = download(get, pin)?;
    let parent = data.join("node-host");
    store::ensure_dir(&parent)?;
    let tmp = parent.join(format!(".unpack-{}", store::random_id()));
    let result = unpack_tgz(&bytes, &tmp).and_then(|_| server_root(&tmp).map(|_| ()).ok_or(HostError::BadArchive));
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    // SECURITY: only a fully unpacked and checked folder is moved into place (CWE-362).
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&tmp, &dest).map_err(|_| {
        let _ = std::fs::remove_dir_all(&tmp);
        HostError::Io
    })?;
    server_root(&dest).ok_or(HostError::BadArchive)
}

/// Copies one folder tree from `src` to the new folder `dst`. Regular files and folders only: links and anything
/// else are skipped, so a planted link in the source can't pull files from elsewhere (CWE-59).
fn copy_tree(src: &Path, dst: &Path, depth: usize) -> Result<(), HostError> {
    if depth > 48 {
        return Err(HostError::BadArchive);
    }
    std::fs::create_dir(dst).map_err(|_| HostError::Io)?;
    for e in std::fs::read_dir(src).map_err(|_| HostError::Io)? {
        let e = e.map_err(|_| HostError::Io)?;
        // SECURITY: file_type() does not follow links (CWE-59).
        let t = e.file_type().map_err(|_| HostError::Io)?;
        let to = dst.join(e.file_name());
        if t.is_dir() {
            copy_tree(&e.path(), &to, depth + 1)?;
        } else if t.is_file() {
            std::fs::copy(e.path(), &to).map_err(|_| HostError::Io)?;
        }
    }
    Ok(())
}

/// Builds the server's extensions folder `dest` fresh from the enabled Node extensions in `store`, in the layout the
/// server reads (`<id>-<version>/package.json`). Rebuilt on every start, so a turned-off or removed extension is
/// never loaded, and nothing the server or an extension wrote there survives a restart.
pub fn stage_extensions(store: &crate::extensions::ExtStore, dest: &Path) -> Result<usize, HostError> {
    use crate::extensions::Kind;
    let list = store.list().map_err(|_| HostError::Io)?;
    let tmp = dest.with_file_name(format!(".node-extensions-{}", store::random_id()));
    std::fs::create_dir_all(&tmp).map_err(|_| HostError::Io)?;
    let mut n = 0;
    let result = (|| {
        for e in list.iter().filter(|e| e.enabled && e.kind == Kind::Node) {
            // SECURITY: the folder name comes from our own saved list, built from allow-listed parts (CWE-22).
            if e.folder.contains(['/', '\\']) || e.folder.starts_with('.') {
                return Err(HostError::BadInput);
            }
            // The store keeps the contents of the `.vsix`'s `extension/` folder directly in `<folder>/`
            // (extensions::install strips that prefix), which is the layout the server reads.
            copy_tree(&store.dir().join(&e.folder), &tmp.join(&e.folder), 0)?;
            n += 1;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(dest);
    std::fs::rename(&tmp, dest).map_err(|_| {
        let _ = std::fs::remove_dir_all(&tmp);
        HostError::Io
    })?;
    Ok(n)
}

/// The folders the person trusts to run Node extensions, kept in `<data>/trusted-folders.json`.
pub struct TrustStore {
    file: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct TrustFile {
    folders: Vec<String>,
}

const MAX_TRUST_FILE: u64 = 256 << 10;
const MAX_TRUSTED: usize = 500;

impl TrustStore {
    /// The store in the app data folder `data`.
    pub fn new(data: &Path) -> Self {
        TrustStore { file: data.join("trusted-folders.json") }
    }

    fn load(&self) -> Result<TrustFile, HostError> {
        match store::read_capped(&self.file, MAX_TRUST_FILE) {
            Ok(b) => serde_json::from_slice(&b).map_err(|_| HostError::Io),
            Err(StoreError::Missing) => Ok(TrustFile::default()),
            Err(_) => Err(HostError::Io),
        }
    }

    /// The canonical form of a folder: SECURITY: resolves links and `..` so one folder has one name (CWE-41).
    fn key(folder: &Path) -> Result<String, HostError> {
        let c = std::fs::canonicalize(folder).map_err(|_| HostError::BadInput)?;
        if !c.is_dir() {
            return Err(HostError::BadInput);
        }
        c.into_os_string().into_string().map_err(|_| HostError::BadInput)
    }

    /// True when the person trusted `folder`.
    pub fn is_trusted(&self, folder: &Path) -> bool {
        let Ok(k) = Self::key(folder) else { return false };
        self.load().map(|f| f.folders.contains(&k)).unwrap_or(false)
    }

    /// Trusts or stops trusting `folder`. Called only from the person's click in the shell.
    pub fn set(&self, folder: &Path, trusted: bool) -> Result<(), HostError> {
        let k = Self::key(folder)?;
        let mut f = self.load()?;
        f.folders.retain(|x| x != &k);
        if trusted {
            if f.folders.len() >= MAX_TRUSTED {
                return Err(HostError::BadInput);
            }
            f.folders.push(k);
        }
        if let Some(dir) = self.file.parent() {
            store::ensure_dir(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(&f).map_err(|_| HostError::Io)?;
        store::write_atomic(&self.file, &bytes)?;
        Ok(())
    }
}

/// Environment variables the server may inherit (everything else is dropped, CWE-522). These let extensions find
/// the person's tools (git, compilers) and the usual per-user folders.
pub const ENV_ALLOW: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "TMPDIR",
    "SHELL",
    "XDG_RUNTIME_DIR",
    "USERPROFILE",
    "USERNAME",
    "APPDATA",
    "LOCALAPPDATA",
    "TEMP",
    "TMP",
    "SystemRoot",
    "SystemDrive",
    "windir",
    "ComSpec",
    "PATHEXT",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramData",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
];

/// Builds the server's environment from `vars` (usually `std::env::vars()`): only [`ENV_ALLOW`] names survive.
pub fn scrubbed_env(vars: impl Iterator<Item = (String, String)>) -> Vec<(String, String)> {
    vars.filter(|(k, _)| ENV_ALLOW.iter().any(|a| a.eq_ignore_ascii_case(k))).collect()
}

/// The folder's path as the editor sends it to the server (inside `vscode-remote://<authority>/<path>` URIs).
///
/// `canonicalize` gives Windows paths the verbatim prefix (`\\?\D:\proj`); the editor turns the path into a URI
/// path, and one starting with `//` is not a valid URI without an authority, so the server refused every file
/// (found on the PC in phase I6c-3). This removes the prefix from drive paths and refuses network shares
/// (`\\?\UNC\...`, `\\server\...`), which would need the share as the URI's authority. Unix paths pass unchanged.
///
/// Security: only changes how the trusted folder is written; the server still gets the same folder (CWE-22: no
/// `..` handling here, the path is the canonical one the person opened).
pub fn editor_folder(folder: &Path) -> Result<String, HostError> {
    editor_folder_str(&folder.to_string_lossy())
}

fn editor_folder_str(s: &str) -> Result<String, HostError> {
    let s = s.strip_prefix(r"\\?\").unwrap_or(s);
    if s.starts_with(r"\\") || s.starts_with("//") || s.starts_with(r"UNC\") || s.starts_with("UNC/") {
        return Err(HostError::FolderNotLocal);
    }
    Ok(s.to_string())
}

/// Reads the port from a server output line: `Extension host agent listening on 40123`.
pub fn parse_listen(line: &str) -> Option<u16> {
    let rest = line.trim().rsplit_once("listening on ")?.1.trim();
    // Some builds print `127.0.0.1:40123`; take the part after the last colon.
    let port = rest.rsplit(':').next()?.trim();
    if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    port.parse::<u16>().ok().filter(|p| *p >= 1024)
}

/// A running server.
pub struct Running {
    child: Child,
    /// The port on 127.0.0.1.
    pub port: u16,
    /// The connection token (48 hex). SECURITY: never log it (CWE-532).
    pub token: String,
    /// The folder it serves.
    pub folder: PathBuf,
}

impl Running {
    /// Stops the server.
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// True while the process runs.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

fn token() -> Result<String, HostError> {
    let mut b = [0u8; 24];
    // SECURITY: 192 bits from the OS CSPRNG (CWE-330, CWE-338).
    getrandom::getrandom(&mut b).map_err(|_| HostError::StartFailed)?;
    Ok(hex(&b))
}

/// How to launch the server: the program and its first arguments. Tests point this at a fake.
pub struct Launcher {
    /// The Node program (`node` or `node.exe` in the server root).
    pub program: PathBuf,
    /// The server script and any arguments before ours.
    pub args: Vec<String>,
    /// Fixed variables added after the scrubbed environment (phase I6d: `ELECTRON_RUN_AS_NODE=1` for the Electron
    /// compatibility host, `crate::electron_host::launcher`). Empty for the server's own Node.
    pub env: Vec<(String, String)>,
}

impl Launcher {
    /// The VSCodium server's own Node and `out/server-main.js`.
    pub fn for_root(root: &Path) -> Launcher {
        let node = if cfg!(windows) { "node.exe" } else { "node" };
        Launcher {
            program: root.join(node),
            args: vec![root.join("out").join("server-main.js").to_string_lossy().into_owned()],
            env: vec![],
        }
    }
}

/// Starts the server for a trusted `folder`. `data` is the app data folder (the server's own state lives there).
pub fn start(launcher: &Launcher, data: &Path, trust: &TrustStore, folder: &Path) -> Result<Running, HostError> {
    // SECURITY: workspace trust is checked here, in the core, on every start (CWE-829, NIST AC-3).
    if !trust.is_trusted(folder) {
        return Err(HostError::NotTrusted);
    }
    let state = data.join("node-host-state");
    store::ensure_dir(&state)?;
    let token = token()?;
    let token_file = state.join("connection-token");
    // SECURITY: 0600 file (store::write_atomic), not a command-line argument other users could read (CWE-214).
    store::write_atomic(&token_file, token.as_bytes())?;
    let mut cmd = Command::new(&launcher.program);
    cmd.args(&launcher.args)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .arg("--connection-token-file")
        .arg(&token_file)
        .arg("--server-data-dir")
        .arg(state.join("server"))
        .arg("--user-data-dir")
        .arg(state.join("user"))
        .arg("--extensions-dir")
        .arg(data.join("node-extensions"))
        .arg("--disable-telemetry")
        .arg("--accept-server-license-terms")
        .current_dir(folder)
        // SECURITY: a fresh environment from the allow-list only, so no keys or tokens reach extensions (CWE-522).
        .env_clear()
        .envs(scrubbed_env(std::env::vars()))
        // SECURITY: only the launcher's own fixed values (never from the app's environment) are added (CWE-94).
        .envs(launcher.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .env("VSCODE_SERVER_NO_UPDATE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the server's node.exe is a console program; without this flag the windowed app gives
        // it a visible console window that stays open while the host runs (found on the PC in phase I6c-3).
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|_| HostError::StartFailed)?;
    let out = child.stdout.take().ok_or(HostError::StartFailed)?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = BufReader::new(out.take(1 << 20)).lines();
        let mut sent = false;
        for line in lines.by_ref().map_while(Result::ok) {
            if !sent {
                if let Some(p) = parse_listen(&line) {
                    let _ = tx.send(p);
                    sent = true;
                }
            }
        }
    });
    let deadline = Instant::now() + START_TIMEOUT;
    let port = rx.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    match port {
        Ok(port) => Ok(Running { child, port, token, folder: folder.to_path_buf() }),
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(HostError::StartFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-nodehost-{name}-{}", store::random_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A tar.gz built by hand so tests can hold entries a normal builder refuses.
    fn tgz(entries: &[(&str, tar::EntryType, &[u8], u32)]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, kind, data, mode) in entries {
            let mut h = tar::Header::new_gnu();
            {
                let raw = h.as_old_mut();
                raw.name[..name.len()].copy_from_slice(name.as_bytes());
            }
            h.set_entry_type(*kind);
            h.set_size(data.len() as u64);
            h.set_mode(*mode);
            if *kind == tar::EntryType::Symlink || *kind == tar::EntryType::Link {
                h.set_link_name("../../etc/passwd").unwrap();
            }
            h.set_cksum();
            b.append(&h, *data).unwrap();
        }
        let tar = b.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    fn good() -> Vec<u8> {
        tgz(&[
            ("bin/", tar::EntryType::Directory, b"", 0o755),
            ("node", tar::EntryType::Regular, b"#!fake", 0o755),
            ("out/server-main.js", tar::EntryType::Regular, b"// server", 0o644),
            ("node_modules/.bin/x", tar::EntryType::Symlink, b"", 0o777),
        ])
    }

    #[test]
    fn editor_folder_drops_the_verbatim_prefix_and_refuses_shares() {
        assert_eq!(editor_folder_str(r"\\?\D:\p2p\ws").unwrap(), r"D:\p2p\ws");
        assert_eq!(editor_folder_str(r"C:\work").unwrap(), r"C:\work");
        assert_eq!(editor_folder_str("/home/me/proj").unwrap(), "/home/me/proj");
        for share in [r"\\?\UNC\server\share\x", r"\\server\share", "//server/share"] {
            assert_eq!(editor_folder_str(share), Err(HostError::FolderNotLocal), "{share}");
        }
    }

    #[test]
    fn pins_fail_closed_and_check_shape() {
        // Every shipped pin is well formed and on an allowed host; unlisted platforms get nothing.
        for p in PINS {
            assert_eq!(pin_for(PINS, p.os, p.arch), Some(*p), "{p:?}");
            assert!(p.url.ends_with(".tar.gz") && p.url.contains(SERVER_BUILD) && allowed_url(p.url));
            assert_eq!(p.commit, SERVER_COMMIT);
        }
        assert_eq!(PINS.len(), 2);
        assert_eq!(pin_for(PINS, "macos", "aarch64"), None, "no builder yet");
        assert_eq!(pin_for(PINS, "windows", "aarch64"), None);
        assert_eq!(pin_for(PINS, "freebsd", "x86_64"), None);
        let ok = Pin {
            os: "linux",
            arch: "x86_64",
            version: "1",
            url: "https://github.com/a/b",
            sha256: Box::leak("a".repeat(64).into_boxed_str()),
            commit: Box::leak("b".repeat(40).into_boxed_str()),
        };
        assert!(pin_for(&[ok], "linux", "x86_64").is_some());
        for bad in [
            Pin { sha256: "", ..ok },
            Pin { sha256: Box::leak("A".repeat(64).into_boxed_str()), ..ok },
            Pin { url: "http://github.com/a", ..ok },
            Pin { url: "https://github.com@evil.example/a", ..ok },
            Pin { url: "https://evil.example/a", ..ok },
            Pin { commit: "", ..ok },
            Pin { commit: Box::leak("g".repeat(40).into_boxed_str()), ..ok },
        ] {
            assert_eq!(pin_for(&[bad], "linux", "x86_64"), None, "{bad:?}");
        }
    }

    #[test]
    fn download_needs_the_pinned_hash() {
        let body = good();
        let sum: &'static str = Box::leak(hex(&Sha256::digest(&body)).into_boxed_str());
        let pin = Pin {
            os: "linux",
            arch: "x86_64",
            version: "1.140.0.1",
            url: "https://github.com/x/y.tar.gz",
            sha256: sum,
            commit: "0123456789abcdef0123456789abcdef01234567",
        };
        let b2 = body.clone();
        let get = move |_: &str, _: u64| Ok(b2.clone());
        assert_eq!(download(&get, &pin).unwrap(), body);
        let tampered = |_: &str, _: u64| Ok(b"evil".to_vec());
        assert_eq!(download(&tampered, &pin), Err(HostError::HashMismatch));

        // Installs once, then reuses the folder without downloading again.
        let data = tmp("install");
        let calls = AtomicUsize::new(0);
        let b3 = body.clone();
        let counting = move |_: &str, _: u64| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(b3.clone())
        };
        let root = ensure_installed(&counting, &data, &pin).unwrap();
        assert!(root.join("out/server-main.js").is_file());
        assert!(!root.join("node_modules/.bin/x").exists(), "links are never made");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(root.join("node")).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(std::fs::metadata(root.join("out/server-main.js")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        ensure_installed(&counting, &data, &pin).unwrap();
        // A tampered download leaves nothing behind.
        let data2 = tmp("install-bad");
        assert_eq!(ensure_installed(&tampered, &data2, &pin), Err(HostError::HashMismatch));
        assert!(!install_dir(&data2, &pin).exists());
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&data2);
    }

    #[test]
    fn unpack_refuses_unsafe_entries() {
        let cases: Vec<Vec<u8>> = vec![
            tgz(&[("../evil", tar::EntryType::Regular, b"x", 0o644)]),
            tgz(&[("a/../../evil", tar::EntryType::Regular, b"x", 0o644)]),
            tgz(&[("/etc/evil", tar::EntryType::Regular, b"x", 0o644)]),
            tgz(&[("a\\..\\evil", tar::EntryType::Regular, b"x", 0o644)]),
            tgz(&[("C:evil", tar::EntryType::Regular, b"x", 0o644)]),
            tgz(&[("hard", tar::EntryType::Link, b"", 0o644)]),
            tgz(&[("dev", tar::EntryType::Char, b"", 0o644)]),
            tgz(&[("a", tar::EntryType::Regular, b"1", 0o644), ("a", tar::EntryType::Regular, b"2", 0o644)]),
            b"not a gzip".to_vec(),
        ];
        for (i, c) in cases.iter().enumerate() {
            let base = tmp("unsafe");
            let dest = base.join("d");
            assert!(unpack_tgz(c, &dest).is_err(), "case {i}");
            assert!(!base.join("evil").exists(), "case {i}");
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn trust_is_per_canonical_folder() {
        let data = tmp("trust");
        let folder = tmp("folder");
        let t = TrustStore::new(&data);
        assert!(!t.is_trusted(&folder));
        // A folder that does not exist is refused. (Windows resolves `missing\..` by name, so the missing folder
        // itself is used here, not a path through it.)
        t.set(&folder.join("missing"), true).unwrap_err();
        std::fs::create_dir(folder.join("sub")).unwrap();
        t.set(&folder.join("sub").join(".."), true).unwrap();
        assert!(t.is_trusted(&folder));
        assert!(!t.is_trusted(&folder.join("sub")), "trust does not spread to other folders");
        t.set(&folder, false).unwrap();
        assert!(!t.is_trusted(&folder));
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn env_and_port_parsing() {
        let env = scrubbed_env(
            [
                ("PATH", "/bin"),
                ("P2P_GATEWAY_KEY", "k"),
                ("GITHUB_TOKEN", "t"),
                ("HOME", "/h"),
                ("AWS_SECRET_ACCESS_KEY", "s"),
            ]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string())),
        );
        let names: Vec<_> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["PATH", "HOME"]);
        assert_eq!(parse_listen("Extension host agent listening on 40123"), Some(40123));
        assert_eq!(parse_listen("Extension host agent listening on 127.0.0.1:40124"), Some(40124));
        for bad in ["listening on 80", "listening on 99999", "listening on 4a", "hello", "listening on "] {
            assert_eq!(parse_listen(bad), None, "{bad}");
        }
    }

    #[test]
    fn stage_copies_only_enabled_node_extensions() {
        use crate::extensions::{ExtStore, Installed, Kind};
        let base = tmp("stage");
        let ext_dir = base.join("extensions");
        let entry = |id: &str, kind: Kind, enabled: bool| Installed {
            id: id.into(),
            namespace: "acme".into(),
            name: id.into(),
            version: "1.0.0".into(),
            display_name: id.into(),
            description: String::new(),
            kind,
            enabled,
            installed_at: 0,
            sha256: "0".repeat(64),
            verified_publisher: false,
            folder: format!("{id}-1.0.0"),
            network: false,
        };
        let list = vec![
            entry("acme.on", Kind::Node, true),
            entry("acme.off", Kind::Node, false),
            entry("acme.web", Kind::Web, true),
        ];
        // The store's own layout: the `.vsix`'s `extension/` contents directly in `<folder>/` (found on the PC:
        // an earlier fixture used `<folder>/extension/`, so the real install failed to stage).
        for e in &list {
            let d = ext_dir.join(&e.folder).join("out");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("ext.js"), "x").unwrap();
            std::fs::write(ext_dir.join(&e.folder).join("package.json"), "{}").unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", ext_dir.join("acme.on-1.0.0/out/link")).unwrap();
        std::fs::write(ext_dir.join("installed.json"), serde_json::to_vec(&list).unwrap()).unwrap();
        let store = ExtStore::new(ext_dir);
        let dest = base.join("node-extensions");
        std::fs::create_dir_all(dest.join("stale-1.0.0")).unwrap();
        assert_eq!(stage_extensions(&store, &dest).unwrap(), 1);
        assert!(dest.join("acme.on-1.0.0/package.json").is_file());
        assert!(dest.join("acme.on-1.0.0/out/ext.js").is_file());
        assert!(!dest.join("acme.on-1.0.0/out/link").exists(), "links are never copied");
        assert!(!dest.join("acme.off-1.0.0").exists() && !dest.join("acme.web-1.0.0").exists());
        assert!(!dest.join("stale-1.0.0").exists(), "rebuilt fresh on every start");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn start_needs_trust_and_passes_no_secrets() {
        use std::os::unix::fs::PermissionsExt;
        let data = tmp("start");
        let folder = tmp("start-folder");
        let fake = data.join("fake-node.sh");
        // The fake server records its arguments and environment, then says its port and waits.
        let rec = data.join("rec");
        std::fs::create_dir(&rec).unwrap();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{0}/args.txt'; env > '{0}/env.txt'\n\
             echo 'Extension host agent listening on 45678'\nexec sleep 30\n",
            rec.display()
        );
        std::fs::write(&fake, script).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let launcher = Launcher { program: fake, args: vec!["server-main.js".into()], env: vec![] };
        let trust = TrustStore::new(&data);
        assert!(matches!(start(&launcher, &data, &trust, &folder), Err(HostError::NotTrusted)));
        trust.set(&folder, true).unwrap();
        std::env::set_var("P2P_TEST_GATEWAY_KEY", "secret-value");
        let run = start(&launcher, &data, &trust, &folder).unwrap();
        std::env::remove_var("P2P_TEST_GATEWAY_KEY");
        assert_eq!(run.port, 45678);
        assert_eq!(run.token.len(), 48);
        let token_file = data.join("node-host-state/connection-token");
        assert_eq!(std::fs::read_to_string(&token_file).unwrap(), run.token);
        assert_eq!(std::fs::metadata(&token_file).unwrap().permissions().mode() & 0o777, 0o600);
        let args = std::fs::read_to_string(rec.join("args.txt")).unwrap();
        assert!(args.contains("127.0.0.1") && args.contains("--connection-token-file"));
        assert!(!args.contains(&run.token), "the token is never on the command line");
        let env = std::fs::read_to_string(rec.join("env.txt")).unwrap();
        assert!(!env.contains("secret-value"));
        run.stop();
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
