//! extensions -- VS Code / VSCodium extensions from Open VSX: search, verified install, list, enable, remove
//! (phase I6a, ADR-029 D1; docs/features/extensions.md).
//!
//! Purpose: the Extensions view searches Open VSX (the registry VSCodium uses) and installs an extension only when the
//! person clicks Install. This module downloads the `.vsix`, checks it, unpacks it into the app's own data folder and
//! keeps the list of installed extensions. The webview then reads the files an extension contributes (themes,
//! grammars, snippets, language settings) through [`ExtStore::read_file`] and hands them to the editor services.
//!
//! Phase I6a installs **declarative** extensions only: no `main` (Node code) and no `browser` (web worker code), so
//! nothing an extension ships is ever run. Extensions with code are refused with a message saying which phase brings
//! their host (I6b web worker, I6c Node sidecar).
//!
//! Inputs and trust boundaries:
//!   * Search text, namespace, name and version come from the webview (untrusted): allow-listed before they reach a URL.
//!   * Open VSX answers and the `.vsix` are untrusted network data: JSON parsed into fixed shapes with size caps; the
//!     archive is checked entry by entry before anything is written.
//!
//! Security controls:
//!   * Network: https only, Mozilla roots, no proxy from the environment, public addresses only
//!     ([`crate::discovery::PublicOnly`], CWE-918). Redirects are followed by hand, at most one, and only to the
//!     Open VSX hosts in [`ALLOWED_HOSTS`] (CWE-601).
//!   * Integrity and origin (NIST SI-7, SSDF PS.2, CWE-494): the `.vsix` must match the SHA-256 Open VSX publishes and
//!     carry a valid Ed25519 signature (Open VSX's `.sigzip`) made with the key Open VSX publishes for that version.
//!     Unsigned extensions are refused. The key comes from open-vsx.org, the file usually from its download CDN, so a
//!     changed file on the CDN fails the check.
//!   * Archive (CWE-22 zip slip, CWE-409 zip bomb, CWE-59): only entries under `extension/` are unpacked; every path
//!     part is allow-listed (no `..`, no absolute or drive paths, no backslashes or control characters); at most
//!     10,000 files, 50 MiB each and 300 MiB in total, counted on the bytes actually read; links are never created.
//!     Files are written through a cap-std handle on a fresh folder, then the folder is renamed into place.
//!   * The manifest must name the same publisher and name that were asked for (CWE-345).
//!   * Caps on every download: metadata 1 MiB, `.vsix` 50 MiB, signature 1 MiB, key 4 KiB (CWE-400).
//!   * Errors are fixed messages by kind (CWE-209); installs and removals are audited by the shell (NIST AU-2).
//!
//! Dependencies: ureq 3 (already used for gateways), sha2, ed25519-dalek 2 and base64 (already used for adverts),
//! cap-std (already used for the workspace), and zip 9 with only the pure-Rust deflate reader (new; the zip-rs
//! project's maintained crate, MIT; `.vsix` files are zip archives).

use crate::store::{self, StoreError};
use base64::Engine;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The registry. Open VSX is the only source (ADR-029: the Microsoft Marketplace terms allow only Microsoft products).
pub const OPEN_VSX: &str = "https://open-vsx.org";
/// Hosts an Open VSX file may come from: the API itself and its download CDN.
pub const ALLOWED_HOSTS: [&str; 2] = ["open-vsx.org", "openvsx.eclipsecontent.org"];

const MAX_META: u64 = 1 << 20;
const MAX_VSIX: u64 = 50 << 20;
const MAX_SIG: u64 = 1 << 20;
const MAX_KEY: u64 = 4096;
const MAX_ENTRIES: usize = 10_000;
const MAX_FILE: u64 = 50 << 20;
const MAX_TOTAL: u64 = 300 << 20;
const MAX_LIST: u64 = 1 << 20;
/// Search answers are at most this many results.
pub const PAGE: usize = 20;

/// Fixed error kinds; [`ExtError::message`] is safe to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtError {
    /// A namespace, name, version or search text was not allowed.
    BadInput,
    /// Open VSX could not be reached or answered with an error.
    Network,
    /// An answer was larger than its cap.
    TooLarge,
    /// An answer or archive was not in the expected shape.
    BadPackage,
    /// The file does not match its published SHA-256.
    HashMismatch,
    /// The extension has no signature, or the signature does not verify.
    BadSignature,
    /// The archive holds a path that could leave the extension folder.
    UnsafePath,
    /// The extension runs code; `kind` says which host it needs.
    NeedsHost(Kind),
    /// Not installed, or no such file in the extension.
    NotFound,
    /// The app's data folder could not be read or written.
    Io,
}

impl ExtError {
    /// A short message with no path, URL or OS detail (CWE-209).
    pub fn message(&self) -> String {
        match self {
            ExtError::BadInput => "that name or search text is not allowed".into(),
            ExtError::Network => "Open VSX could not be reached or refused the request".into(),
            ExtError::TooLarge => "the extension or an answer is larger than allowed".into(),
            ExtError::BadPackage => "the extension package is damaged or not a VS Code extension".into(),
            ExtError::HashMismatch => "the download does not match its published SHA-256; nothing was installed".into(),
            ExtError::BadSignature => "the extension has no valid Open VSX signature; nothing was installed".into(),
            ExtError::UnsafePath => "the extension holds a file path that is not allowed; nothing was installed".into(),
            ExtError::NeedsHost(Kind::Web) => {
                "this extension runs code in a web worker; that extension host arrives in phase I6b".into()
            }
            ExtError::NeedsHost(_) => {
                "this extension runs Node.js code; the Node extension host arrives in phase I6c".into()
            }
            ExtError::NotFound => "that extension or file is not installed".into(),
            ExtError::Io => "could not read or write the app's extensions folder".into(),
        }
    }
}

impl From<StoreError> for ExtError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Missing => ExtError::NotFound,
            StoreError::Bad => ExtError::BadPackage,
            StoreError::Io => ExtError::Io,
        }
    }
}

/// What an extension needs to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Only contributions read from files (themes, grammars, snippets, ...). No code runs.
    Declarative,
    /// A `browser` entry: code for the web worker extension host.
    Web,
    /// A `main` entry: Node.js code for the Node extension host.
    Node,
}

/// One search result, as the webview shows it (plain text only).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Summary {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub downloads: u64,
}

/// One installed extension, saved in `installed.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Installed {
    /// `namespace.name`, lowercase.
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub kind: Kind,
    pub enabled: bool,
    pub installed_at: i64,
    /// SHA-256 of the verified `.vsix`, hex.
    pub sha256: String,
    /// Whether Open VSX marks the namespace as verified (its owner proved who they are).
    pub verified_publisher: bool,
    /// Folder name under the extensions folder.
    pub folder: String,
}

/// The network seam: GET a URL, return at most `max` bytes. Tests pass a fake; [`real_get`] is the real one.
pub type Get<'a> = &'a (dyn Fn(&str, u64) -> Result<Vec<u8>, ExtError> + Sync);

fn host_of(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    // SECURITY: no user info and no port, so "open-vsx.org@evil" or "open-vsx.org:1" cannot pass (CWE-601).
    if host.contains('@') || host.contains(':') {
        return None;
    }
    Some(host)
}

/// True when `url` is https on one of [`ALLOWED_HOSTS`].
pub fn allowed_url(url: &str) -> bool {
    host_of(url).is_some_and(|h| ALLOWED_HOSTS.iter().any(|a| a.eq_ignore_ascii_case(h)))
}

/// The real network seam: https only, public addresses only, one redirect at most and only to an allowed host.
pub fn real_get() -> impl Fn(&str, u64) -> Result<Vec<u8>, ExtError> + Sync {
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .https_only(true)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent("p2p-ai-ide/0.1")
        .build();
    let agent = ureq::Agent::with_parts(
        config,
        ureq::unversioned::transport::DefaultConnector::default(),
        crate::discovery::PublicOnly,
    );
    move |url: &str, max: u64| {
        let mut target = url.to_string();
        for hop in 0..2 {
            if !allowed_url(&target) {
                return Err(ExtError::Network);
            }
            let resp = agent.get(&target).call().map_err(|_| ExtError::Network)?;
            let status = resp.status().as_u16();
            if (300..400).contains(&status) && hop == 0 {
                let loc = resp.headers().get("location").and_then(|v| v.to_str().ok()).ok_or(ExtError::Network)?;
                target = loc.to_string();
                continue;
            }
            if status != 200 {
                return Err(ExtError::Network);
            }
            let mut buf = vec![];
            resp.into_body().into_reader().take(max + 1).read_to_end(&mut buf).map_err(|_| ExtError::Network)?;
            if buf.len() as u64 > max {
                return Err(ExtError::TooLarge);
            }
            return Ok(buf);
        }
        Err(ExtError::Network)
    }
}

/// Allow-list for a namespace or name: 1-100 of `A-Z a-z 0-9 - _ .`, not starting with a dot (CWE-20).
pub fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && !s.starts_with('.')
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Allow-list for a version: 1-64 of `0-9 A-Z a-z . - +`, or `latest`.
pub fn version_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
        && !s.contains("..")
}

/// Percent-encodes search text for a query string (unreserved characters stay).
fn query_escape(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn text(v: &serde_json::Value, key: &str, max: usize) -> String {
    let s = v.get(key).and_then(|x| x.as_str()).unwrap_or("");
    // SECURITY: strip control characters and cap length; the webview renders it as text (CWE-79, CWE-117).
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// Searches Open VSX. `query` is 1-100 characters without control characters; `offset` is a multiple of [`PAGE`].
pub fn search(get: Get, query: &str, offset: usize) -> Result<Vec<Summary>, ExtError> {
    let q = query.trim();
    if q.is_empty() || q.chars().count() > 100 || q.chars().any(char::is_control) || offset > 10_000 {
        return Err(ExtError::BadInput);
    }
    let url = format!("{OPEN_VSX}/api/-/search?query={}&size={PAGE}&offset={offset}", query_escape(q));
    let body = get(&url, MAX_META)?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|_| ExtError::BadPackage)?;
    let list = v.get("extensions").and_then(|x| x.as_array()).ok_or(ExtError::BadPackage)?;
    Ok(list
        .iter()
        .take(PAGE)
        .filter_map(|e| {
            let s = Summary {
                namespace: text(e, "namespace", 100),
                name: text(e, "name", 100),
                version: text(e, "version", 64),
                display_name: text(e, "displayName", 200),
                description: text(e, "description", 500),
                downloads: e.get("downloadCount").and_then(|x| x.as_u64()).unwrap_or(0),
            };
            (name_ok(&s.namespace) && name_ok(&s.name) && version_ok(&s.version)).then_some(s)
        })
        .collect())
}

/// Parses Open VSX's PEM public key: an Ed25519 SubjectPublicKeyInfo (RFC 8410). Anything else is refused.
fn parse_ed25519_pem(pem: &[u8]) -> Result<ed25519_dalek::VerifyingKey, ExtError> {
    const SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
    let text = std::str::from_utf8(pem).map_err(|_| ExtError::BadSignature)?;
    let b64: String = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("-----")).collect();
    if !text.contains("-----BEGIN PUBLIC KEY-----") {
        return Err(ExtError::BadSignature);
    }
    let der = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|_| ExtError::BadSignature)?;
    if der.len() != 44 || der[..12] != SPKI_PREFIX {
        return Err(ExtError::BadSignature);
    }
    let key: [u8; 32] = der[12..].try_into().map_err(|_| ExtError::BadSignature)?;
    ed25519_dalek::VerifyingKey::from_bytes(&key).map_err(|_| ExtError::BadSignature)
}

/// Reads one small entry of a zip archive, by exact name, within `max` bytes.
fn zip_entry(bytes: &[u8], name: &str, max: u64) -> Result<Vec<u8>, ExtError> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ExtError::BadPackage)?;
    let f = z.by_name(name).map_err(|_| ExtError::BadPackage)?;
    let mut out = vec![];
    f.take(max + 1).read_to_end(&mut out).map_err(|_| ExtError::BadPackage)?;
    if out.len() as u64 > max {
        return Err(ExtError::TooLarge);
    }
    Ok(out)
}

/// Checks the `.sigzip`: its `.signature.sig` must be a valid Ed25519 signature of the whole `.vsix` by `key`.
pub fn verify_signature(vsix: &[u8], sigzip: &[u8], key_pem: &[u8]) -> Result<(), ExtError> {
    use ed25519_dalek::Verifier;
    let key = parse_ed25519_pem(key_pem)?;
    let sig = zip_entry(sigzip, ".signature.sig", 1024).map_err(|_| ExtError::BadSignature)?;
    let sig: [u8; 64] = sig.as_slice().try_into().map_err(|_| ExtError::BadSignature)?;
    // SECURITY: strict verification over every byte of the downloaded file (NIST SI-7, CWE-347).
    key.verify(vsix, &ed25519_dalek::Signature::from_bytes(&sig)).map_err(|_| ExtError::BadSignature)
}

/// Decides what an extension needs from its `package.json`.
pub fn kind_of(manifest: &serde_json::Value) -> Kind {
    let has = |k: &str| manifest.get(k).and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty());
    if has("main") {
        Kind::Node
    } else if has("browser") {
        Kind::Web
    } else {
        Kind::Declarative
    }
}

/// Splits an archive entry name into safe parts below `extension/`, or `None` for entries that are not unpacked.
/// Errors on any part that could leave the folder.
fn safe_parts(name: &str) -> Result<Option<Vec<String>>, ExtError> {
    let Some(rest) = name.strip_prefix("extension/") else { return Ok(None) };
    if rest.is_empty() || rest.ends_with('/') {
        return Ok(None);
    }
    let mut parts = vec![];
    for p in rest.split('/') {
        // SECURITY: allow-list every part (CWE-22): no empty, ".", ".." parts, no backslash, colon (drive or
        // alternate data stream) or control characters, and no Windows reserved trailing dot or space.
        if p.is_empty()
            || p == "."
            || p == ".."
            || p.len() > 255
            || p.ends_with('.')
            || p.ends_with(' ')
            || p.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        {
            return Err(ExtError::UnsafePath);
        }
        parts.push(p.to_string());
    }
    if parts.len() > 32 {
        return Err(ExtError::UnsafePath);
    }
    Ok(Some(parts))
}

/// The installed extensions, kept under `<app data>/extensions/`.
pub struct ExtStore {
    dir: PathBuf,
}

impl ExtStore {
    /// `dir` is the app's own extensions folder, chosen by the shell (never by the webview).
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn list_path(&self) -> PathBuf {
        self.dir.join("installed.json")
    }

    /// Every installed extension, in install order.
    pub fn list(&self) -> Result<Vec<Installed>, ExtError> {
        match store::read_capped(&self.list_path(), MAX_LIST) {
            Ok(b) => serde_json::from_slice(&b).map_err(|_| ExtError::BadPackage),
            Err(StoreError::Missing) => Ok(vec![]),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, list: &[Installed]) -> Result<(), ExtError> {
        let bytes = serde_json::to_vec_pretty(list).map_err(|_| ExtError::Io)?;
        store::write_atomic(&self.list_path(), &bytes)?;
        Ok(())
    }

    fn find(&self, id: &str) -> Result<Installed, ExtError> {
        self.list()?.into_iter().find(|e| e.id == id).ok_or(ExtError::NotFound)
    }

    /// Downloads, verifies and unpacks one extension. `version` is a version or `latest`.
    ///
    /// Security: see the module notes; nothing is written before the hash, signature, manifest and every archive
    /// path have been checked, and a failed unpack leaves no folder behind.
    pub fn install(&self, get: Get, namespace: &str, name: &str, version: &str) -> Result<Installed, ExtError> {
        if !name_ok(namespace) || !name_ok(name) || !version_ok(version) {
            return Err(ExtError::BadInput);
        }
        let meta_url = format!("{OPEN_VSX}/api/{namespace}/{name}/{version}");
        let meta: serde_json::Value =
            serde_json::from_slice(&get(&meta_url, MAX_META)?).map_err(|_| ExtError::BadPackage)?;
        let files = meta.get("files").ok_or(ExtError::BadPackage)?;
        let file_url = |k: &str| -> Result<String, ExtError> {
            let u = files.get(k).and_then(|v| v.as_str()).ok_or(if k == "signature" || k == "publicKey" {
                ExtError::BadSignature
            } else {
                ExtError::BadPackage
            })?;
            // SECURITY: only Open VSX's own hosts, https (CWE-918, CWE-601).
            if !allowed_url(u) {
                return Err(ExtError::BadPackage);
            }
            Ok(u.to_string())
        };
        let (dl, sha, sig, key) =
            (file_url("download")?, file_url("sha256")?, file_url("signature")?, file_url("publicKey")?);
        let real_version = text(&meta, "version", 64);
        if !version_ok(&real_version) || real_version == "latest" {
            return Err(ExtError::BadPackage);
        }

        let vsix = get(&dl, MAX_VSIX)?;
        let want = String::from_utf8(get(&sha, 1024)?).map_err(|_| ExtError::BadPackage)?;
        let want = want.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        let got: String = Sha256::digest(&vsix).iter().map(|b| format!("{b:02x}")).collect();
        if want.len() != 64 || want != got {
            return Err(ExtError::HashMismatch);
        }
        verify_signature(&vsix, &get(&sig, MAX_SIG)?, &get(&key, MAX_KEY)?)?;

        // The manifest decides the kind and must name what was asked for.
        let manifest: serde_json::Value =
            serde_json::from_slice(&zip_entry(&vsix, "extension/package.json", MAX_META)?)
                .map_err(|_| ExtError::BadPackage)?;
        let publisher = text(&manifest, "publisher", 100);
        let mname = text(&manifest, "name", 100);
        if !publisher.eq_ignore_ascii_case(namespace) || !mname.eq_ignore_ascii_case(name) {
            return Err(ExtError::BadPackage);
        }
        let kind = kind_of(&manifest);
        if kind != Kind::Declarative {
            return Err(ExtError::NeedsHost(kind));
        }

        let id = format!("{}.{}", namespace.to_ascii_lowercase(), name.to_ascii_lowercase());
        let folder = format!("{id}-{real_version}");
        store::ensure_dir(&self.dir)?;
        let staging = self.dir.join(format!(".staging-{}", store::random_id()));
        let result = unpack(&vsix, &staging);
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
        let target = self.dir.join(&folder);
        let mut list = self.list()?;
        // Replace an older copy of the same extension (update or reinstall).
        if let Some(old) = list.iter().find(|e| e.id == id) {
            let _ = std::fs::remove_dir_all(self.dir.join(&old.folder));
        }
        let _ = std::fs::remove_dir_all(&target);
        if std::fs::rename(&staging, &target).is_err() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(ExtError::Io);
        }
        let entry = Installed {
            id: id.clone(),
            namespace: namespace.to_string(),
            name: name.to_string(),
            version: real_version,
            display_name: text(&manifest, "displayName", 200),
            description: text(&manifest, "description", 500),
            kind,
            enabled: true,
            installed_at: store::now_s(),
            sha256: got,
            verified_publisher: meta.get("verified").and_then(|v| v.as_bool()).unwrap_or(false),
            folder,
        };
        list.retain(|e| e.id != id);
        list.push(entry.clone());
        self.save(&list)?;
        Ok(entry)
    }

    /// Turns an extension on or off (off: kept on disk, not given to the editor).
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<Installed, ExtError> {
        let mut list = self.list()?;
        let e = list.iter_mut().find(|e| e.id == id).ok_or(ExtError::NotFound)?;
        e.enabled = enabled;
        let out = e.clone();
        self.save(&list)?;
        Ok(out)
    }

    /// Removes an extension and its folder.
    pub fn uninstall(&self, id: &str) -> Result<(), ExtError> {
        let mut list = self.list()?;
        let Some(pos) = list.iter().position(|e| e.id == id) else { return Err(ExtError::NotFound) };
        let e = list.remove(pos);
        // SECURITY: the folder name comes from our own saved list, built from allow-listed parts (CWE-22).
        let _ = std::fs::remove_dir_all(self.dir.join(&e.folder));
        self.save(&list)
    }

    /// The extension's `package.json`, parsed.
    pub fn manifest(&self, id: &str) -> Result<serde_json::Value, ExtError> {
        serde_json::from_slice(&self.read_file(id, "package.json")?).map_err(|_| ExtError::BadPackage)
    }

    /// One file of an installed extension, by its path inside the extension (`themes/dark.json`).
    ///
    /// Security: opened relative to a cap-std handle on the extension's folder, so `..`, absolute paths and links
    /// that point outside cannot be followed (CWE-22, CWE-59); at most [`MAX_FILE`] bytes (CWE-400).
    pub fn read_file(&self, id: &str, rel: &str) -> Result<Vec<u8>, ExtError> {
        let e = self.find(id)?;
        let rel = rel.trim_start_matches("./");
        let parts = safe_parts(&format!("extension/{rel}"))?.ok_or(ExtError::NotFound)?;
        let dir = Dir::open_ambient_dir(self.dir.join(&e.folder), ambient_authority()).map_err(|_| ExtError::Io)?;
        let path: PathBuf = parts.iter().collect();
        let f = dir.open(&path).map_err(|_| ExtError::NotFound)?;
        let meta = f.metadata().map_err(|_| ExtError::Io)?;
        if !meta.is_file() || meta.len() > MAX_FILE {
            return Err(ExtError::NotFound);
        }
        let mut out = vec![];
        f.into_std().take(MAX_FILE + 1).read_to_end(&mut out).map_err(|_| ExtError::Io)?;
        Ok(out)
    }

    /// The folder that holds the extensions (for the shell's audit lines only).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Unpacks the `extension/` entries of `vsix` into the new folder `dest`, with every check in the module notes.
fn unpack(vsix: &[u8], dest: &Path) -> Result<(), ExtError> {
    let mut z = zip::ZipArchive::new(Cursor::new(vsix)).map_err(|_| ExtError::BadPackage)?;
    if z.len() > MAX_ENTRIES {
        return Err(ExtError::TooLarge);
    }
    // Check every name before writing anything.
    let mut plan = vec![];
    for i in 0..z.len() {
        let f = z.by_index_raw(i).map_err(|_| ExtError::BadPackage)?;
        if f.is_symlink() {
            return Err(ExtError::UnsafePath);
        }
        let entry_name = f.name().map_err(|_| ExtError::UnsafePath)?;
        if let Some(parts) = safe_parts(&entry_name)? {
            plan.push((i, parts));
        }
    }
    std::fs::create_dir(dest).map_err(|_| ExtError::Io)?;
    let root = Dir::open_ambient_dir(dest, ambient_authority()).map_err(|_| ExtError::Io)?;
    let mut total: u64 = 0;
    for (i, parts) in plan {
        let mut f = z.by_index(i).map_err(|_| ExtError::BadPackage)?;
        if f.is_dir() {
            continue;
        }
        let (file, dirs) = parts.split_last().ok_or(ExtError::UnsafePath)?;
        let sub: PathBuf = dirs.iter().collect();
        if !dirs.is_empty() {
            root.create_dir_all(&sub).map_err(|_| ExtError::Io)?;
        }
        // SECURITY: create_new so an entry can never overwrite another or follow a link (CWE-59).
        let mut opts = cap_std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        let mut out = root.open_with(sub.join(file), &opts).map_err(|_| ExtError::UnsafePath)?;
        // SECURITY: count real bytes, not the archive's claimed sizes (CWE-409).
        let mut buf = [0u8; 64 * 1024];
        let mut this: u64 = 0;
        loop {
            let n = f.read(&mut buf).map_err(|_| ExtError::BadPackage)?;
            if n == 0 {
                break;
            }
            this += n as u64;
            total += n as u64;
            if this > MAX_FILE || total > MAX_TOTAL {
                return Err(ExtError::TooLarge);
            }
            out.write_all(&buf[..n]).map_err(|_| ExtError::Io)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(vec![]));
        let opts: zip::write::SimpleFileOptions =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (n, b) in entries {
            w.start_file(*n, opts).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn pem_of(k: &SigningKey) -> String {
        let mut der = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
        der.extend_from_slice(k.verifying_key().as_bytes());
        format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            base64::engine::general_purpose::STANDARD.encode(der)
        )
    }

    struct Fake {
        files: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl Fake {
        /// A fake Open VSX serving one extension; `tamper` edits the published files after signing.
        fn new(manifest: &str, extra: &[(&str, &[u8])], tamper: impl FnOnce(&mut HashMap<String, Vec<u8>>)) -> Self {
            let mut entries: Vec<(&str, &[u8])> = vec![("extension/package.json", manifest.as_bytes())];
            entries.extend_from_slice(extra);
            let vsix = zip_of(&entries);
            let key = SigningKey::from_bytes(&[7u8; 32]);
            let sig = key.sign(&vsix);
            let sigzip = zip_of(&[(".signature.sig", &sig.to_bytes()), (".signature.manifest", b"{}")]);
            let base = "https://open-vsx.org/api/acme/theme-x/1.0.0/file/acme.theme-x-1.0.0";
            let meta = serde_json::json!({
                "version": "1.0.0", "verified": true,
                "files": {
                    "download": format!("{base}.vsix"), "sha256": format!("{base}.sha256"),
                    "signature": format!("{base}.sigzip"), "publicKey": "https://open-vsx.org/api/-/public-key/k1"
                }
            });
            let sha: String = Sha256::digest(&vsix).iter().map(|b| format!("{b:02x}")).collect();
            let mut m = HashMap::new();
            m.insert("https://open-vsx.org/api/acme/theme-x/latest".to_string(), meta.to_string().into_bytes());
            m.insert(format!("{base}.vsix"), vsix);
            m.insert(format!("{base}.sha256"), format!("{sha}  acme.theme-x-1.0.0.vsix").into_bytes());
            m.insert(format!("{base}.sigzip"), sigzip);
            m.insert("https://open-vsx.org/api/-/public-key/k1".to_string(), pem_of(&key).into_bytes());
            tamper(&mut m);
            Fake { files: Mutex::new(m) }
        }

        fn get(&self, url: &str, max: u64) -> Result<Vec<u8>, ExtError> {
            let b = self.files.lock().unwrap().get(url).cloned().ok_or(ExtError::Network)?;
            if b.len() as u64 > max {
                return Err(ExtError::TooLarge);
            }
            Ok(b)
        }
    }

    const THEME: &str = r#"{"name":"theme-x","publisher":"acme","displayName":"Theme X","version":"1.0.0",
        "contributes":{"themes":[{"label":"X Dark","uiTheme":"vs-dark","path":"./themes/x.json"}]}}"#;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-ext-{}", store::random_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn installs_a_signed_declarative_extension_and_reads_its_files() {
        let fake = Fake::new(THEME, &[("extension/themes/x.json", b"{\"colors\":{}}")], |_| {});
        let store = ExtStore::new(tmp());
        let e = store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest").unwrap();
        assert_eq!(
            (e.id.as_str(), e.version.as_str(), e.kind, e.enabled),
            ("acme.theme-x", "1.0.0", Kind::Declarative, true)
        );
        assert!(e.verified_publisher);
        assert_eq!(store.read_file("acme.theme-x", "./themes/x.json").unwrap(), b"{\"colors\":{}}");
        assert_eq!(store.manifest("acme.theme-x").unwrap()["displayName"], "Theme X");
        assert_eq!(store.list().unwrap().len(), 1);
        store.set_enabled("acme.theme-x", false).unwrap();
        assert!(!store.list().unwrap()[0].enabled);
        store.uninstall("acme.theme-x").unwrap();
        assert!(store.list().unwrap().is_empty());
        assert_eq!(store.read_file("acme.theme-x", "package.json"), Err(ExtError::NotFound));
    }

    #[test]
    fn refuses_a_changed_file() {
        let fake = Fake::new(THEME, &[], |m| {
            let k = "https://open-vsx.org/api/acme/theme-x/1.0.0/file/acme.theme-x-1.0.0.vsix";
            m.get_mut(k).unwrap().push(0);
        });
        let store = ExtStore::new(tmp());
        assert_eq!(store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest"), Err(ExtError::HashMismatch));
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn refuses_a_bad_or_missing_signature() {
        // Same file and hash, but the key is a different one: the signature does not verify.
        let fake = Fake::new(THEME, &[], |m| {
            let other = SigningKey::from_bytes(&[9u8; 32]);
            m.insert("https://open-vsx.org/api/-/public-key/k1".into(), pem_of(&other).into_bytes());
        });
        let store = ExtStore::new(tmp());
        assert_eq!(store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest"), Err(ExtError::BadSignature));
        let unsigned = Fake::new(THEME, &[], |m| {
            let k = "https://open-vsx.org/api/acme/theme-x/latest".to_string();
            let mut meta: serde_json::Value = serde_json::from_slice(&m[&k]).unwrap();
            meta["files"].as_object_mut().unwrap().remove("signature");
            m.insert(k, meta.to_string().into_bytes());
        });
        assert_eq!(store.install(&|u, m| unsigned.get(u, m), "acme", "theme-x", "latest"), Err(ExtError::BadSignature));
    }

    #[test]
    fn refuses_extensions_with_code() {
        let node = THEME.replace("\"displayName\"", "\"main\":\"./out/ext.js\",\"displayName\"");
        let fake = Fake::new(&node, &[], |_| {});
        let store = ExtStore::new(tmp());
        assert_eq!(
            store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest"),
            Err(ExtError::NeedsHost(Kind::Node))
        );
        let web = THEME.replace("\"displayName\"", "\"browser\":\"./dist/web.js\",\"displayName\"");
        let fake = Fake::new(&web, &[], |_| {});
        assert_eq!(
            store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest"),
            Err(ExtError::NeedsHost(Kind::Web))
        );
    }

    #[test]
    fn refuses_a_manifest_for_another_extension() {
        let other = THEME.replace("\"publisher\":\"acme\"", "\"publisher\":\"evil\"");
        let fake = Fake::new(&other, &[], |_| {});
        let store = ExtStore::new(tmp());
        assert_eq!(store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest"), Err(ExtError::BadPackage));
    }

    #[test]
    fn refuses_zip_slip_and_leaves_nothing() {
        for bad in ["extension/../evil.txt", "extension/a/../../evil", "extension/C:\\x", "extension/a\\..\\b"] {
            let fake = Fake::new(THEME, &[(bad, b"x")], |_| {});
            let dir = tmp();
            let store = ExtStore::new(dir.clone());
            assert_eq!(
                store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest"),
                Err(ExtError::UnsafePath),
                "{bad}"
            );
            let left: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).collect();
            assert!(left.is_empty(), "{bad}: something was written");
        }
    }

    #[test]
    fn read_file_cannot_leave_the_extension() {
        let fake = Fake::new(THEME, &[], |_| {});
        let store = ExtStore::new(tmp());
        store.install(&|u, m| fake.get(u, m), "acme", "theme-x", "latest").unwrap();
        for bad in ["../installed.json", "/etc/passwd", "a/../../installed.json", "..\\installed.json"] {
            assert!(store.read_file("acme.theme-x", bad).is_err(), "{bad}");
        }
    }

    /// Live check against the real Open VSX: `cargo test --lib extensions::tests::live -- --ignored`.
    #[test]
    #[ignore]
    fn live_open_vsx_install() {
        let get = real_get();
        let found = search(&get, "dracula", 0).unwrap();
        assert!(found.iter().any(|s| s.namespace == "dracula-theme"), "search found Dracula");
        let store = ExtStore::new(tmp());
        let e = store.install(&get, "dracula-theme", "theme-dracula", "latest").unwrap();
        assert_eq!((e.kind, e.enabled), (Kind::Declarative, true));
        let m = store.manifest(&e.id).unwrap();
        let theme = m["contributes"]["themes"][0]["path"].as_str().unwrap();
        assert!(store.read_file(&e.id, theme).unwrap().len() > 1000);
        // A Node extension is refused before anything is written.
        assert_eq!(
            store.install(&get, "redhat", "vscode-yaml", "latest").unwrap_err(),
            ExtError::NeedsHost(Kind::Node)
        );
        println!("installed {} {} sha256 {}", e.id, e.version, e.sha256);
    }

    #[test]
    fn input_allow_lists() {
        assert!(name_ok("dracula-theme") && name_ok("theme_x.y") && !name_ok("../x") && !name_ok("a/b"));
        assert!(!name_ok("") && !name_ok(".hidden") && !name_ok("a b"));
        assert!(version_ok("1.2.3-beta+4") && version_ok("latest") && !version_ok("1/2") && !version_ok("1..2"));
        assert!(allowed_url("https://open-vsx.org/api/x") && allowed_url("https://openvsx.eclipsecontent.org/a"));
        for bad in [
            "http://open-vsx.org/x",
            "https://open-vsx.org.evil.com/x",
            "https://open-vsx.org@evil/x",
            "https://open-vsx.org:444/x",
        ] {
            assert!(!allowed_url(bad), "{bad}");
        }
        let fake = |_: &str, _: u64| -> Result<Vec<u8>, ExtError> { Ok(br#"{"extensions":[]}"#.to_vec()) };
        assert_eq!(search(&fake, "", 0), Err(ExtError::BadInput));
        assert_eq!(search(&fake, "a\u{7}", 0), Err(ExtError::BadInput));
        assert_eq!(search(&fake, "dracula", 0), Ok(vec![]));
    }

    #[test]
    fn search_parses_and_drops_bad_rows() {
        let body = br#"{"extensions":[
            {"namespace":"dracula-theme","name":"theme-dracula","version":"2.25.1","displayName":"Dracula","description":"Dark\u0007 theme","downloadCount":5},
            {"namespace":"../x","name":"y","version":"1"}]}"#;
        let seen = Mutex::new(String::new());
        let fake = |u: &str, _: u64| -> Result<Vec<u8>, ExtError> {
            *seen.lock().unwrap() = u.to_string();
            Ok(body.to_vec())
        };
        let out = search(&fake, "dark theme&x=1", 20).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].description, "Dark theme");
        assert!(seen.lock().unwrap().ends_with("query=dark%20theme%26x%3D1&size=20&offset=20"));
    }
}
