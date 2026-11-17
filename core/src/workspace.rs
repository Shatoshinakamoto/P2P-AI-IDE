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
//!     prefixes, backslashes, colons (Windows alternate data streams) or NUL; the final path is canonicalised and must
//!     stay under the canonical root, so a symlink cannot lead outside (CWE-22, CWE-59, CWE-73, ASVS V12.3.1).
//!   * Size caps on reads, writes, listings and search results (CWE-400, CWE-770).
//!   * Saves are atomic (temp file created with create_new in the same folder, flushed, then renamed) and check a
//!     SHA-256 version, so a save never silently overwrites a change made outside the IDE (CWE-367 lost update) and
//!     never leaves a half-written file.
//!   * Errors are short fixed messages without full paths or OS details (CWE-209).
//!   * Known limit: between the canonical check and the open there is a small TOCTOU window if another local program
//!     swaps a folder for a symlink. The re-check after open (read) and the same-folder temp file (write) narrow it;
//!     TODO(security): use openat-style handles (cap-std) when the agent tools land in phase I3.
//!
//! Dependencies: std, serde (records), sha2 (file versions; SHA-256 is an approved hash, NIST FIPS 180-4).

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
const SEARCH_SKIP_DIRS: [&str; 6] = [".git", "node_modules", "target", "dist", ".venv", "__pycache__"];

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
fn io_err(e: std::io::Error) -> WsError {
    match e.kind() {
        std::io::ErrorKind::NotFound => WsError::NotFound,
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

/// An open workspace: the canonical root folder.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
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
        Ok(Workspace { root })
    }

    /// The workspace folder's own name, for the title bar (never the full path, which may hold a user name).
    pub fn name(&self) -> String {
        self.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "workspace".to_string())
    }

    /// The canonical root (for the shell only; never sent to the webview).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Checks a relative path against the allow-list and joins it to the root. Does not touch the disk.
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
            return Ok(self.root.clone());
        }
        // SECURITY: refuse characters that change meaning on some OS before looking at components.
        if rel.chars().any(|c| c == '\\' || c == ':' || c.is_control()) {
            return Err(WsError::BadPath);
        }
        let mut out = self.root.clone();
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

    /// Resolves an existing path and proves it is inside the workspace.
    ///
    /// Security: canonicalises (following symlinks) and requires the result to start with the canonical root, so a
    /// symlink or junction cannot point outside (CWE-59).
    fn resolve_existing(&self, rel: &str) -> Result<PathBuf, WsError> {
        let joined = self.join_checked(rel)?;
        let real = fs::canonicalize(&joined).map_err(io_err)?;
        if !real.starts_with(&self.root) {
            return Err(WsError::OutsideWorkspace);
        }
        Ok(real)
    }

    /// Lists one folder, folders first, then files, each sorted by name (case-insensitive).
    ///
    /// * `rel` -- folder path relative to the root (`""` for the root).
    ///
    /// Security: symlinks are reported as `Symlink` and not followed here; the listing is capped (CWE-400).
    pub fn list_dir(&self, rel: &str) -> Result<Listing, WsError> {
        let dir = self.resolve_existing(rel)?;
        if !dir.is_dir() {
            return Err(WsError::WrongKind);
        }
        let mut entries = Vec::new();
        let mut truncated = false;
        for item in fs::read_dir(&dir).map_err(io_err)? {
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
            // symlink_metadata does not follow links, so the listing never reaches outside the folder.
            let meta = match fs::symlink_metadata(item.path()) {
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
        let path = self.resolve_existing(rel)?;
        let meta = fs::metadata(&path).map_err(io_err)?;
        if !meta.is_file() {
            return Err(WsError::WrongKind);
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(WsError::TooLarge);
        }
        let file = File::open(&path).map_err(io_err)?;
        let mut bytes = Vec::with_capacity(meta.len() as usize);
        // SECURITY: take() bounds the read even if the file grew after the size check.
        file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(io_err)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(WsError::TooLarge);
        }
        let version = version_of(&bytes);
        let text = String::from_utf8(bytes).map_err(|_| WsError::NotText)?;
        Ok(FileText { text, version })
    }

    /// Saves a text file atomically.
    ///
    /// * `rel` -- file path relative to the root. Its folder must already exist inside the workspace.
    /// * `text` -- the new contents (at most 5 MiB as UTF-8).
    /// * `base_version` -- the version the editor loaded, or `None` to create a new file (refused if it exists).
    ///
    /// Returns the new version. Errors: `Conflict` when the file on disk no longer matches `base_version`.
    ///
    /// Security: confinement of the parent folder (canonical, inside the root) and of the target if it exists; the
    /// target must not be a symlink (a link could redirect the write, CWE-59); the temp file is created with
    /// create_new in the same folder (no clobbering, no cross-device rename) and renamed over the target only after a
    /// full write and flush.
    pub fn write_text(&self, rel: &str, text: &str, base_version: Option<&str>) -> Result<String, WsError> {
        if text.len() as u64 > MAX_FILE_BYTES {
            return Err(WsError::TooLarge);
        }
        let joined = self.join_checked(rel)?;
        if rel.is_empty() {
            return Err(WsError::WrongKind);
        }
        let file_name = joined.file_name().ok_or(WsError::BadPath)?.to_os_string();
        let parent_rel = match rel.rfind('/') {
            Some(i) => &rel[..i],
            None => "",
        };
        // SECURITY: the folder the file goes into must be real and inside the workspace.
        let parent = self.resolve_existing(parent_rel)?;
        if !parent.is_dir() {
            return Err(WsError::WrongKind);
        }
        let target = parent.join(&file_name);
        match fs::symlink_metadata(&target) {
            Ok(meta) => {
                // SECURITY: never write through a symlink (CWE-59).
                if meta.file_type().is_symlink() {
                    return Err(WsError::OutsideWorkspace);
                }
                if !meta.is_file() {
                    return Err(WsError::WrongKind);
                }
                // SECURITY: lost-update check -- the file must still be what the editor loaded (CWE-367).
                let current = self.read_text(rel)?;
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

        let tmp = self.temp_path(&parent, &file_name.to_string_lossy());
        let result = (|| -> Result<(), WsError> {
            // SECURITY: create_new fails if the name exists (also if it is a symlink), so the temp file is ours.
            let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(io_err)?;
            f.write_all(text.as_bytes()).map_err(io_err)?;
            f.sync_all().map_err(io_err)?;
            drop(f);
            fs::rename(&tmp, &target).map_err(io_err)?;
            Ok(())
        })();
        if result.is_err() {
            // Best effort: never leave a stray temp file behind.
            let _ = fs::remove_file(&tmp);
        }
        result?;
        Ok(version_of(text.as_bytes()))
    }

    /// Builds a temp file path next to the target: `.<name>.p2pide-<pid>-<time>-<n>.tmp`.
    ///
    /// The name only needs to be unique (create_new refuses a clash); it is not a secret, so no CSPRNG is needed.
    fn temp_path(&self, parent: &Path, name: &str) -> PathBuf {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let short: String = name.chars().take(64).collect();
        parent.join(format!(".{short}.p2pide-{}-{t}-{n}.tmp", std::process::id()))
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
