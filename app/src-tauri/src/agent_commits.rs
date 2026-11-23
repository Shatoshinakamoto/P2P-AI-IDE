//! agent_commits.rs -- the shell's commands for agent commits in the Source Control view (docs/features/agent-commits.md).
//!
//! Purpose: an agent that calls `git_commit` with mode `local` leaves a checkpoint on the gateway that waits for the
//! person. The Source Control view lists those checkpoints for the linked project, shows each one's message and
//! diff, and on the person's click either commits it in the open folder's own repository (with the person's own git
//! identity) or rejects it. The outcome is reported back to the gateway once.
//!
//! Trust boundaries: the gateway's answers (ids, messages, the patch) are untrusted; the checkpoint id from the
//! webview is untrusted. The person's yes is the click on Commit, which the webview sends only from that button.
//!
//! Security controls:
//!   * Nothing runs without the person's click: listing and showing never touch the repository (OWASP LLM06).
//!   * Ids go into URL paths only after `projects::id_ok` (CWE-74, CWE-918).
//!   * The patch is fetched again and checked (`scm::check_patch`: plain file edits of allow-listed paths only)
//!     right before it is applied, so what is committed is what passed the check now (CWE-367, CWE-22).
//!   * git gets the patch and the message on standard input, never as arguments; hooks stay off (core scm.rs).
//!   * One git write at a time (`scm_busy`), shared with the Source Control view's own stage and commit.
//!   * Audit lines with fixed words only: agent commit made, rejected, failed (NIST AU-2, AU-3, CWE-532).
//!
//! Dependencies: tauri, serde, serde_json, p2p-ide-core.

use crate::missions::linked;
use crate::{audit, AppState};
use p2p_ide_core::projects::{checkpoint_view, id_ok, CheckpointView};
use p2p_ide_core::scm::{check_patch, Repo, ScmError};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

/// Most checkpoints listed.
const MAX_LISTED: usize = 50;

/// Runs a blocking call off the UI thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|_| "internal error".to_string())?
}

/// Holds the one-git-write-at-a-time flag until dropped.
struct Busy<'a>(&'a std::sync::atomic::AtomicBool);
impl<'a> Busy<'a> {
    fn take(flag: &'a std::sync::atomic::AtomicBool) -> Result<Busy<'a>, String> {
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

fn check_id(cid: &str) -> Result<(), String> {
    if id_ok(cid) {
        Ok(())
    } else {
        Err("that is not a checkpoint id".into())
    }
}

/// The checkpoints of the linked project that wait for the person, newest first. An empty list when the folder is
/// not linked (the view then shows nothing).
#[tauri::command]
pub async fn agent_commits_pending(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<CheckpointView>, String> {
    let Ok((client, pid, _)) = linked(&app, &state) else { return Ok(vec![]) };
    blocking(move || {
        let v = client
            .api("GET", &format!("/v1/projects/{pid}/checkpoints?local=pending"), None)
            .map_err(|e| e.message())?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(checkpoint_view).filter(|c| c.local == "pending").take(MAX_LISTED).collect())
            .unwrap_or_default())
    })
    .await
}

/// What the view shows for one checkpoint before the person decides.
#[derive(Serialize)]
pub struct AgentCommitDiff {
    /// The checked diff (plain text, shown read-only; never run).
    pub diff: String,
    pub files: Vec<String>,
}

/// Fetches and checks the checkpoint's patch from the gateway; returns the diff part.
fn fetch_patch(client: &p2p_ide_core::client::Client, pid: &str, cid: &str) -> Result<String, String> {
    let bytes = client.download(&format!("/v1/projects/{pid}/checkpoints/{cid}/patch")).map_err(|e| e.message())?;
    String::from_utf8(bytes).map_err(|_| ScmError::BadPatch.message().to_string())
}

/// The diff of one checkpoint, checked against the open folder's allow-list.
#[tauri::command]
pub async fn agent_commit_diff(
    app: AppHandle,
    state: State<'_, AppState>,
    cid: String,
) -> Result<AgentCommitDiff, String> {
    check_id(&cid)?;
    let (client, pid, ws) = linked(&app, &state)?;
    blocking(move || {
        let patch = fetch_patch(&client, &pid, &cid)?;
        let c = check_patch(&patch, |p| ws.check_rel(p).is_ok()).map_err(|e| e.message().to_string())?;
        Ok(AgentCommitDiff { diff: c.diff, files: c.files })
    })
    .await
}

/// Reports the outcome of a checkpoint to the gateway (once; the gateway refuses a second report).
fn report(client: &p2p_ide_core::client::Client, pid: &str, cid: &str, body: Value) -> Result<(), String> {
    client
        .api("POST", &format!("/v1/projects/{pid}/checkpoints/{cid}/local"), Some(&body))
        .map(|_| ())
        .map_err(|e| e.message())
}

/// Commits the agent's checkpoint in the open folder's repository after the person clicked Commit; returns the
/// new commit's id. A patch that does not fit is reported as failed, so the agent learns it.
#[tauri::command]
pub async fn agent_commit_apply(app: AppHandle, state: State<'_, AppState>, cid: String) -> Result<String, String> {
    check_id(&cid)?;
    let (client, pid, ws) = linked(&app, &state)?;
    let _busy = Busy::take(&state.scm_busy)?;
    blocking(move || {
        // The message comes from the gateway's own list, as the person saw it.
        let list = client
            .api("GET", &format!("/v1/projects/{pid}/checkpoints?local=pending"), None)
            .map_err(|e| e.message())?;
        let cp = list
            .get("data")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().filter_map(checkpoint_view).find(|c| c.id == cid))
            .ok_or("this checkpoint no longer waits for a commit")?;
        let patch = fetch_patch(&client, &pid, &cid)?;
        let repo = Repo::open(&ws).map_err(|e| e.message().to_string())?;
        match repo.apply_checkpoint(&ws, &patch, &cp.message) {
            Ok(id) => {
                audit("agent-commit", "made");
                report(&client, &pid, &cid, json!({"state": "committed", "commit": id}))
                    .map_err(|e| format!("committed as {id}, but the gateway was not told: {e}"))?;
                Ok(id)
            }
            // Outcomes the agent should hear about; the person can still fix their folder and is told why.
            Err(e @ (ScmError::NotApplied | ScmError::BadPatch)) => {
                audit("agent-commit", "failed");
                let _ = report(&client, &pid, &cid, json!({"state": "failed", "note": e.message()}));
                Err(e.message().to_string())
            }
            // The person's own state (staged changes, identity, git missing): nothing is reported, they can retry.
            Err(e) => Err(e.message().to_string()),
        }
    })
    .await
}

/// The person said no: the checkpoint stays on the gateway, and the agent learns it was rejected.
#[tauri::command]
pub async fn agent_commit_reject(app: AppHandle, state: State<'_, AppState>, cid: String) -> Result<(), String> {
    check_id(&cid)?;
    let (client, pid, _) = linked(&app, &state)?;
    blocking(move || {
        report(&client, &pid, &cid, json!({"state": "rejected"}))?;
        audit("agent-commit", "rejected");
        Ok(())
    })
    .await
}
