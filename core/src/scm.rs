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
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console window flashes up.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|_| ScmError::NoGit)?;
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
}
