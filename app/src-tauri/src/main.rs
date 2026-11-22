//! main.rs -- the P2P-AI IDE's Tauri shell (phases I1-I5, docs/features/editor-shell.md,
//! docs/features/gateways-and-chat.md, docs/features/agent-tools.md, docs/features/missions.md,
//! docs/features/source-control.md).
//!
//! Purpose: opens the window and exposes typed commands to the webview: pick a workspace folder, show its name,
//! list a folder, read a file, save a file and search (here), plus the gateway and chat commands in `net.rs`, the
//! chat tools in `agent.rs` and the Missions view in `missions.rs`.
//! All file and network work is done by p2p-ide-core.
//!
//! Trust boundaries: the webview is untrusted input to every command (it renders file contents and, later, model
//! output). The folder itself is chosen by the person in the OS folder picker, never typed by the webview, so a
//! compromised page cannot point the IDE at another folder (CWE-73, NIST AC-6).
//!
//! Security controls:
//!   * No plugin permissions for the webview; the dialog plugin is called from Rust only (capabilities/default.json).
//!   * Every relative path, file body and query is checked by p2p-ide-core (CWE-22, CWE-59, CWE-400).
//!   * Errors reach the webview as short fixed messages (CWE-209). Refusals are logged to stderr with a UTC timestamp
//!     and the error kind only, never the path or file text (NIST AU-3, CWE-532, CWE-117 log injection).
//!   * The full root path stays in Rust; the webview only learns the folder's name (CWE-200).
//!
//! Dependencies: tauri 2 (the app shell), tauri-plugin-dialog 2 (native folder and file pickers), keyring 4 (OS
//! keychain, net.rs), serde, p2p-ide-core.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod ext;
mod missions;
mod net;
mod nodehost;
mod run;
mod scm;

use p2p_ide_core::{FileText, Listing, SearchResult, Workspace, WsError};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

/// A run request waiting for the person: its random id and where the Allow (true) or Decline (false) goes.
type WaitingRun = (String, std::sync::mpsc::Sender<bool>);

/// The open workspace, if any (one per window), and the state of the chat turn that is streaming.
struct AppState {
    workspace: Mutex<Option<Workspace>>,
    /// True while a chat answer is coming in (one turn at a time).
    chat_busy: AtomicBool,
    /// Set by the Stop button; the streaming loop checks it between pieces of text.
    chat_stop: Arc<AtomicBool>,
    /// The last public gateway list (phase I2b). `gateway_add_public` takes the address from here, never from the
    /// webview, so a picked gateway's address is the one in its verified advert (CWE-345).
    public: Mutex<Vec<p2p_ide_core::discovery::PublicGateway>>,
    /// Changes the agent proposed in this workspace, waiting for the person's review (phase I3). Never written
    /// without an accept; cleared when another folder opens.
    pending: Mutex<p2p_ide_core::agent::Pending>,
    /// True while a push, collect or pull with the gateway project runs (phase I4; one at a time).
    /// Shared with the run commands (phase I5b), which push the folder first; an Arc so a run the assistant asked
    /// for can hold it from its worker thread.
    sync_busy: Arc<AtomicBool>,
    /// True while a git stage, unstage or commit runs (phase I5; one at a time).
    scm_busy: AtomicBool,
    /// The run the assistant is waiting on (phase I5b): its id and where the person's Allow or Decline goes. Only
    /// `agent_decide` with this id answers it.
    run_decision: Arc<Mutex<Option<WaitingRun>>>,
    /// Phase I6a: one extension install at a time.
    ext_busy: AtomicBool,
}

/// What the webview learns about the open workspace: its name only.
#[derive(Serialize)]
struct WorkspaceInfo {
    name: String,
}

/// Writes one audit line to stderr: `<unix seconds> p2p-ide <event> <kind>`.
///
/// Security: `event` and `kind` are fixed strings from this file, never user input, so a line cannot be forged
/// (CWE-117); no paths or contents are logged (CWE-532).
pub(crate) fn audit(event: &'static str, kind: &'static str) {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    eprintln!("{t} p2p-ide {event} {kind}");
}

/// Converts a core error to the message the webview shows, logging refusals that may be an attack.
fn refuse(e: WsError) -> String {
    match e {
        WsError::BadPath => audit("refused", "bad_path"),
        WsError::OutsideWorkspace => audit("refused", "outside_workspace"),
        _ => {}
    }
    e.message().to_string()
}

/// Runs `f` with the open workspace, or fails with "open a folder first".
fn with_ws<T>(state: &State<AppState>, f: impl FnOnce(&Workspace) -> Result<T, WsError>) -> Result<T, String> {
    let guard = state.workspace.lock().map_err(|_| "internal error".to_string())?;
    let ws = guard.as_ref().ok_or_else(|| "open a folder first".to_string())?;
    f(ws).map_err(refuse)
}

/// Shows the OS folder picker and opens the chosen folder as the workspace.
///
/// Returns the folder's name, or `None` when the person cancels.
///
/// Security: the path comes from the OS dialog, not from the webview; Workspace::open checks it (absolute, existing,
/// not a drive root).
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Option<WorkspaceInfo>, String> {
    // blocking_pick_folder must not run on the main thread; async commands run on Tauri's worker pool.
    let Some(picked) = app.dialog().file().set_title("Open folder").blocking_pick_folder() else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| WsError::BadRoot.message().to_string())?;
    let ws = Workspace::open(&path).map_err(refuse)?;
    // SECURITY: the Node extension host serves one folder; opening another ends it (phase I6c, CWE-829).
    nodehost::stop_all();
    let info = WorkspaceInfo { name: ws.name() };
    *state.workspace.lock().map_err(|_| "internal error".to_string())? = Some(ws);
    // SECURITY: pending agent changes belong to the folder they were proposed in; never carry them over.
    *state.pending.lock().map_err(|_| "internal error".to_string())? = Default::default();
    audit("workspace", "opened");
    Ok(Some(info))
}

/// The open workspace's name, or `None`.
#[tauri::command]
fn workspace_info(state: State<AppState>) -> Result<Option<WorkspaceInfo>, String> {
    let guard = state.workspace.lock().map_err(|_| "internal error".to_string())?;
    Ok(guard.as_ref().map(|w| WorkspaceInfo { name: w.name() }))
}

/// Lists a folder of the workspace (`rel` = "" for the root).
#[tauri::command]
fn list_dir(state: State<AppState>, rel: String) -> Result<Listing, String> {
    with_ws(&state, |ws| ws.list_dir(&rel))
}

/// Reads a text file for the editor.
#[tauri::command]
fn read_file(state: State<AppState>, rel: String) -> Result<FileText, String> {
    with_ws(&state, |ws| ws.read_text(&rel))
}

/// Saves a text file. `base_version` is the version the editor loaded, or null to create a new file.
#[tauri::command]
fn write_file(
    state: State<AppState>,
    rel: String,
    text: String,
    base_version: Option<String>,
) -> Result<String, String> {
    with_ws(&state, |ws| ws.write_text(&rel, &text, base_version.as_deref()))
}

/// Plain-text search across the workspace.
#[tauri::command]
fn search(state: State<AppState>, query: String) -> Result<SearchResult, String> {
    with_ws(&state, |ws| ws.search(&query))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            workspace: Mutex::new(None),
            chat_busy: AtomicBool::new(false),
            chat_stop: Arc::new(AtomicBool::new(false)),
            public: Mutex::new(Vec::new()),
            pending: Mutex::new(Default::default()),
            sync_busy: Arc::new(AtomicBool::new(false)),
            run_decision: Arc::new(Mutex::new(None)),
            scm_busy: AtomicBool::new(false),
            ext_busy: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            workspace_info,
            list_dir,
            read_file,
            write_file,
            search,
            net::gateways_list,
            net::gateway_add,
            net::gateway_add_local,
            net::gateway_pick_ca,
            net::gateway_clear_ca,
            net::gateway_set_key,
            net::gateway_remove,
            net::gateway_set_active,
            net::gateway_check,
            net::tandemforge_status,
            net::tandemforge_connect,
            net::chat_send,
            net::chat_stop,
            net::chats_list,
            net::chat_load,
            net::chat_save,
            net::chat_delete,
            net::chats_search,
            net::directories_list,
            net::directories_set,
            net::public_list,
            net::gateway_add_public,
            agent::agent_send,
            agent::changes_list,
            agent::change_get,
            agent::change_accept,
            agent::change_reject,
            agent::agent_decide,
            run::run_start,
            run::runners_list,
            run::container_images,
            run::container_prepare,
            scm::scm_status,
            scm::scm_diff,
            scm::scm_stage,
            scm::scm_unstage,
            scm::scm_commit,
            scm::scm_log,
            missions::mission_project,
            missions::projects_list,
            missions::project_create,
            missions::project_link,
            missions::project_push,
            missions::project_collect,
            missions::project_pull,
            missions::switches_set,
            missions::missions_list,
            missions::mission_get,
            missions::mission_start,
            missions::mission_control,
            missions::approvals_list,
            missions::approval_decide,
            missions::proposals_list,
            missions::proposal_act,
            ext::ext_search,
            ext::ext_install,
            ext::ext_list,
            ext::ext_set_enabled,
            ext::ext_uninstall,
            ext::ext_manifest,
            ext::ext_file,
            ext::ext_set_network,
            ext::ext_fetch,
            nodehost::node_host_status,
            nodehost::node_host_trust,
            nodehost::node_host_start,
            nodehost::node_host_stop,
            nodehost::node_host_electron
        ])
        .build(tauri::generate_context!())
        .expect("the IDE window could not start")
        .run(|_, event| {
            // SECURITY: the Node extension host never outlives the app (phase I6c).
            if let tauri::RunEvent::Exit = event {
                nodehost::stop_all();
            }
        });
}
