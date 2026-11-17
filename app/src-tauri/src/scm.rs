//! scm.rs -- the shell's commands for the Source Control view (phase I5, docs/features/source-control.md).
//!
//! Purpose: status, diff text against the last commit, stage, unstage, commit and the recent history of the open
//! folder's git repository. p2p_ide_core::scm does the work with the person's installed git.
//!
//! Trust boundaries: paths and the commit message come from the webview (checked in the core); git's output is
//! untrusted text returned as data (rendered by React as text, or in Monaco's read-only diff, CWE-79).
//!
//! Security controls:
//!   * The repository is re-opened (and its settings re-checked) on every command, so a settings change made while
//!     the IDE runs is caught (CWE-367).
//!   * One git write (stage, unstage, commit) at a time (`scm_busy`), so two clicks never race on git's index lock.
//!   * Audit lines with fixed words only, never paths or messages: staged, unstaged, committed (NIST AU-3, CWE-532).
//!
//! Dependencies: tauri, serde, p2p-ide-core.

use crate::{audit, AppState};
use p2p_ide_core::scm::{Commit, Repo, ScmError, Status};
use p2p_ide_core::workspace::Workspace;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::State;

fn workspace(state: &State<AppState>) -> Result<Workspace, String> {
    state
        .workspace
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or_else(|| "open a folder first".to_string())
}

fn msg(e: ScmError) -> String {
    e.message().to_string()
}

/// Runs a git call off the UI thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|_| "internal error".to_string())?
}

/// Holds the one-write-at-a-time flag until dropped.
struct Busy<'a>(&'a AtomicBool);

impl<'a> Busy<'a> {
    fn take(flag: &'a AtomicBool) -> Result<Busy<'a>, String> {
        if flag.swap(true, Ordering::SeqCst) {
            return Err("git is still busy with the last step".into());
        }
        Ok(Busy(flag))
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The branch and changed files, or why source control is off for this folder.
#[derive(Serialize)]
pub struct ScmState {
    /// False when the folder is not a repository, git is missing or the repository is refused; `reason` says why.
    pub available: bool,
    pub reason: String,
    pub status: Option<Status>,
}

/// The source control state of the open folder.
#[tauri::command]
pub async fn scm_status(state: State<'_, AppState>) -> Result<ScmState, String> {
    let ws = workspace(&state)?;
    blocking(move || {
        let repo = match Repo::open(&ws) {
            Ok(r) => r,
            Err(e) => return Ok(ScmState { available: false, reason: msg(e), status: None }),
        };
        match repo.status(&ws) {
            Ok(s) => Ok(ScmState { available: true, reason: String::new(), status: Some(s) }),
            Err(e) => Ok(ScmState { available: false, reason: msg(e), status: None }),
        }
    })
    .await
}

/// A file's text in the last commit and on disk, for the read-only diff.
#[derive(Serialize)]
pub struct ScmDiff {
    pub rel: String,
    pub before: String,
    pub after: String,
    /// True when the file is gone from disk.
    pub deleted: bool,
}

/// The diff of one changed file against the last commit.
#[tauri::command]
pub async fn scm_diff(state: State<'_, AppState>, rel: String) -> Result<ScmDiff, String> {
    let ws = workspace(&state)?;
    blocking(move || {
        let repo = Repo::open(&ws).map_err(msg)?;
        let before = repo.head_text(&ws, &rel).map_err(msg)?;
        let (after, deleted) = match ws.read_text(&rel) {
            Ok(f) => (f.text, false),
            Err(p2p_ide_core::WsError::NotFound) => (String::new(), true),
            Err(e) => return Err(e.message().to_string()),
        };
        Ok(ScmDiff { rel, before, after, deleted })
    })
    .await
}

/// Stages one file, or every change with `rel` null.
#[tauri::command]
pub async fn scm_stage(state: State<'_, AppState>, rel: Option<String>) -> Result<(), String> {
    let ws = workspace(&state)?;
    let _busy = Busy::take(&state.scm_busy)?;
    blocking(move || {
        Repo::open(&ws).and_then(|r| r.stage(&ws, rel.as_deref())).map_err(msg)?;
        audit("scm", "staged");
        Ok(())
    })
    .await
}

/// Unstages one file, or everything with `rel` null. The working file is not changed.
#[tauri::command]
pub async fn scm_unstage(state: State<'_, AppState>, rel: Option<String>) -> Result<(), String> {
    let ws = workspace(&state)?;
    let _busy = Busy::take(&state.scm_busy)?;
    blocking(move || {
        Repo::open(&ws).and_then(|r| r.unstage(&ws, rel.as_deref())).map_err(msg)?;
        audit("scm", "unstaged");
        Ok(())
    })
    .await
}

/// Commits what is staged; returns the new commit's short id.
#[tauri::command]
pub async fn scm_commit(state: State<'_, AppState>, message: String) -> Result<String, String> {
    let ws = workspace(&state)?;
    let _busy = Busy::take(&state.scm_busy)?;
    blocking(move || {
        let id = Repo::open(&ws).and_then(|r| r.commit(&message)).map_err(msg)?;
        audit("scm", "committed");
        Ok(id)
    })
    .await
}

/// The last commits, newest first.
#[tauri::command]
pub async fn scm_log(state: State<'_, AppState>) -> Result<Vec<Commit>, String> {
    let ws = workspace(&state)?;
    blocking(move || Repo::open(&ws).and_then(|r| r.log()).map_err(msg)).await
}
