//! scm.rs -- source control for the open folder through the person's own `git` (phase I5, docs/features/source-control.md).
//!
//! Purpose: the Source Control view shows the branch, the changed files, a diff of each against the last commit,
//! and lets the person stage, unstage and commit. Nothing here talks to the network (no fetch, pull or push).
//!
//! Inputs and trust boundaries:
//!   * the open folder, which may be a repository someone else made (its `.git/config`, attributes and file names are
//!     untrusted);
//!   * paths and the commit message come from the webview (untrusted, checked here);
//!   * git's output (file names, commit subjects, author names) is untrusted text, returned as data.
//!
//! Security controls:
//!   * No shell: git runs from an absolute path found on PATH (never the open folder or a relative PATH entry), with
//!     an argument list (CWE-78, CWE-427).
//!   * Repository settings that make git run other programs are refused before any git call: local `filter.*` (except
//!     the standard git-lfs entries), `gpg.*`, `include.*`/`includeIf.*`, `core.worktree`, `extensions.worktreeConfig`
//!     (CWE-829, CWE-94). Hooks are switched off for every call (`core.hooksPath` to the null device, and
//!     `--no-verify` on commit) and so is `core.fsmonitor`; no external diff or text conversion is used.
//!   * A cleared environment (only PATH, home and temp folders, Windows system folders), `GIT_TERMINAL_PROMPT=0`,
//!     `GIT_LITERAL_PATHSPECS=1` so a file name like `*` or `:(top)` is never a pattern, `protocol.allow=never` so
//!     nothing reaches the network (CWE-88, CWE-526).
//!   * Paths git reports are checked with the workspace allow-list before the IDE offers any action on them; `.git`
//!     itself is never touched (CWE-22, CWE-73). The folder must be the repository's top (its own `.git` folder).
//!   * Caps on output size, entries and time; a git call past its time is killed (CWE-400, CWE-770).
//!   * Errors are fixed messages; git's own text is never shown or logged (CWE-209, CWE-532).
//!
//! Dependencies: std only (the person's installed git; no git library is added).

use crate::workspace::{Workspace, MAX_FILE_BYTES};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Most changed files listed.
pub const MAX_STATUS_ENTRIES: usize = 5000;
/// Largest git output read (status of a big repository).
const MAX_OUTPUT: usize = 8 << 20;
/// Most commits listed.
pub const MAX_LOG: usize = 50;
/// Longest commit message, in characters.
pub const MAX_MESSAGE: usize = 5000;
/// Time allowed for one git call.
const GIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Errors of a source control call; each has one fixed message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScmError {
    NoGit,
    NotRepo,
    Unsafe,
    NotOwner,
    Identity,
    NothingStaged,
    BadPath,
    BadMessage,
    TooLarge,
    NotText,
    Timeout,
    Failed,
    BadPatch,
    Staged,
    NotApplied,
}

impl ScmError {
    /// A short, fixed message that is safe to show (CWE-209).
    pub fn message(&self) -> &'static str {
        match self {
            ScmError::NoGit => "git was not found; install Git to use source control",
            ScmError::NotRepo => "this folder is not the top of a git repository",
            ScmError::Unsafe => {
                "this repository's own git settings name programs to run (filters, signing or includes), so source control is off for it"
            }
            ScmError::NotOwner => "git does not trust this folder because another user owns it",
            ScmError::Identity => "set your name and email in git first (git config --global user.name and user.email)",
            ScmError::NothingStaged => "stage at least one change before committing",
            ScmError::BadPath => "that file cannot be changed from source control",
            ScmError::BadMessage => "write a commit message of 1 to 5000 characters",
            ScmError::TooLarge => "the file or git's answer is too large to show",
            ScmError::NotText => "the file is not text, so no diff is shown",
            ScmError::Timeout => "git took too long and was stopped",
            ScmError::BadPatch => "the agent's change is not a plain text patch of files in this folder, so it was not applied",
            ScmError::Staged => "commit or unstage your own staged changes first, so the agent's commit holds only its change",
            ScmError::NotApplied => {
                "the agent's change does not fit this folder's files (they differ from the previous checkpoint); nothing was changed"
            }
            ScmError::Failed => "git could not do that",
        }
    }
}

// ---- finding git

/// Finds the git program: the first absolute PATH entry that holds it, never inside `avoid` (the open folder).
///
/// Security: relative PATH entries and the open folder are skipped, so a repository cannot plant its own `git`
/// (CWE-427).
pub fn find_git(avoid: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if !dir.is_absolute() || dir.starts_with(avoid) {
            continue;
        }
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

// ---- repository settings check

/// The standard git-lfs filter settings, the only local `filter.*` entries accepted.
const LFS_OK: [(&str, &str); 4] = [
    ("filter.lfs.clean", "git-lfs clean -- %f"),
    ("filter.lfs.smudge", "git-lfs smudge -- %f"),
    ("filter.lfs.process", "git-lfs filter-process"),
    ("filter.lfs.required", "true"),
];

/// Whether one local setting (`section.sub.key` lower-cased, its value) makes git run a program or read other files.
fn risky(key: &str, value: &str) -> bool {
    let section = key.split('.').next().unwrap_or("");
    if section == "filter" {
        return !LFS_OK.iter().any(|(k, v)| *k == key && *v == value.trim());
    }
    matches!(section, "gpg" | "include" | "includeif")
        || key == "core.worktree"
        || key == "extensions.worktreeconfig"
        || key == "core.gitproxy"
}

/// Reads `.git/config` and refuses settings that would run programs during status, add or commit.
///
/// The parser follows git's format for sections (`[name]`, `[name "sub"]`, `[name.sub]`) and `key = value` lines;
/// anything it cannot read makes it refuse (fail closed).
pub fn check_config(text: &str) -> Result<(), ScmError> {
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let mut rest = line;
        if rest.starts_with('[') {
            let end = rest.find(']').ok_or(ScmError::Unsafe)?;
            let head = &rest[1..end];
            section = match head.find('"') {
                Some(q) => {
                    let name = head[..q].trim().to_ascii_lowercase();
                    let sub = head[q + 1..].trim_end().trim_end_matches('"');
                    format!("{name}.{sub}")
                }
                None => head.trim().to_ascii_lowercase(),
            };
            rest = rest[end + 1..].trim();
            if rest.is_empty() || rest.starts_with('#') || rest.starts_with(';') {
                continue;
            }
        }
        if section.is_empty() {
            return Err(ScmError::Unsafe);
        }
        let (k, v) = match rest.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim().trim_matches('"')),
            None => (rest, "true"),
        };
        let key = format!("{section}.{}", k.to_ascii_lowercase());
        if risky(&key, v) {
            return Err(ScmError::Unsafe);
        }
    }
    Ok(())
}

// ---- running git

/// The open folder's repository: the folder itself must hold a `.git` folder (not a file: worktrees and submodules
/// keep their settings elsewhere).
#[derive(Debug, Clone)]
pub struct Repo {
    git: PathBuf,
    root: PathBuf,
}

impl Repo {
    /// Opens the repository at the workspace root after checking its settings.
    pub fn open(ws: &Workspace) -> Result<Repo, ScmError> {
        let root = ws.root().to_path_buf();
        let dot = root.join(".git");
        let meta = std::fs::symlink_metadata(&dot).map_err(|_| ScmError::NotRepo)?;
        if !meta.is_dir() {
            return Err(ScmError::NotRepo);
        }
        let config = std::fs::read(dot.join("config")).map_err(|_| ScmError::NotRepo)?;
        if config.len() > 256 << 10 {
            return Err(ScmError::Unsafe);
        }
        check_config(&String::from_utf8_lossy(&config))?;
        let git = find_git(&root).ok_or(ScmError::NoGit)?;
        Ok(Repo { git, root })
    }

    /// Runs git with the fixed safety options and `args`; returns stdout (capped) or a fixed error.
    fn run(&self, args: &[&str]) -> Result<Vec<u8>, ScmError> {
        self.run_with(args, None)
    }

    /// `run` with `input` written to git's standard input (a patch for `git apply -`, a message for `commit -F -`),
    /// so neither ever becomes a command-line argument or a file in the repository (CWE-88).
    fn run_with(&self, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, ScmError> {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let mut cmd = Command::new(&self.git);
        // SECURITY: a cleared environment, then only what git needs to find the person's own settings (CWE-526).
        cmd.env_clear();
        for k in [
            "PATH",
            "HOME",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
            "APPDATA",
            "LOCALAPPDATA",
            "XDG_CONFIG_HOME",
            "SYSTEMROOT",
            "WINDIR",
            "TEMP",
            "TMP",
            "TMPDIR",
        ] {
            if let Some(v) = std::env::var_os(k) {
                cmd.env(k, v);
            }
        }
        cmd.env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_LITERAL_PATHSPECS", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .arg("--no-pager")
            .arg("-C")
            .arg(&self.root)
            // SECURITY: no hooks, no file-system monitor program, nothing from the network (CWE-829).
            .args(["-c", &format!("core.hooksPath={null}")])
            .args(["-c", "core.fsmonitor=false", "-c", "protocol.allow=never", "-c", "submodule.recurse=false"])
            .args(["-c", "gc.auto=0", "-c", "maintenance.auto=false", "-c", "core.quotePath=false"])
            .args(args)
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console window flashes up.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|_| ScmError::NoGit)?;
        if let Some(data) = input {
            let mut sin = child.stdin.take().ok_or(ScmError::Failed)?;
            let data = data.to_vec();
            // Written on its own thread and then closed, so a git that stops reading never blocks this one.
            std::thread::spawn(move || {
                use std::io::Write;
                let _ = sin.write_all(&data);
            });
        }
        let mut out = child.stdout.take().ok_or(ScmError::Failed)?;
        let mut err = child.stderr.take().ok_or(ScmError::Failed)?;
        let reader = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = (&mut out).take(MAX_OUTPUT as u64 + 1).read_to_end(&mut b);
            // Drain the rest so git never blocks on a full pipe.
            let _ = std::io::copy(&mut out, &mut std::io::sink());
            b
        });
        let ereader = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = (&mut err).take(64 << 10).read_to_end(&mut b);
            let _ = std::io::copy(&mut err, &mut std::io::sink());
            b
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if start.elapsed() > GIT_TIMEOUT => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ScmError::Timeout);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => return Err(ScmError::Failed),
            }
        };
        let stdout = reader.join().map_err(|_| ScmError::Failed)?;
        let stderr = String::from_utf8_lossy(&ereader.join().map_err(|_| ScmError::Failed)?).to_ascii_lowercase();
        if stdout.len() > MAX_OUTPUT {
            return Err(ScmError::TooLarge);
        }
        if status.success() {
            return Ok(stdout);
        }
        // Known failures get their own fixed message; git's text itself is never passed on (CWE-209).
        Err(if stderr.contains("dubious ownership") {
            ScmError::NotOwner
        } else if stderr.contains("tell me who you are") || stderr.contains("empty ident") {
            ScmError::Identity
        } else if stderr.contains("not a git repository") {
            ScmError::NotRepo
        } else {
            ScmError::Failed
        })
    }
}

// ---- status

/// One changed file. `index` and `work` are git's status letters (`M`, `A`, `D`, `R`, `C`, `U`, `?`, or `.` for
/// none) for the staged and the unstaged side.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileStatus {
    pub rel: String,
    pub index: String,
    pub work: String,
    pub conflict: bool,
    pub untracked: bool,
    /// False when the IDE offers no action on it (a name outside the allow-list, `.git`, or a submodule).
    pub actionable: bool,
}

/// The branch and the changed files.
#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct Status {
    /// The branch name, or empty when detached.
    pub branch: String,
    /// True before the first commit.
    pub initial: bool,
    pub ahead: i64,
    pub behind: i64,
    pub files: Vec<FileStatus>,
    pub truncated: bool,
}

/// Parses `git status --porcelain=v2 -z --branch --no-renames` output. Paths are checked with `ok_path`.
pub fn parse_status(out: &[u8], ok_path: impl Fn(&str) -> bool) -> Status {
    let mut st = Status::default();
    for rec in out.split(|b| *b == 0) {
        if rec.is_empty() {
            continue;
        }
        let line = String::from_utf8_lossy(rec);
        let clean = |s: &str| s.chars().filter(|c| !c.is_control()).take(200).collect::<String>();
        if let Some(h) = line.strip_prefix("# branch.head ") {
            st.branch = if h == "(detached)" { String::new() } else { clean(h) };
        } else if let Some(o) = line.strip_prefix("# branch.oid ") {
            st.initial = o == "(initial)";
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut it = ab.split_whitespace();
            st.ahead = it.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
            st.behind = it.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if line.starts_with('#') {
            continue;
        } else {
            if st.files.len() >= MAX_STATUS_ENTRIES {
                st.truncated = true;
                continue;
            }
            // Fields before the path: "1" has 8, "u" has 10, "?" has 1 (then the path, which may hold spaces).
            let (kind, fields) = match line.chars().next() {
                Some('1') => ('1', 8),
                Some('u') => ('u', 10),
                Some('?') => ('?', 1),
                _ => continue,
            };
            let mut parts = line.splitn(fields + 1, ' ');
            let head: Vec<&str> = parts.by_ref().take(fields).collect();
            let Some(path) = parts.next() else { continue };
            let (x, y) = if kind == '?' {
                (".".to_string(), "?".to_string())
            } else {
                let xy = head.get(1).copied().unwrap_or("..");
                (xy.chars().next().unwrap_or('.').to_string(), xy.chars().nth(1).unwrap_or('.').to_string())
            };
            let submodule = head.get(2).is_some_and(|s| s.starts_with('S'));
            let inside_git = path == ".git" || path.starts_with(".git/");
            st.files.push(FileStatus {
                rel: clean(path),
                index: x,
                work: y,
                conflict: kind == 'u',
                untracked: kind == '?',
                actionable: !submodule && !inside_git && path.len() <= 1024 && ok_path(path),
            });
        }
    }
    st
}

impl Repo {
    /// The branch and changed files.
    pub fn status(&self, ws: &Workspace) -> Result<Status, ScmError> {
        let out = self.run(&["status", "--porcelain=v2", "-z", "--branch", "--no-renames", "--untracked-files=all"])?;
        Ok(parse_status(&out, |p| ws.check_rel(p).is_ok()))
    }

    /// Checks a path the webview names before git acts on it.
    fn checked<'a>(&self, ws: &Workspace, rel: &'a str) -> Result<&'a str, ScmError> {
        if rel == ".git" || rel.starts_with(".git/") || ws.check_rel(rel).is_err() {
            return Err(ScmError::BadPath);
        }
        Ok(rel)
    }

    /// Stages one file (also a deletion), or everything with `rel = None`.
    pub fn stage(&self, ws: &Workspace, rel: Option<&str>) -> Result<(), ScmError> {
        match rel {
            Some(r) => self.run(&["add", "-A", "--", self.checked(ws, r)?]).map(|_| ()),
            None => self.run(&["add", "-A"]).map(|_| ()),
        }
    }

    /// Unstages one file, or everything with `rel = None` (the working file is left as it is).
    pub fn unstage(&self, ws: &Workspace, rel: Option<&str>) -> Result<(), ScmError> {
        match rel {
            Some(r) => self.run(&["reset", "-q", "--", self.checked(ws, r)?]).map(|_| ()),
            None => self.run(&["reset", "-q"]).map(|_| ()),
        }
    }

    /// Commits what is staged with `message`; returns the new commit's short id.
    ///
    /// Security: the message is one argument (no shell); hooks are off (`--no-verify` and `core.hooksPath`).
    pub fn commit(&self, message: &str) -> Result<String, ScmError> {
        let m = message.trim();
        if m.is_empty()
            || m.chars().count() > MAX_MESSAGE
            || m.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(ScmError::BadMessage);
        }
        // Nothing staged: git would refuse with a message about untracked files; say it plainly instead.
        if self.run(&["diff", "--cached", "--quiet", "--no-ext-diff"]).is_ok() {
            return Err(ScmError::NothingStaged);
        }
        self.run(&["commit", "--no-verify", "-q", "-m", m])?;
        let id = self.run(&["rev-parse", "--short", "HEAD"])?;
        Ok(String::from_utf8_lossy(&id).trim().chars().filter(|c| c.is_ascii_hexdigit()).take(40).collect())
    }

    /// The file as it is in the last commit (empty when it is new). `None` when it is not text.
    pub fn head_text(&self, ws: &Workspace, rel: &str) -> Result<String, ScmError> {
        let rel = self.checked(ws, rel)?;
        // `HEAD:<path>` names the path literally; cat-file applies no filters or text conversion.
        let spec = format!("HEAD:{rel}");
        match self.run(&["cat-file", "blob", &spec]) {
            Ok(b) if b.len() as u64 > MAX_FILE_BYTES => Err(ScmError::TooLarge),
            Ok(b) => String::from_utf8(b).map_err(|_| ScmError::NotText),
            // New file, or no commit yet.
            Err(ScmError::Failed) => Ok(String::new()),
            Err(e) => Err(e),
        }
    }

    /// The last commits, newest first.
    pub fn log(&self) -> Result<Vec<Commit>, ScmError> {
        let n = format!("-n{MAX_LOG}");
        let out = match self.run(&["log", &n, "--no-show-signature", "--format=%h%x1f%s%x1f%an%x1f%at%x1e"]) {
            Ok(o) => o,
            // No commit yet.
            Err(ScmError::Failed) => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        Ok(parse_log(&out))
    }
}

// ---- the agent's commits (git_commit mode "local", docs/features/agent-commits.md)

/// Largest patch the IDE applies (the gateway stops at 8 MiB).
pub const MAX_PATCH: usize = 8 << 20;

/// A checked patch: the diff part only (from the first `diff --git` line, without the email head and the
/// signature), and the files it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPatch {
    pub diff: String,
    pub files: Vec<String>,
}

/// Checks a checkpoint patch from the gateway before git sees it, and cuts it to its diff part.
///
/// Allowed: per file `diff --git a/P b/P` (the same path twice), then optionally `new file mode 100644` or
/// `deleted file mode 100644` and an `index` line, then `--- a/P` or `--- /dev/null`, `+++ b/P` or `+++ /dev/null`,
/// then hunks whose line counts match their `@@` header exactly, each line starting with ` `, `+`, `-` or `\\`.
/// Everything else in the diff part is refused: renames, copies, mode changes, symbolic links, binary patches,
/// submodules (CWE-20, CWE-73). Each path must pass `ok_path` and must not be `.git` or under it (CWE-22).
///
/// SECURITY: git only ever sees what this function returns; `git apply` checks the paths again on its own.
pub fn check_patch(patch: &str, ok_path: impl Fn(&str) -> bool) -> Result<CheckedPatch, ScmError> {
    if patch.len() > MAX_PATCH || patch.contains('\0') || patch.contains('\r') {
        return Err(ScmError::BadPatch);
    }
    let lines: Vec<&str> = patch.split('\n').collect();
    let Some(first) = lines.iter().position(|l| l.starts_with("diff --git ")) else {
        return Err(ScmError::BadPatch);
    };
    // SECURITY: whatever ok_path says, a path is relative, uses "/" only, and has no empty, "." or ".." part and no
    // ".git" part (CWE-22, CWE-73).
    let path_ok = |p: &str| {
        !p.is_empty()
            && !p.starts_with('/')
            && !p.contains('\\')
            && !p.contains(':')
            && p.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != ".." && !seg.eq_ignore_ascii_case(".git"))
            && ok_path(p)
    };
    let mut out: Vec<&str> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut i = first;
    while i < lines.len() {
        let l = lines[i];
        // The signature ("-- " then the sender) and the empty end close the diff part.
        if l == "-- " || (l.is_empty() && i + 1 == lines.len()) {
            break;
        }
        let Some(rest) = l.strip_prefix("diff --git a/") else { return Err(ScmError::BadPatch) };
        // "P b/P": the same path twice, so a space in a name cannot shift the split.
        if rest.len() < 3 || (rest.len() - 3) % 2 != 0 {
            return Err(ScmError::BadPatch);
        }
        let n = (rest.len() - 3) / 2;
        let (pa, pb) = (&rest[..n], &rest[n..]);
        let path = pa;
        if pb != format!(" b/{path}") || !path_ok(path) || files.iter().any(|f| f == path) {
            return Err(ScmError::BadPatch);
        }
        files.push(path.to_string());
        out.push(l);
        i += 1;
        let (mut new_file, mut deleted) = (false, false);
        if lines.get(i) == Some(&"new file mode 100644") {
            new_file = true;
            out.push(lines[i]);
            i += 1;
        } else if lines.get(i) == Some(&"deleted file mode 100644") {
            deleted = true;
            out.push(lines[i]);
            i += 1;
        }
        if let Some(x) = lines.get(i) {
            if let Some(ix) = x.strip_prefix("index ") {
                if !ix.chars().all(|c| c.is_ascii_hexdigit() || c == '.' || c == ' ' || c.is_ascii_digit()) {
                    return Err(ScmError::BadPatch);
                }
                out.push(lines[i]);
                i += 1;
            }
        }
        let minus = if new_file { "--- /dev/null".to_string() } else { format!("--- a/{path}") };
        let plus = if deleted { "+++ /dev/null".to_string() } else { format!("+++ b/{path}") };
        // A new or deleted empty file has no ---/+++ lines and no hunks.
        if (new_file || deleted)
            && lines.get(i).map(|x| x.starts_with("diff --git ") || *x == "-- " || x.is_empty()) != Some(false)
        {
            continue;
        }
        if lines.get(i) != Some(&minus.as_str()) || lines.get(i + 1) != Some(&plus.as_str()) {
            return Err(ScmError::BadPatch);
        }
        out.push(lines[i]);
        out.push(lines[i + 1]);
        i += 2;
        let mut hunks = 0;
        while let Some(h) = lines.get(i).and_then(|x| x.strip_prefix("@@ -")) {
            let (a, b) = hunk_counts(h).ok_or(ScmError::BadPatch)?;
            out.push(lines[i]);
            i += 1;
            let (mut ra, mut rb) = (a, b);
            while ra > 0 || rb > 0 {
                let x = *lines.get(i).ok_or(ScmError::BadPatch)?;
                match x.as_bytes().first() {
                    Some(b' ') if ra > 0 && rb > 0 => (ra, rb) = (ra - 1, rb - 1),
                    Some(b'-') if ra > 0 => ra -= 1,
                    Some(b'+') if rb > 0 => rb -= 1,
                    Some(b'\\') => {}
                    _ => return Err(ScmError::BadPatch),
                }
                out.push(x);
                i += 1;
            }
            // "\ No newline at end of file" after the last line of a hunk.
            while lines.get(i).map(|x| x.starts_with('\\')) == Some(true) {
                out.push(lines[i]);
                i += 1;
            }
            hunks += 1;
        }
        if hunks == 0 {
            return Err(ScmError::BadPatch);
        }
    }
    if files.is_empty() {
        return Err(ScmError::BadPatch);
    }
    let mut diff = out.join("\n");
    diff.push('\n');
    Ok(CheckedPatch { diff, files })
}

/// Reads `A[,N] +B[,M] @@...` (after "@@ -"): the line counts of both sides (1 when N or M is left out).
fn hunk_counts(h: &str) -> Option<(usize, usize)> {
    let (ranges, _) = h.split_once(" @@")?;
    let (a, b) = ranges.split_once(" +")?;
    let count = |r: &str| -> Option<usize> {
        let mut it = r.splitn(2, ',');
        it.next()?.parse::<usize>().ok()?;
        match it.next() {
            Some(n) => n.parse::<usize>().ok().filter(|n| *n <= 1_000_000),
            None => Some(1),
        }
    };
    Some((count(a)?, count(b)?))
}

impl Repo {
    /// Applies an agent's checkpoint patch to the index and the files, and commits it with `message` under the
    /// person's own git identity; returns the new commit's full id. Call only after the person said yes.
    ///
    /// Security: the patch is checked first (`check_patch`) and reaches git on standard input; the message goes to
    /// `commit -F -` on standard input; hooks stay off. Nothing of the person's is mixed in: with anything staged
    /// it refuses. If the patch does not fit, git changes nothing (`git apply` is all or nothing) (CWE-20, CWE-88).
    pub fn apply_checkpoint(&self, ws: &Workspace, patch: &str, message: &str) -> Result<String, ScmError> {
        let m = message.trim();
        if m.is_empty()
            || m.chars().count() > MAX_MESSAGE
            || m.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(ScmError::BadMessage);
        }
        let checked = check_patch(patch, |p| ws.check_rel(p).is_ok())?;
        // Staged changes would end up in the agent's commit.
        if self.run(&["diff", "--cached", "--quiet", "--no-ext-diff"]).is_err() {
            if self.run(&["rev-parse", "--verify", "-q", "HEAD"]).is_ok() {
                return Err(ScmError::Staged);
            }
            // No commit yet: only an empty index is fine.
            if !self.run(&["ls-files", "--cached", "-z"])?.is_empty() {
                return Err(ScmError::Staged);
            }
        }
        // `apply --index` compares the files' stat data with the index; refresh it first so an unchanged file that
        // was only touched is not taken for a change. (`-q` and a non-zero exit for changed files are expected.)
        let _ = self.run(&["update-index", "-q", "--refresh"]);
        self.run_with(&["apply", "--index", "--check", "-"], Some(checked.diff.as_bytes())).map_err(|e| {
            if e == ScmError::Failed {
                ScmError::NotApplied
            } else {
                e
            }
        })?;
        self.run_with(&["apply", "--index", "-"], Some(checked.diff.as_bytes())).map_err(|e| {
            if e == ScmError::Failed {
                ScmError::NotApplied
            } else {
                e
            }
        })?;
        let mut body = m.to_string();
        body.push('\n');
        if let Err(e) = self.run_with(&["commit", "--no-verify", "-q", "-F", "-"], Some(body.as_bytes())) {
            // Put the files and the index back as they were, so a failed commit leaves nothing half done.
            let _ = self.run_with(&["apply", "--index", "-R", "-"], Some(checked.diff.as_bytes()));
            return Err(e);
        }
        let id = self.run(&["rev-parse", "HEAD"])?;
        let id: String = String::from_utf8_lossy(&id).trim().to_string();
        if !(id.len() == 40 || id.len() == 64) || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ScmError::Failed);
        }
        Ok(id.to_ascii_lowercase())
    }
}

/// One commit in the history list.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub subject: String,
    pub author: String,
    pub time: i64,
}

/// Parses the `log` format above (fields split by 0x1f, records by 0x1e).
pub fn parse_log(out: &[u8]) -> Vec<Commit> {
    let text = String::from_utf8_lossy(out);
    let clean = |s: &str, n: usize| s.chars().filter(|c| !c.is_control()).take(n).collect::<String>();
    text.split('\u{1e}')
        .filter_map(|r| {
            let f: Vec<&str> = r.trim_start_matches('\n').split('\u{1f}').collect();
            if f.len() != 4 || f[0].is_empty() || !f[0].bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            Some(Commit {
                id: clean(f[0], 40),
                subject: clean(f[1], 300),
                author: clean(f[2], 100),
                time: f[3].trim().parse().unwrap_or(0),
            })
        })
        .take(MAX_LOG)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_settings_that_run_programs_are_refused() {
        assert!(check_config(
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[remote \"origin\"]\n\turl = x\n"
        )
        .is_ok());
        // Husky-style hooks path is fine: hooks are switched off for every call anyway.
        assert!(check_config("[core]\n\thooksPath = .husky\n").is_ok());
        assert!(check_config("[filter \"lfs\"]\n\tclean = git-lfs clean -- %f\n\tsmudge = git-lfs smudge -- %f\n\tprocess = git-lfs filter-process\n\trequired = true\n").is_ok());
        for bad in [
            "[filter \"x\"]\n\tclean = sh -c evil\n",
            "[filter \"lfs\"]\n\tclean = evil\n",
            "[gpg]\n\tprogram = evil\n",
            "[include]\n\tpath = ../other\n",
            "[includeIf \"gitdir:/x\"]\n\tpath = y\n",
            "[core]\n\tworktree = /elsewhere\n",
            "[extensions]\n\tworktreeConfig = true\n",
            "[FILTER \"x\"] clean = evil\n",
            "[filter.x]\n\tsmudge = evil\n",
            "key = value-before-any-section\n",
            "[core\n",
        ] {
            assert_eq!(check_config(bad), Err(ScmError::Unsafe), "{bad}");
        }
    }

    #[test]
    fn status_is_parsed_and_odd_paths_get_no_actions() {
        let out = b"# branch.oid (initial)\0# branch.head main\0# branch.ab +2 -1\0\
1 M. N... 100644 100644 100644 a b src/a b.py\0\
1 .D N... 100644 100644 000000 a b gone.txt\0\
u UU N... 100644 100644 100644 100644 a b c both.txt\0\
? new dir/x.txt\0\
? bad\\name\0\
1 .M S.M. 160000 160000 160000 a b sub\0";
        let st = parse_status(out, |p| !p.contains('\\'));
        assert_eq!((st.branch.as_str(), st.initial, st.ahead, st.behind), ("main", true, 2, 1));
        assert_eq!(st.files.len(), 6);
        assert_eq!(
            (st.files[0].rel.as_str(), st.files[0].index.as_str(), st.files[0].work.as_str()),
            ("src/a b.py", "M", ".")
        );
        assert_eq!((st.files[1].index.as_str(), st.files[1].work.as_str()), (".", "D"));
        assert!(st.files[2].conflict);
        assert!(st.files[3].untracked && st.files[3].actionable);
        assert!(!st.files[4].actionable, "a name the workspace refuses");
        assert!(!st.files[5].actionable, "a submodule");
    }

    #[test]
    fn log_is_parsed_and_cleaned() {
        let out = "abc1234\u{1f}Fix add\u{1b}[31m\u{1f}Dev\u{1f}1700000000\u{1e}\nzz\u{1f}x\u{1f}y\u{1f}1\u{1e}\n";
        let l = parse_log(out.as_bytes());
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].subject, "Fix add[31m", "control characters are removed");
        assert_eq!(l[0].time, 1_700_000_000);
    }

    /// End to end with the real git, when it is installed: status, stage, commit, diff text, hooks never run.
    #[test]
    fn real_git_round_trip_without_hooks() {
        let d = std::env::temp_dir().join(format!("p2pide-scm-{}", crate::store::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        let Some(git) = find_git(&d) else { return };
        let g = |args: &[&str]| {
            Command::new(&git).arg("-C").arg(&d).args(args).output().unwrap();
        };
        g(&["init", "-q"]);
        g(&["config", "user.name", "Test"]);
        g(&["config", "user.email", "test@example.invalid"]);
        // A hook that would leave a marker file if it ran.
        let hooks = d.join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let marker = d.join("hook-ran");
        std::fs::write(hooks.join("pre-commit"), format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(hooks.join("pre-commit"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(d.join("a.txt"), "one\n").unwrap();
        // `[a].txt` is a valid name on every system and, read as a pattern, would also match a.txt.
        std::fs::write(d.join("[a].txt"), "glob\n").unwrap();
        let ws = Workspace::open(&d).unwrap();
        let repo = Repo::open(&ws).unwrap();
        let st = repo.status(&ws).unwrap();
        assert!(st.initial);
        assert_eq!(st.files.iter().filter(|f| f.untracked).count(), 2);
        assert_eq!(repo.commit("x"), Err(ScmError::NothingStaged));
        // SECURITY: a file named like a pattern is staged literally, not as a pattern matching a.txt too.
        repo.stage(&ws, Some("[a].txt")).unwrap();
        let st = repo.status(&ws).unwrap();
        let a = st.files.iter().find(|f| f.rel == "a.txt").unwrap();
        assert!(a.untracked, "a.txt was not staged by the `[a].txt` file");
        repo.stage(&ws, None).unwrap();
        let id = repo.commit("first commit").unwrap();
        assert!(id.len() >= 7);
        assert!(!marker.exists(), "SECURITY: the pre-commit hook must not run");
        std::fs::write(d.join("a.txt"), "two\n").unwrap();
        assert_eq!(repo.head_text(&ws, "a.txt").unwrap(), "one\n");
        assert_eq!(repo.head_text(&ws, "brand-new.txt").unwrap(), "");
        assert_eq!(repo.head_text(&ws, ".git/config"), Err(ScmError::BadPath));
        assert_eq!(repo.head_text(&ws, "../x"), Err(ScmError::BadPath));
        repo.stage(&ws, Some("a.txt")).unwrap();
        repo.unstage(&ws, Some("a.txt")).unwrap();
        let st = repo.status(&ws).unwrap();
        let a = st.files.iter().find(|f| f.rel == "a.txt").unwrap();
        assert_eq!((a.index.as_str(), a.work.as_str()), (".", "M"));
        let log = repo.log().unwrap();
        assert_eq!(log[0].subject, "first commit");
        // A filter in the repository's own settings turns source control off.
        let cfg = d.join(".git/config");
        let mut text = std::fs::read_to_string(&cfg).unwrap();
        text.push_str("[filter \"evil\"]\n\tclean = touch pwned\n");
        std::fs::write(&cfg, text).unwrap();
        assert_eq!(Repo::open(&ws).err(), Some(ScmError::Unsafe));
        let _ = std::fs::remove_dir_all(&d);
    }

    const GOOD: &str = "From 0000000000000000000000000000000000000000 Mon Sep 17 00:00:00 2001\n\
From: P2P-AI checkpoint <checkpoint@p2p-ai.invalid>\n\
Subject: [PATCH] Update\n\n body\n ---\n diff --git a/x b/x\n---\n\n\
diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-alpha\n+ALPHA\n\
diff --git a/my file.txt b/my file.txt\nnew file mode 100644\n--- /dev/null\n+++ b/my file.txt\n@@ -0,0 +1 @@\n+no break\n\\ No newline at end of file\n\
diff --git a/empty.txt b/empty.txt\nnew file mode 100644\n\
diff --git a/b.txt b/b.txt\ndeleted file mode 100644\n--- a/b.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-beta\n\
-- \nP2P-AI\n";

    #[test]
    fn checkpoint_patches_are_checked_and_cut_to_the_diff() {
        let c = check_patch(GOOD, |_| true).unwrap();
        assert_eq!(c.files, vec!["a.txt", "my file.txt", "empty.txt", "b.txt"]);
        assert!(c.diff.starts_with("diff --git a/a.txt b/a.txt\n"), "the email head is cut off");
        assert!(!c.diff.contains("P2P-AI\n") && !c.diff.contains("\n-- \n"), "the signature is cut off");
        // SECURITY: whatever a hostile gateway puts in the diff part, only plain file edits pass (CWE-20, CWE-22).
        let hunk = "--- a/P\n+++ b/P\n@@ -1 +1 @@\n-a\n+b\n";
        let one = |path: &str, extra: &str| format!("diff --git a/{path} b/{path}\n{extra}{}", hunk.replace('P', path));
        for (name, bad) in [
            ("dot dot", one("../evil", "")),
            ("dot git", one(".git/config", "")),
            ("nested dot git", one("sub/.git/hooks/post-checkout", "")),
            ("rename", one("a.txt", "rename from a.txt\nrename to b.txt\n")),
            ("mode", one("a.txt", "old mode 100644\nnew mode 100755\n")),
            (
                "symlink",
                "diff --git a/l b/l\nnew file mode 120000\n--- /dev/null\n+++ b/l\n@@ -0,0 +1 @@\n+/etc/passwd\n"
                    .into(),
            ),
            ("binary", "diff --git a/a.txt b/a.txt\nGIT binary patch\nliteral 0\n".into()),
            ("other path in ---", "diff --git a/a.txt b/a.txt\n--- a/b.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+b\n".into()),
            ("two paths", "diff --git a/a.txt b/b.txt\n--- a/a.txt\n+++ b/b.txt\n@@ -1 +1 @@\n-a\n+b\n".into()),
            ("short hunk", "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n-a\n+b\n".into()),
            ("extra line", format!("{}+smuggled\n", one("a.txt", ""))),
            ("same file twice", format!("{}{}", one("a.txt", ""), one("a.txt", ""))),
            ("carriage return", one("a.txt", "").replace("+b", "+b\r")),
            ("no diff", "Subject: x\n\nnothing\n".into()),
            ("no hunk", "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n".into()),
        ] {
            assert_eq!(check_patch(&bad, |_| true), Err(ScmError::BadPatch), "{name}");
        }
        assert_eq!(check_patch(GOOD, |p| p != "a.txt"), Err(ScmError::BadPatch), "the workspace allow-list applies");
        assert_eq!(hunk_counts("3,0 +4,2 @@ fn x"), Some((0, 2)));
        assert_eq!(hunk_counts("3 +4 @@"), Some((1, 1)));
        assert_eq!(hunk_counts("x +4 @@"), None);
    }

    /// With the real git: the agent's patch is applied and committed with the message, hooks never run; refused
    /// with the person's own staged changes; a patch that does not fit changes nothing.
    #[test]
    fn real_git_applies_an_agent_checkpoint() {
        let d = std::env::temp_dir().join(format!("p2pide-ckpt-{}", crate::store::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        let Some(git) = find_git(&d) else { return };
        let g = |args: &[&str]| {
            Command::new(&git).arg("-C").arg(&d).args(args).output().unwrap();
        };
        g(&["init", "-q"]);
        g(&["config", "user.name", "Test"]);
        g(&["config", "user.email", "test@example.invalid"]);
        // The person's own line-ending setting applies to the files git writes (Windows often has autocrlf on);
        // the test fixes it so the bytes it compares are the same on every system.
        g(&["config", "core.autocrlf", "false"]);
        std::fs::write(d.join("a.txt"), "alpha\n").unwrap();
        std::fs::write(d.join("b.txt"), "beta\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        // A hook that leaves a marker if it runs, added after the test's own base commit (which runs hooks).
        let marker = d.join("hook-ran");
        let hooks = d.join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        for name in ["pre-commit", "commit-msg", "post-commit", "pre-applypatch", "post-applypatch"] {
            std::fs::write(hooks.join(name), format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(hooks.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        let ws = Workspace::open(&d).unwrap();
        let repo = Repo::open(&ws).unwrap();

        // The person's own staged change: refused, nothing touched.
        std::fs::write(d.join("b.txt"), "mine\n").unwrap();
        repo.stage(&ws, Some("b.txt")).unwrap();
        assert_eq!(repo.apply_checkpoint(&ws, GOOD, "Update"), Err(ScmError::Staged));
        repo.unstage(&ws, None).unwrap();
        // A patch that does not fit (b.txt differs): nothing changes.
        assert_eq!(repo.apply_checkpoint(&ws, GOOD, "Update"), Err(ScmError::NotApplied));
        assert_eq!(std::fs::read_to_string(d.join("a.txt")).unwrap(), "alpha\n");
        std::fs::write(d.join("b.txt"), "beta\n").unwrap();

        let msg = "Update files\n\n---\nWhy: the parser.";
        let id = repo.apply_checkpoint(&ws, GOOD, msg).unwrap();
        assert!(id.len() == 40 || id.len() == 64, "full commit id: {id}");
        assert_eq!(std::fs::read_to_string(d.join("a.txt")).unwrap(), "ALPHA\n");
        assert_eq!(std::fs::read_to_string(d.join("my file.txt")).unwrap(), "no break");
        assert_eq!(std::fs::read_to_string(d.join("empty.txt")).unwrap(), "");
        assert!(!d.join("b.txt").exists());
        assert!(!marker.exists(), "SECURITY: no repository hook may run");
        let body = Command::new(&git).arg("-C").arg(&d).args(["log", "-1", "--format=%B"]).output().unwrap().stdout;
        assert_eq!(String::from_utf8_lossy(&body).trim(), msg, "the message is the commit message, as written");
        assert!(repo.status(&ws).unwrap().files.is_empty(), "everything is committed");
        let _ = std::fs::remove_dir_all(&d);
    }
}
