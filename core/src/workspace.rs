//! workspace.rs -- one open workspace folder and every file operation the IDE makes inside it.
//!
//! Purpose: the editor shell (phase I1, docs/features/editor-shell.md) opens a folder on the user's computer and lists,
//! reads, saves and searches files in it. Later the local agent tools (phase I3) use these same functions, so the agent
//! is held to exactly the same limits as the person.
//!
//! Inputs and trust boundaries:
//!   * the folder path the person picks (trusted intent, but checked: absolute, an existing folder, not a drive root);
//!   * every relative path and every file body comes from the webview, which is treated as untrusted (it renders
//!     file contents, and later model output), so each one is checked here before any file system call.
//!
//! Security controls:
//!   * Path confinement: relative paths are an allow-list of plain components; no absolute paths, `..`, drive or UNC
//!     prefixes, backslashes, colons (Windows alternate data streams) or NUL (CWE-22, CWE-73, ASVS V12.3.1).
//!   * Handle-based access (cap-std): the workspace holds an open handle to its root folder, and every list, read and
//!     write resolves the relative path beneath that handle (openat2 RESOLVE_BENEATH on Linux, component-by-component
//!     resolution elsewhere). A symlink or junction that leads outside is refused at the moment of opening, not at an
//!     earlier check, so another program swapping a folder for a link mid-operation cannot redirect it (CWE-59,
//!     CWE-367 TOCTOU). Writes resolve the parent folder once to a handle and do the version check, temp file and
//!     rename all inside that handle.
//!   * Size caps on reads, writes, listings and search results (CWE-400, CWE-770).
//!   * Saves are atomic (temp file created with create_new in the same folder, flushed, then renamed) and check a
//!     SHA-256 version, so a save never silently overwrites a change made outside the IDE (CWE-367 lost update) and
//!     never leaves a half-written file.
//!   * Errors are short fixed messages without full paths or OS details (CWE-209).
//!   * The root handle follows the folder: if the folder is renamed while open, the IDE keeps working on it at its new
//!     place rather than on whatever now has the old name.
//!
//! Dependencies: std, serde (records), sha2 (file versions; SHA-256 is an approved hash, NIST FIPS 180-4), cap-std
//! (Bytecode Alliance; capability-based file system access, used by Wasmtime) for the handle-based confinement.

use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Largest file the editor opens or saves (5 MiB). Bigger files are refused rather than freezing the UI.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Largest number of entries returned for one folder.
pub const MAX_DIR_ENTRIES: usize = 5000;
/// Longest relative path accepted, in bytes.
pub const MAX_REL_PATH_BYTES: usize = 1024;
/// Search: most matches returned, longest query, largest file searched, most files visited.
pub const MAX_SEARCH_RESULTS: usize = 200;
pub const MAX_QUERY_BYTES: usize = 256;
pub const MAX_SEARCH_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_SEARCH_FILES: usize = 20_000;
/// Folders that search skips because they are large build or dependency output.
pub const SEARCH_SKIP_DIRS: [&str; 6] = [".git", "node_modules", "target", "dist", ".venv", "__pycache__"];

/// The errors a workspace call can return. Each maps to one short message for the person (no paths, no OS text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsError {
    /// The folder to open is not absolute, does not exist, is not a folder, or is a whole drive.
    BadRoot,
    /// A relative path broke the allow-list rules.
    BadPath,
    /// The path leads outside the workspace (for example through a symlink).
    OutsideWorkspace,
    /// Nothing at that path.
    NotFound,
    /// The path is a folder where a file was expected, or the reverse.
    WrongKind,
    /// The file is larger than the cap.
    TooLarge,
    /// The file is not UTF-8 text (binary files are not opened in the editor).
    NotText,
    /// The file changed on disk since it was opened; the save was refused.
    Conflict,
    /// The search query is empty or too long.
    BadQuery,
    /// Any other I/O failure.
    Io,
}

impl WsError {
    /// A short, fixed message that is safe to show in the UI.
    ///
    /// Security: never includes a path or the OS error text (CWE-209).
    pub fn message(&self) -> &'static str {
        match self {
            WsError::BadRoot => "choose an existing folder (not a whole drive)",
            WsError::BadPath => "that file name is not allowed",
            WsError::OutsideWorkspace => "that path leads outside the workspace folder",
            WsError::NotFound => "file or folder not found",
            WsError::WrongKind => "that is a folder, not a file (or the reverse)",
            WsError::TooLarge => "the file is larger than 5 MiB, so it is not opened in the editor",
            WsError::NotText => "the file is not UTF-8 text, so it is not opened in the editor",
            WsError::Conflict => "the file changed on disk since you opened it; reload it before saving",
            WsError::BadQuery => "enter 1 to 256 characters to search",
            WsError::Io => "the file system refused the operation",
        }
    }
}

/// Maps an I/O error to a workspace error without keeping its text.
///
/// cap-std reports a path that would leave the root handle (through `..` in a link target, an absolute link or a
/// junction) as PermissionDenied "a path led outside of the filesystem", or EXDEV from openat2 RESOLVE_BENEATH on
/// Linux; both become `OutsideWorkspace`.
fn io_err(e: std::io::Error) -> WsError {
    match e.kind() {
        std::io::ErrorKind::NotFound => WsError::NotFound,
        std::io::ErrorKind::PermissionDenied if e.to_string().contains("outside of the filesystem") => {
            WsError::OutsideWorkspace
        }
        std::io::ErrorKind::CrossesDevices => WsError::OutsideWorkspace,
        _ if e.raw_os_error() == Some(18) && cfg!(unix) => WsError::OutsideWorkspace, // EXDEV on Linux and macOS
        _ => WsError::Io,
    }
}

/// What kind of entry a folder listing shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Dir,
    /// A symlink is shown but never followed by the listing (opening it still runs the confinement check).
    Symlink,
}

/// One entry of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// The entry's own name.
    pub name: String,
    /// Its path relative to the workspace root, with `/` separators.
    pub rel: String,
    pub kind: EntryKind,
    /// Size in bytes for files, 0 otherwise.
    pub size: u64,
}

/// A folder listing, with a flag when it was cut at MAX_DIR_ENTRIES.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub truncated: bool,
}

/// A file read for the editor: its text and its version (SHA-256 of the bytes, lowercase hex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileText {
    pub text: String,
    pub version: String,
}

/// One search match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Match {
    pub rel: String,
    /// Line number, from 1.
    pub line: usize,
    /// The matching line, cut to 200 characters.
    pub preview: String,
}

/// Search results, with a flag when a cap was reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    pub matches: Vec<Match>,
    pub truncated: bool,
}

/// An open workspace: the canonical root folder and an open handle to it.
///
/// Security: every file operation goes through `dir` (cap-std), never through `root`, so a path can only resolve
/// beneath the folder the person opened (CWE-22, CWE-59, CWE-367). `root` is kept for display and network scoping only.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    dir: Arc<Dir>,
}

/// Counter that makes temp file names unique within this process.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// SHA-256 of bytes as lowercase hex: the file "version" used to detect outside changes.
pub fn version_of(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

impl Workspace {
    /// Opens a workspace folder.
    ///
    /// * `path` -- absolute path of an existing folder, chosen by the person.
    ///
    /// Returns the workspace with its canonical root, or `BadRoot`.
    ///
    /// Security: refuses relative paths and filesystem or drive roots (a whole drive is far too broad a scope for the
    /// agent later, NIST AC-6 least privilege), and canonicalises so every later check compares real paths (CWE-59).
    pub fn open(path: &Path) -> Result<Workspace, WsError> {
        if !path.is_absolute() {
            return Err(WsError::BadRoot);
        }
        let root = fs::canonicalize(path).map_err(|_| WsError::BadRoot)?;
        if !root.is_dir() {
            return Err(WsError::BadRoot);
        }
        // SECURITY: a root with no parent is "/" or a drive root such as C:\ -- refuse it (least privilege).
        if root.parent().is_none() {
            return Err(WsError::BadRoot);
        }
        // SECURITY: the one use of ambient authority -- the folder the person chose in the OS dialog. Everything else
        // is relative to this handle (cap-std capability model).
        let dir = Dir::open_ambient_dir(&root, ambient_authority()).map_err(|_| WsError::BadRoot)?;
        Ok(Workspace { root, dir: Arc::new(dir) })
    }

    /// The workspace folder's own name, for the title bar (never the full path, which may hold a user name).
    pub fn name(&self) -> String {
        self.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "workspace".to_string())
    }

    /// The canonical root (for the shell only; never sent to the webview).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Checks a relative path against the same allow-list as every file call, without touching the disk (phase I5:
    /// paths reported by git are checked before the IDE acts on them).
    ///
    /// Security: see `join_checked` (CWE-22, CWE-73).
    pub fn check_rel(&self, rel: &str) -> Result<(), WsError> {
        if rel.is_empty() {
            return Err(WsError::BadPath);
        }
        self.join_checked(rel).map(|_| ())
    }

    /// Checks a relative path against the allow-list and returns it as a relative path for the root handle (`.` for the
    /// root itself). Does not touch the disk.
    ///
    /// * `rel` -- `/`-separated path relative to the root; `""` means the root itself.
    ///
    /// Security: allow-list of plain components only (CWE-22, CWE-73): no absolute paths, no `..` or `.`, no drive or
    /// UNC prefixes, no backslashes (one separator only, so Windows and Unix agree), no `:` (alternate data streams),
    /// no NUL or other control characters, length capped.
    fn join_checked(&self, rel: &str) -> Result<PathBuf, WsError> {
        if rel.len() > MAX_REL_PATH_BYTES {
            return Err(WsError::BadPath);
        }
        if rel.is_empty() {
            return Ok(PathBuf::from("."));
        }
        // SECURITY: refuse characters that change meaning on some OS before looking at components.
        if rel.chars().any(|c| c == '\\' || c == ':' || c.is_control()) {
            return Err(WsError::BadPath);
        }
        let mut out = PathBuf::new();
        for part in rel.split('/') {
            // SECURITY: empty parts (`a//b`, leading or trailing `/`), `.` and `..` are all refused.
            if part.is_empty() || part == "." || part == ".." {
                return Err(WsError::BadPath);
            }
            // SECURITY: Path::components must see exactly one Normal component, so nothing like a prefix slips in.
            let mut comps = Path::new(part).components();
            match (comps.next(), comps.next()) {
                (Some(Component::Normal(_)), None) => out.push(part),
                _ => return Err(WsError::BadPath),
            }
        }
        Ok(out)
    }

    /// Opens an existing folder of the workspace as its own handle (`""` gives a second handle to the root).
    ///
    /// Security: resolved beneath the root handle; a link that leads outside fails here (CWE-59). Later operations use
    /// the returned handle, so the folder cannot be swapped underneath them (CWE-367).
    fn open_dir(&self, rel: &str) -> Result<Dir, WsError> {
        let path = self.join_checked(rel)?;
        if rel.is_empty() {
            return self.dir.try_clone().map_err(io_err);
        }
        // A file where a folder is expected is WrongKind, not a generic I/O error.
        match self.dir.metadata(&path) {
            Ok(m) if !m.is_dir() => return Err(WsError::WrongKind),
            Ok(_) => {}
            Err(e) => return Err(io_err(e)),
        }
        self.dir.open_dir(&path).map_err(io_err)
    }

    /// Lists one folder, folders first, then files, each sorted by name (case-insensitive).
    ///
    /// * `rel` -- folder path relative to the root (`""` for the root).
    ///
    /// Security: symlinks are reported as `Symlink` and not followed here; the listing is capped (CWE-400).
    pub fn list_dir(&self, rel: &str) -> Result<Listing, WsError> {
        let dir = self.open_dir(rel)?;
        let mut entries = Vec::new();
        let mut truncated = false;
        for item in dir.entries().map_err(io_err)? {
            let item = item.map_err(io_err)?;
            if entries.len() >= MAX_DIR_ENTRIES {
                truncated = true;
                break;
            }
            let name = match item.file_name().into_string() {
                Ok(n) => n,
                // A name that is not valid Unicode cannot be addressed through the `/` path API; skip it.
                Err(_) => continue,
            };
            // Names the path rules would refuse (for example with `:` or `\` on Unix) are skipped, so every listed
            // entry can be opened.
            if name.chars().any(|c| c == '\\' || c == ':' || c.is_control()) {
                continue;
            }
            // DirEntry::metadata does not follow links, so the listing never reaches outside the folder.
            let meta = match item.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let kind = if meta.file_type().is_symlink() {
                EntryKind::Symlink
            } else if meta.is_dir() {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            let rel_child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            entries.push(Entry {
                size: if kind == EntryKind::File { meta.len() } else { 0 },
                name,
                rel: rel_child,
                kind,
            });
        }
        entries.sort_by(|a, b| {
            let rank = |k: EntryKind| if k == EntryKind::Dir { 0 } else { 1 };
            rank(a.kind)
                .cmp(&rank(b.kind))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(Listing { entries, truncated })
    }

    /// Reads a text file for the editor.
    ///
    /// * `rel` -- file path relative to the root.
    ///
    /// Returns its text and version. Errors: `TooLarge` over 5 MiB, `NotText` when not UTF-8, `WrongKind` for folders.
    ///
    /// Security: confinement as above; reads at most MAX_FILE_BYTES + 1 bytes even if the file grows while being read
    /// (CWE-400).
    pub fn read_text(&self, rel: &str) -> Result<FileText, WsError> {
        let path = self.join_checked(rel)?;
        if rel.is_empty() {
            return Err(WsError::WrongKind);
        }
        read_text_in(&self.dir, &path)
    }
}

/// Reads a text file through a folder handle (the workspace root or a parent folder already opened).
///
/// Security: the file is opened beneath `dir` (CWE-59), and its type and size are taken from the open handle, so they
/// describe the file actually read (CWE-367); at most MAX_FILE_BYTES + 1 bytes are read (CWE-400).
fn read_text_in(dir: &Dir, path: &Path) -> Result<FileText, WsError> {
    let (bytes, version) = read_bytes_in(dir, path, MAX_FILE_BYTES)?;
    let text = String::from_utf8(bytes).map_err(|_| WsError::NotText)?;
    Ok(FileText { text, version })
}

/// Reads at most `max` bytes of a file through a folder handle. Returns `(bytes, version)`.
///
/// Errors: `WrongKind` for a folder or other non-file, `TooLarge` past `max`, the mapped I/O error otherwise.
///
/// Security: opened beneath `dir`, so the path cannot leave the workspace even through a link (CWE-22, CWE-59); type
/// and size come from the open handle, so they describe the file actually read (CWE-367); the read is bounded by
/// `take()` even if the file grows after the size check (CWE-400).
fn read_bytes_in(dir: &Dir, path: &Path, max: u64) -> Result<(Vec<u8>, String), WsError> {
    // A folder cannot be opened for reading on every OS; ask first so the answer is WrongKind everywhere.
    match dir.metadata(path) {
        Ok(m) if m.is_dir() => return Err(WsError::WrongKind),
        Ok(_) => {}
        Err(e) => return Err(io_err(e)),
    }
    let file = dir.open(path).map_err(io_err)?;
    let meta = file.metadata().map_err(io_err)?;
    if !meta.is_file() {
        return Err(WsError::WrongKind);
    }
    if meta.len() > max {
        return Err(WsError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    // SECURITY: take() bounds the read even if the file grew after the size check (CWE-400).
    file.take(max.saturating_add(1)).read_to_end(&mut bytes).map_err(io_err)?;
    if bytes.len() as u64 > max {
        return Err(WsError::TooLarge);
    }
    let version = version_of(&bytes);
    Ok((bytes, version))
}

impl Workspace {
    /// Reads a file's raw bytes (phase I4: sending the folder to a gateway project), at most `max` bytes.
    ///
    /// Returns `(bytes, version)`; `TooLarge` past `max`, `WrongKind` for a folder.
    ///
    /// Security: same confinement as [`Workspace::read_text`]: opened through the workspace folder handle, so it
    /// cannot leave the folder even through a link (CWE-22, CWE-59, CWE-367); the read is bounded (CWE-400).
    pub fn read_bytes(&self, rel: &str, max: u64) -> Result<(Vec<u8>, String), WsError> {
        let path = self.join_checked(rel)?;
        if rel.is_empty() {
            return Err(WsError::WrongKind);
        }
        read_bytes_in(&self.dir, &path, max)
    }

    /// Lists every file under the root (path and size), skipping build and dependency folders and symlinks, as
    /// search does. Stops after `max_files` and reports `true` when it did.
    ///
    /// Security: symlinks are never followed (CWE-59); the walk is iterative and capped (CWE-400).
    pub fn all_files(&self, max_files: usize) -> (Vec<(String, u64)>, bool) {
        let mut out = Vec::new();
        let mut stack: Vec<String> = vec![String::new()];
        while let Some(dir_rel) = stack.pop() {
            let Ok(listing) = self.list_dir(&dir_rel) else { continue };
            for e in listing.entries {
                match e.kind {
                    EntryKind::Dir if !SEARCH_SKIP_DIRS.contains(&e.name.as_str()) => stack.push(e.rel),
                    EntryKind::File => {
                        if out.len() >= max_files {
                            return (out, true);
                        }
                        out.push((e.rel, e.size));
                    }
                    _ => {}
                }
            }
        }
        out.sort();
        (out, false)
    }

    /// Saves a text file atomically.
    ///
    /// * `rel` -- file path relative to the root. Its folder must already exist inside the workspace.
    /// * `text` -- the new contents (at most 5 MiB as UTF-8).
    /// * `base_version` -- the version the editor loaded, or `None` to create a new file (refused if it exists).
    ///
    /// Returns the new version. Errors: `Conflict` when the file on disk no longer matches `base_version`.
    ///
    /// Security: the parent folder is opened once as a handle beneath the root (CWE-59), and the symlink check, version
    /// check, temp file and rename all happen inside that handle, so the folder cannot be swapped mid-save (CWE-367);
    /// the target must not be a symlink (a link could redirect the write, CWE-59); the temp file is created with
    /// create_new (no clobbering, no cross-device rename) and renamed over the target only after a full write and flush.
    pub fn write_text(&self, rel: &str, text: &str, base_version: Option<&str>) -> Result<String, WsError> {
        if text.len() as u64 > MAX_FILE_BYTES {
            return Err(WsError::TooLarge);
        }
        let joined = self.join_checked(rel)?;
        if rel.is_empty() {
            return Err(WsError::WrongKind);
        }
        let file_name = PathBuf::from(joined.file_name().ok_or(WsError::BadPath)?);
        let parent_rel = match rel.rfind('/') {
            Some(i) => &rel[..i],
            None => "",
        };
        // SECURITY: the folder the file goes into is opened once, beneath the root; everything below uses this handle.
        let parent = self.open_dir(parent_rel)?;
        match parent.symlink_metadata(&file_name) {
            Ok(meta) => {
                // SECURITY: never write through a symlink (CWE-59).
                if meta.file_type().is_symlink() {
                    return Err(WsError::OutsideWorkspace);
                }
                if !meta.is_file() {
                    return Err(WsError::WrongKind);
                }
                // SECURITY: lost-update check -- the file must still be what the editor loaded (CWE-367).
                let current = read_text_in(&parent, &file_name)?;
                match base_version {
                    Some(v) if v == current.version => {}
                    _ => return Err(WsError::Conflict),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // A new file: only allowed when the caller says it is creating one.
                if base_version.is_some() {
                    return Err(WsError::Conflict);
                }
            }
            Err(e) => return Err(io_err(e)),
        }

        let tmp = temp_name(&file_name.to_string_lossy());
        let result = (|| -> Result<(), WsError> {
            // SECURITY: create_new fails if the name exists (also if it is a symlink), so the temp file is ours.
            let mut f = parent.open_with(&tmp, OpenOptions::new().write(true).create_new(true)).map_err(io_err)?;
            f.write_all(text.as_bytes()).map_err(io_err)?;
            f.sync_all().map_err(io_err)?;
            drop(f);
            parent.rename(&tmp, &parent, &file_name).map_err(io_err)?;
            Ok(())
        })();
        if result.is_err() {
            // Best effort: never leave a stray temp file behind.
            let _ = parent.remove_file(&tmp);
        }
        result?;
        Ok(version_of(text.as_bytes()))
    }

    /// Searches text files for a plain substring (not a regular expression, so no ReDoS, CWE-1333).
    ///
    /// * `query` -- 1 to 256 bytes; matched case-insensitively.
    ///
    /// Skips build and dependency folders, symlinks (never followed, CWE-59), files over 1 MiB and files that are not
    /// UTF-8. Stops at MAX_SEARCH_RESULTS matches or MAX_SEARCH_FILES files (CWE-400).
    pub fn search(&self, query: &str) -> Result<SearchResult, WsError> {
        if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.chars().any(|c| c.is_control()) {
            return Err(WsError::BadQuery);
        }
        let needle = query.to_lowercase();
        let mut matches = Vec::new();
        let mut truncated = false;
        let mut files_seen = 0usize;
        // Iterative walk with an explicit stack (no recursion depth limit to hit).
        let mut stack: Vec<String> = vec![String::new()];
        'walk: while let Some(dir_rel) = stack.pop() {
            let listing = match self.list_dir(&dir_rel) {
                Ok(l) => l,
                Err(_) => continue,
            };
            for e in listing.entries {
                match e.kind {
                    EntryKind::Dir => {
                        if !SEARCH_SKIP_DIRS.contains(&e.name.as_str()) {
                            stack.push(e.rel);
                        }
                    }
                    EntryKind::Symlink => {}
                    EntryKind::File => {
                        files_seen += 1;
                        if files_seen > MAX_SEARCH_FILES {
                            truncated = true;
                            break 'walk;
                        }
                        if e.size > MAX_SEARCH_FILE_BYTES {
                            continue;
                        }
                        let Ok(ft) = self.read_text(&e.rel) else { continue };
                        for (i, line) in ft.text.lines().enumerate() {
                            if line.to_lowercase().contains(&needle) {
                                if matches.len() >= MAX_SEARCH_RESULTS {
                                    truncated = true;
                                    break 'walk;
                                }
                                matches.push(Match {
                                    rel: e.rel.clone(),
                                    line: i + 1,
                                    preview: line.trim().chars().take(200).collect(),
                                });
                            }
                        }
                    }
                }
            }
        }
        matches.sort_by(|a, b| a.rel.cmp(&b.rel).then(a.line.cmp(&b.line)));
        Ok(SearchResult { matches, truncated })
    }
}

/// Builds a temp file name for the target's folder: `.<name>.p2pide-<pid>-<time>-<n>.tmp`.
///
/// The name only needs to be unique (create_new refuses a clash); it is not a secret, so no CSPRNG is needed.
fn temp_name(name: &str) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let short: String = name.chars().take(64).collect();
    PathBuf::from(format!(".{short}.p2pide-{}-{t}-{n}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    //! Security unit tests with malicious inputs (CLAUDE.md section 6 step 6).
    use super::*;

    /// A fresh temp folder per test, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> TempDir {
            let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("p2pide-test-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TempDir(fs::canonicalize(&p).unwrap())
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn setup(tag: &str) -> (TempDir, Workspace) {
        let t = TempDir::new(tag);
        fs::create_dir_all(t.0.join("ws/src")).unwrap();
        fs::write(t.0.join("ws/src/main.py"), "print('hi')\nTIMEOUT = 5\n").unwrap();
        fs::write(t.0.join("ws/README.md"), "# Demo\n").unwrap();
        fs::write(t.0.join("secret.txt"), "outside").unwrap();
        let ws = Workspace::open(&t.0.join("ws")).unwrap();
        (t, ws)
    }

    #[test]
    fn open_refuses_relative_missing_file_and_root() {
        assert_eq!(Workspace::open(Path::new("relative/dir")).unwrap_err(), WsError::BadRoot);
        let (t, _) = setup("open");
        assert_eq!(Workspace::open(&t.0.join("nope")).unwrap_err(), WsError::BadRoot);
        assert_eq!(Workspace::open(&t.0.join("secret.txt")).unwrap_err(), WsError::BadRoot);
        #[cfg(unix)]
        assert_eq!(Workspace::open(Path::new("/")).unwrap_err(), WsError::BadRoot);
    }

    #[test]
    fn name_is_folder_name_only() {
        let (_t, ws) = setup("name");
        assert_eq!(ws.name(), "ws");
    }

    #[test]
    fn traversal_and_odd_paths_are_refused() {
        let (_t, ws) = setup("trav");
        for bad in [
            "../secret.txt",
            "src/../../secret.txt",
            "..",
            ".",
            "./README.md",
            "/etc/passwd",
            "src//main.py",
            "src/",
            "/",
            "src\\main.py",
            "C:/Windows",
            "README.md:stream",
            "a\0b",
            "a\nb",
        ] {
            assert_eq!(ws.read_text(bad).unwrap_err(), WsError::BadPath, "{bad:?}");
        }
        let long = "a/".repeat(600) + "b";
        assert_eq!(ws.read_text(&long).unwrap_err(), WsError::BadPath);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_out_of_workspace_is_refused() {
        let (t, ws) = setup("link");
        std::os::unix::fs::symlink(t.0.join("secret.txt"), t.0.join("ws/leak.txt")).unwrap();
        std::os::unix::fs::symlink(&t.0, t.0.join("ws/up")).unwrap();
        assert_eq!(ws.read_text("leak.txt").unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(ws.read_text("up/secret.txt").unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(ws.list_dir("up").unwrap_err(), WsError::OutsideWorkspace);
        // Writing through a link (out or in) is refused, and the outside file is untouched.
        assert_eq!(ws.write_text("leak.txt", "x", Some("0")).unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(ws.write_text("up/secret.txt", "x", None).unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(fs::read_to_string(t.0.join("secret.txt")).unwrap(), "outside");
        // The listing shows links as links.
        let l = ws.list_dir("").unwrap();
        assert!(l.entries.iter().any(|e| e.name == "leak.txt" && e.kind == EntryKind::Symlink));
    }

    #[cfg(unix)]
    #[test]
    fn folder_swapped_for_a_link_after_open_is_still_refused() {
        // The race the handle-based access closes: a folder that was real becomes a link to outside. Every operation
        // resolves at open time beneath the root handle, so nothing reaches the outside file (CWE-367, CWE-59).
        let (t, ws) = setup("swap");
        assert!(ws.read_text("src/main.py").is_ok());
        fs::create_dir(t.0.join("outside")).unwrap();
        fs::write(t.0.join("outside/main.py"), "outside").unwrap();
        fs::rename(t.0.join("ws/src"), t.0.join("ws/src-old")).unwrap();
        std::os::unix::fs::symlink(t.0.join("outside"), t.0.join("ws/src")).unwrap();
        assert_eq!(ws.read_text("src/main.py").unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(ws.list_dir("src").unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(ws.write_text("src/main.py", "x", None).unwrap_err(), WsError::OutsideWorkspace);
        assert_eq!(fs::read_to_string(t.0.join("outside/main.py")).unwrap(), "outside");
        // A link that stays inside the workspace still works, as before.
        std::os::unix::fs::symlink("src-old/main.py", t.0.join("ws/inside.py")).unwrap();
        assert_eq!(ws.read_text("inside.py").unwrap().text, "print('hi')\nTIMEOUT = 5\n");
    }

    #[cfg(unix)]
    #[test]
    fn workspace_follows_its_folder_when_it_is_renamed() {
        // The root handle stays on the folder the person opened; a link put at the old name is never followed.
        let (t, ws) = setup("moved");
        fs::rename(t.0.join("ws"), t.0.join("ws-moved")).unwrap();
        fs::create_dir(t.0.join("decoy")).unwrap();
        fs::write(t.0.join("decoy/README.md"), "decoy").unwrap();
        std::os::unix::fs::symlink(t.0.join("decoy"), t.0.join("ws")).unwrap();
        assert_eq!(ws.read_text("README.md").unwrap().text, "# Demo\n");
        let v = ws.read_text("README.md").unwrap().version;
        ws.write_text("README.md", "# Moved\n", Some(&v)).unwrap();
        assert_eq!(fs::read_to_string(t.0.join("ws-moved/README.md")).unwrap(), "# Moved\n");
        assert_eq!(fs::read_to_string(t.0.join("decoy/README.md")).unwrap(), "decoy");
    }

    #[test]
    fn listing_is_sorted_folders_first() {
        let (t, ws) = setup("list");
        fs::write(t.0.join("ws/a.txt"), "a").unwrap();
        fs::create_dir(t.0.join("ws/zdir")).unwrap();
        let names: Vec<_> = ws.list_dir("").unwrap().entries.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["src", "zdir", "a.txt", "README.md"]);
        let src = ws.list_dir("src").unwrap();
        assert_eq!(src.entries[0].rel, "src/main.py");
        assert_eq!(src.entries[0].size, 24);
        assert_eq!(ws.list_dir("README.md").unwrap_err(), WsError::WrongKind);
    }

    #[test]
    fn read_and_save_with_version_check() {
        let (t, ws) = setup("save");
        let f = ws.read_text("src/main.py").unwrap();
        assert_eq!(f.version, version_of(b"print('hi')\nTIMEOUT = 5\n"));
        let v2 = ws.write_text("src/main.py", "print('bye')\n", Some(&f.version)).unwrap();
        assert_eq!(fs::read_to_string(t.0.join("ws/src/main.py")).unwrap(), "print('bye')\n");
        // A stale version is refused and the file keeps the newer contents.
        assert_eq!(ws.write_text("src/main.py", "old", Some(&f.version)).unwrap_err(), WsError::Conflict);
        // An outside change makes the editor's version stale too.
        fs::write(t.0.join("ws/src/main.py"), "changed elsewhere").unwrap();
        assert_eq!(ws.write_text("src/main.py", "mine", Some(&v2)).unwrap_err(), WsError::Conflict);
        // Saving without a version over an existing file is refused.
        assert_eq!(ws.write_text("src/main.py", "mine", None).unwrap_err(), WsError::Conflict);
        // No temp files are left behind.
        assert!(fs::read_dir(t.0.join("ws/src")).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn create_new_file_only_in_existing_folder() {
        let (t, ws) = setup("create");
        let v = ws.write_text("src/new.py", "x = 1\n", None).unwrap();
        assert_eq!(v, version_of(b"x = 1\n"));
        assert_eq!(fs::read_to_string(t.0.join("ws/src/new.py")).unwrap(), "x = 1\n");
        assert_eq!(ws.write_text("nofolder/new.py", "x", None).unwrap_err(), WsError::NotFound);
        assert_eq!(ws.write_text("", "x", None).unwrap_err(), WsError::WrongKind);
        assert_eq!(ws.write_text("src", "x", Some("v")).unwrap_err(), WsError::WrongKind);
        assert_eq!(ws.write_text("../escape.txt", "x", None).unwrap_err(), WsError::BadPath);
        assert!(!t.0.join("escape.txt").exists());
    }

    #[test]
    fn size_and_text_limits() {
        let (t, ws) = setup("size");
        let big = vec![b'a'; (MAX_FILE_BYTES + 1) as usize];
        fs::write(t.0.join("ws/big.txt"), &big).unwrap();
        assert_eq!(ws.read_text("big.txt").unwrap_err(), WsError::TooLarge);
        let big_text = "a".repeat((MAX_FILE_BYTES + 1) as usize);
        assert_eq!(ws.write_text("big2.txt", &big_text, None).unwrap_err(), WsError::TooLarge);
        fs::write(t.0.join("ws/bin.dat"), [0xff, 0xfe, 0x00, 0x01]).unwrap();
        assert_eq!(ws.read_text("bin.dat").unwrap_err(), WsError::NotText);
        assert_eq!(ws.read_text("src").unwrap_err(), WsError::WrongKind);
        assert_eq!(ws.read_text("missing.txt").unwrap_err(), WsError::NotFound);
    }

    #[test]
    fn search_finds_plain_text_and_skips_build_folders() {
        let (t, ws) = setup("search");
        fs::create_dir_all(t.0.join("ws/node_modules/x")).unwrap();
        fs::write(t.0.join("ws/node_modules/x/i.js"), "TIMEOUT").unwrap();
        let r = ws.search("timeout").unwrap();
        assert_eq!(r.matches.len(), 1);
        assert_eq!(r.matches[0].rel, "src/main.py");
        assert_eq!(r.matches[0].line, 2);
        // Regex metacharacters are plain text (no ReDoS possible).
        assert!(ws.search("(a+)+$").unwrap().matches.is_empty());
        assert_eq!(ws.search("").unwrap_err(), WsError::BadQuery);
        assert_eq!(ws.search(&"q".repeat(257)).unwrap_err(), WsError::BadQuery);
    }

    #[test]
    fn error_messages_never_contain_paths() {
        for e in [
            WsError::BadRoot,
            WsError::BadPath,
            WsError::OutsideWorkspace,
            WsError::NotFound,
            WsError::WrongKind,
            WsError::TooLarge,
            WsError::NotText,
            WsError::Conflict,
            WsError::BadQuery,
            WsError::Io,
        ] {
            assert!(!e.message().contains('/'), "{e:?}");
            assert!(!e.message().contains('\\'), "{e:?}");
        }
    }
}
