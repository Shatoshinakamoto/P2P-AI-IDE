//! missions.rs -- the shell's commands for the Missions view (phase I4, docs/features/missions.md).
//!
//! Purpose: link the open folder to a project on the active gateway, send the folder there, switch Autopilot and
//! Auto Improve on or off for the project, start and control missions, decide their pending write calls, triage
//! Auto Improve proposals, collect the node copy's changes and bring them back for review. The work is done by
//! p2p_ide_core::projects and ::gwapi; this file adds the window state and runs network calls off the UI thread.
//!
//! Trust boundaries: every id and text from the webview is untrusted (checked here and in the core); every gateway
//! answer is untrusted (turned into fixed view shapes by the core, rendered as text by the webview, CWE-79).
//!
//! Security controls:
//!   * Ids go into URL paths only after `projects::id_ok` (CWE-74, CWE-918); actions are allow-listed.
//!   * Pulled files never touch the disk here: they join the pending list and wait for Accept (OWASP LLM06).
//!   * An approval is decided with the `arguments_sha256` the person was shown, so the gateway refuses a decision
//!     on arguments that changed meanwhile (CWE-345, CWE-367).
//!   * One push, collect or pull at a time per window (CWE-400).
//!   * Audit lines with fixed words only: folder pushed, changes pulled, mission started, mission control, approval
//!     decided, proposal triaged (NIST AU-2, AU-3, CWE-117, CWE-532).
//!
//! Dependencies: tauri, serde, serde_json, p2p-ide-core.

use crate::net::{active_gateway, data_dir};
use crate::{audit, AppState};
use p2p_ide_core::client::{Client, NetError};
use p2p_ide_core::projects::{
    self, acceptance, approval_view, folder_key, id_ok, mission_view, project_view, proposal_view, switches, sync_view,
    ApprovalView, CheckInput, MissionView, ProjectView, ProposalView, PullReport, PushReport, Switches, SyncView,
};
use p2p_ide_core::workspace::Workspace;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

/// What the Missions view shows at the top.
#[derive(Serialize, Default)]
pub struct ProjectState {
    pub gateway_name: String,
    /// The linked project on the active gateway, if any.
    pub project_id: Option<String>,
    pub project_name: String,
    pub switches: Option<Switches>,
    pub sync: Option<SyncView>,
    /// A short note when part of the state could not be read (the rest still shows).
    pub note: String,
    /// True when the active gateway answered 404 to the P2P-AI projects API: it is a plain model endpoint (such as
    /// Ollama typed in by hand) that can only chat, so the view does not offer to link the folder there.
    pub no_projects: bool,
}

fn workspace(state: &State<AppState>) -> Result<Workspace, String> {
    state
        .workspace
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or_else(|| "open a folder first; missions work on the open folder".to_string())
}

fn msg(e: NetError) -> String {
    e.message()
}

/// Runs a blocking network call off the UI thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|_| "internal error".to_string())?
}

/// The active gateway's client and the linked project id for the open folder.
pub(crate) fn linked(app: &AppHandle, state: &State<AppState>) -> Result<(Client, String, Workspace), String> {
    let ws = workspace(state)?;
    let (client, gid, _) = active_gateway(app)?;
    let link = projects::link_for(&data_dir(app)?, &folder_key(ws.root()), &gid)
        .ok_or("link this folder to a gateway project first")?;
    Ok((client, link.project_id, ws))
}

/// Guards push, collect and pull: one at a time per window.
struct Busy<'a>(&'a std::sync::atomic::AtomicBool);
impl<'a> Busy<'a> {
    fn take(flag: &'a std::sync::atomic::AtomicBool) -> Result<Busy<'a>, String> {
        if flag.swap(true, Ordering::SeqCst) {
            return Err("a send or pull is still running".into());
        }
        Ok(Busy(flag))
    }
}
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The folder's link on the active gateway, the project's switches and the node copy's state.
#[tauri::command]
pub async fn mission_project(app: AppHandle, state: State<'_, AppState>) -> Result<ProjectState, String> {
    let ws = workspace(&state)?;
    let (client, gid, gname) = active_gateway(&app)?;
    let link = projects::link_for(&data_dir(&app)?, &folder_key(ws.root()), &gid);
    let mut out = ProjectState { gateway_name: gname, ..Default::default() };
    let Some(link) = link else {
        // Not linked yet: ask once whether this gateway has the P2P-AI projects API at all. A plain model endpoint
        // (Ollama, LM Studio, any OpenAI-compatible server added by hand) answers 404 here, and offering to send the
        // folder to it would only fail later. Other errors (offline, refused key) keep the link offer and show a note.
        // SECURITY: only a fixed path is requested and only the error kind is kept; nothing from the answer body is
        // shown (CWE-209), and no folder content is sent by this check.
        return blocking(move || {
            match client.api("GET", "/v1/experimental", None) {
                Ok(_) => {}
                Err(NetError::NotOffered) => out.no_projects = true,
                Err(e) => out.note = format!("missions: {}", e.message()),
            }
            Ok(out)
        })
        .await;
    };
    out.project_id = Some(link.project_id.clone());
    out.project_name = link.project_name.clone();
    blocking(move || {
        let pid = link.project_id;
        match (
            client.api("GET", "/v1/experimental", None),
            client.api("GET", &format!("/v1/projects/{pid}/experimental"), None),
        ) {
            (Ok(e), Ok(f)) => out.switches = Some(switches(&e, &f)),
            (Err(e), _) | (_, Err(e)) => out.note = format!("missions: {}", e.message()),
        }
        match client.api("GET", &format!("/v1/projects/{pid}/workspace/sync"), None) {
            Ok(v) => out.sync = Some(sync_view(&v)),
            Err(_) => out.sync = Some(SyncView { state: "off".into(), ..Default::default() }),
        }
        Ok(out)
    })
    .await
}

/// The projects the key can see on the active gateway.
#[tauri::command]
pub async fn projects_list(app: AppHandle) -> Result<Vec<ProjectView>, String> {
    let (client, _, _) = active_gateway(&app)?;
    blocking(move || {
        let v = client.api("GET", "/v1/projects", None).map_err(msg)?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .map(|a| a.iter().take(500).filter_map(project_view).collect())
            .unwrap_or_default())
    })
    .await
}

/// Creates a project named `name` on the active gateway and links the open folder to it.
#[tauri::command]
pub async fn project_create(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<ProjectView, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err("a project name is 1 to 128 characters".into());
    }
    let ws = workspace(&state)?;
    let (client, gid, _) = active_gateway(&app)?;
    let dir = data_dir(&app)?;
    let folder = folder_key(ws.root());
    blocking(move || {
        let v = client.api("POST", "/v1/projects", Some(&json!({"name": name}))).map_err(msg)?;
        let p = project_view(&v).ok_or("the gateway's answer could not be read")?;
        projects::link_set(&dir, &folder, &gid, Some((&p.id, &p.name))).map_err(|e| e.message().to_string())?;
        audit("project", "created");
        Ok(p)
    })
    .await
}

/// Links the open folder to an existing project on the active gateway, or unlinks it (`project_id` null).
#[tauri::command]
pub fn project_link(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: Option<String>,
    project_name: String,
) -> Result<(), String> {
    let ws = workspace(&state)?;
    let (_, gid, _) = active_gateway(&app)?;
    if project_id.as_deref().is_some_and(|p| !id_ok(p)) {
        return Err("that is not a project id".into());
    }
    projects::link_set(
        &data_dir(&app)?,
        &folder_key(ws.root()),
        &gid,
        project_id.as_deref().map(|p| (p, project_name.as_str())),
    )
    .map_err(|e| e.message().to_string())
}

/// Sends the open folder to the linked project and asks for a copy on a node.
#[tauri::command]
pub async fn project_push(app: AppHandle, state: State<'_, AppState>) -> Result<PushReport, String> {
    let (client, pid, ws) = linked(&app, &state)?;
    let _busy = Busy::take(&state.sync_busy)?;
    let r = blocking(move || projects::push(&client, &ws, &pid).map_err(msg)).await;
    if r.is_ok() {
        audit("folder", "pushed");
    }
    r
}

/// Asks the gateway to collect the node copy's changes into the project (a new version).
#[tauri::command]
pub async fn project_collect(app: AppHandle, state: State<'_, AppState>) -> Result<SyncView, String> {
    let (client, pid, _) = linked(&app, &state)?;
    let _busy = Busy::take(&state.sync_busy)?;
    blocking(move || {
        client.api("POST", &format!("/v1/projects/{pid}/workspace/sync/delta"), Some(&json!({}))).map_err(msg)?;
        let v = client.api("GET", &format!("/v1/projects/{pid}/workspace/sync"), None).map_err(msg)?;
        Ok(sync_view(&v))
    })
    .await
}

/// Brings the project's changed files back as pending changes for review.
#[tauri::command]
pub async fn project_pull(app: AppHandle, state: State<'_, AppState>) -> Result<PullReport, String> {
    let (client, pid, ws) = linked(&app, &state)?;
    let _busy = Busy::take(&state.sync_busy)?;
    // The pending list is taken out and put back so no lock is held during the network calls (as agent.rs).
    let mut pending = std::mem::take(&mut *state.pending.lock().map_err(|_| "internal error".to_string())?);
    let (r, pending) = tauri::async_runtime::spawn_blocking(move || {
        let r = projects::pull(&client, &ws, &mut pending, &pid).map_err(msg);
        (r, pending)
    })
    .await
    .map_err(|_| "internal error".to_string())?;
    if let Ok(mut p) = state.pending.lock() {
        // Changes the chat proposed meanwhile are kept; pulled files win only where both touched the same path.
        let newer = std::mem::take(&mut *p);
        *p = pending;
        for (rel, f) in newer.files {
            p.files.entry(rel).or_insert(f);
        }
    }
    if r.is_ok() {
        audit("changes", "pulled");
    }
    r
}

/// Switches Autopilot and Auto Improve on or off for the linked project (the project owner only).
#[tauri::command]
pub async fn switches_set(
    app: AppHandle,
    state: State<'_, AppState>,
    autopilot: bool,
    auto_improve: bool,
) -> Result<Switches, String> {
    let (client, pid, _) = linked(&app, &state)?;
    blocking(move || {
        let f = client
            .api(
                "PUT",
                &format!("/v1/projects/{pid}/experimental"),
                Some(&json!({"autopilot": autopilot, "auto_improve": auto_improve})),
            )
            .map_err(msg)?;
        let e = client.api("GET", "/v1/experimental", None).map_err(msg)?;
        audit("switches", "set");
        Ok(switches(&e, &f))
    })
    .await
}

/// The key's missions on the linked project, newest first.
#[tauri::command]
pub async fn missions_list(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<MissionView>, String> {
    let (client, pid, _) = linked(&app, &state)?;
    blocking(move || {
        let v = client.api("GET", "/v1/missions", None).map_err(msg)?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .map(|a| a.iter().take(200).filter_map(mission_view).filter(|m| m.project_id == pid).collect())
            .unwrap_or_default())
    })
    .await
}

/// One mission with its tasks and checks.
#[tauri::command]
pub async fn mission_get(app: AppHandle, id: String) -> Result<MissionView, String> {
    if !id_ok(&id) {
        return Err("that is not a mission id".into());
    }
    let (client, _, _) = active_gateway(&app)?;
    blocking(move || {
        mission_view(&client.api("GET", &format!("/v1/missions/{id}"), None).map_err(msg)?)
            .ok_or("the gateway's answer could not be read".into())
    })
    .await
}

/// Starts a mission on the linked project.
///
/// * `kind` -- `autopilot` (needs `checks`) or `improve`.
/// * `goal` -- 3 to 1000 characters; `model` -- a model id the key may use.
#[tauri::command]
pub async fn mission_start(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    goal: String,
    model: String,
    checks: Vec<CheckInput>,
) -> Result<MissionView, String> {
    let (client, pid, _) = linked(&app, &state)?;
    p2p_ide_core::client::check_model(&model).map_err(msg)?;
    let goal = goal.trim().to_string();
    if goal.chars().count() < 3 || goal.chars().count() > 1000 || goal.chars().any(|c| c.is_control() && c != '\n') {
        return Err("describe the goal in 3 to 1000 characters".into());
    }
    let mut body = json!({"project_id": pid, "kind": kind, "goal": goal, "model": model});
    match kind.as_str() {
        "autopilot" => body["acceptance"] = acceptance(&checks).map_err(String::from)?,
        "improve" if checks.is_empty() => {}
        _ => return Err("a mission is Autopilot or Auto Improve".into()),
    }
    blocking(move || {
        let v = client.api("POST", "/v1/missions", Some(&body)).map_err(msg)?;
        audit("mission", "started");
        mission_view(&v).ok_or("the gateway's answer could not be read".into())
    })
    .await
}

/// Pauses, resumes or stops a mission (stop is final).
#[tauri::command]
pub async fn mission_control(app: AppHandle, id: String, action: String) -> Result<(), String> {
    if !id_ok(&id) || !matches!(action.as_str(), "pause" | "resume" | "stop") {
        return Err("unknown mission action".into());
    }
    let (client, _, _) = active_gateway(&app)?;
    blocking(move || {
        client.api("POST", &format!("/v1/missions/{id}/{action}"), Some(&json!({}))).map_err(msg)?;
        audit("mission", "control");
        Ok(())
    })
    .await
}

/// The write calls of a mission's run that wait for a decision.
#[tauri::command]
pub async fn approvals_list(app: AppHandle, run_id: String) -> Result<Vec<ApprovalView>, String> {
    if !id_ok(&run_id) {
        return Err("that is not a run id".into());
    }
    let (client, _, _) = active_gateway(&app)?;
    blocking(move || {
        let v = client.api("GET", &format!("/v1/agents/runs/{run_id}/approvals"), None).map_err(msg)?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .map(|a| a.iter().take(16).filter_map(approval_view).collect())
            .unwrap_or_default())
    })
    .await
}

/// Approves or rejects one pending write call, bound to the arguments the person saw.
#[tauri::command]
pub async fn approval_decide(
    app: AppHandle,
    run_id: String,
    approval_id: String,
    decision: String,
    arguments_sha256: String,
) -> Result<(), String> {
    let sha_ok = arguments_sha256.len() == 64 && arguments_sha256.bytes().all(|b| b.is_ascii_hexdigit());
    if !id_ok(&run_id) || !id_ok(&approval_id) || !sha_ok || !matches!(decision.as_str(), "approve" | "reject") {
        return Err("that approval could not be sent".into());
    }
    let (client, _, _) = active_gateway(&app)?;
    blocking(move || {
        let body = json!({"approval_id": approval_id, "decision": decision, "arguments_sha256": arguments_sha256});
        client.api("POST", &format!("/v1/agents/runs/{run_id}/approvals"), Some(&body)).map_err(msg)?;
        audit("approval", "decided");
        Ok(())
    })
    .await
}

/// Auto Improve proposals for the linked project.
#[tauri::command]
pub async fn proposals_list(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<ProposalView>, String> {
    let (client, pid, _) = linked(&app, &state)?;
    blocking(move || {
        let v = client.api("GET", &format!("/v1/projects/{pid}/proposals"), None).map_err(msg)?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .map(|a| a.iter().take(200).filter_map(proposal_view).collect())
            .unwrap_or_default())
    })
    .await
}

/// Accepts, dismisses or snoozes a proposal. Nothing a proposal says is run.
#[tauri::command]
pub async fn proposal_act(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    action: String,
) -> Result<(), String> {
    if !id_ok(&id) || !matches!(action.as_str(), "accept" | "dismiss" | "snooze") {
        return Err("unknown proposal action".into());
    }
    let (client, pid, _) = linked(&app, &state)?;
    blocking(move || {
        // Snooze is for 7 days; the gateway requires a whole number of days (1-90).
        let body = if action == "snooze" { json!({"days": 7}) } else { json!({}) };
        client.api("POST", &format!("/v1/projects/{pid}/proposals/{id}/{action}"), Some(&body)).map_err(msg)?;
        audit("proposal", "triaged");
        Ok(())
    })
    .await
}
