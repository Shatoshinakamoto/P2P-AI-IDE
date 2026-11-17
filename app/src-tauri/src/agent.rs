//! agent.rs -- the shell's commands for Agent chat with tools and for reviewing the changes it proposes (phase I3,
//! docs/features/agent-tools.md).
//!
//! Purpose: runs one agent turn on the active gateway against the open folder (p2p_ide_core::agent does the work),
//! streams text and tool steps to the webview, and lets the person list, view, accept or reject the pending changes.
//!
//! Trust boundaries: the conversation and every path come from the webview (checked in the core); tool calls are
//! model output (checked in the core); answers and file text go to the webview as plain strings (rendered as text or
//! in Monaco's read-only diff view, CWE-79).
//!
//! Security controls:
//!   * Nothing is written without `change_accept`, which the person triggers per file; it uses the same atomic,
//!     version-checked save as the editor (CWE-367, OWASP LLM06, NIST AC-6).
//!   * One turn at a time, shared with plain chat; Stop ends it between pieces of text or tool steps (CWE-400).
//!   * The workspace is copied for the turn and the pending list is taken out and put back, so no lock is held
//!     while the network call runs; opening another folder clears the pending list (main.rs).
//!   * Audit lines with fixed words only, never paths, tool names or text: agent turn, change accepted, change
//!     rejected (NIST AU-3, CWE-117, CWE-532).
//!
//! Dependencies: tauri, p2p-ide-core.

use crate::net::active_client;
use crate::{audit, AppState};
use p2p_ide_core::agent::{self, AgentEvent, ChangeView, TurnEnd};
use p2p_ide_core::client::{Msg, NetError};
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

fn workspace(state: &State<AppState>) -> Result<p2p_ide_core::workspace::Workspace, String> {
    state
        .workspace
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or_else(|| "open a folder first; the assistant works on the open folder".to_string())
}

/// Runs one agent turn: the conversation (user and assistant messages) with the folder tools. Text and tool steps
/// arrive through `on_event`; proposed edits wait in the pending list.
#[tauri::command]
pub async fn agent_send(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
    messages: Vec<Msg>,
    on_event: Channel<AgentEvent>,
) -> Result<TurnEnd, String> {
    let ws = workspace(&state)?;
    let client = active_client(&app)?;
    // SECURITY: one turn at a time per window, shared with plain chat (CWE-400).
    if state.chat_busy.swap(true, Ordering::SeqCst) {
        return Err("an answer is still coming in; stop it first".into());
    }
    state.chat_stop.store(false, Ordering::SeqCst);
    let mut pending = match state.pending.lock() {
        Ok(mut p) => std::mem::take(&mut *p),
        Err(_) => {
            state.chat_busy.store(false, Ordering::SeqCst);
            return Err("internal error".into());
        }
    };
    audit("agent", "turn");
    let stop = state.chat_stop.clone();
    let tools = agent::tool_defs();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut step =
            |m: &[serde_json::Value], on: &mut dyn FnMut(&str) -> bool| client.agent_step(&model, m, &tools, on);
        let r = agent::run_turn(&ws, &mut pending, &messages, &mut step, &mut |e| {
            !stop.load(Ordering::SeqCst) && on_event.send(e).is_ok()
        });
        (r, pending)
    })
    .await;
    let result = match joined {
        Ok((r, pending)) => {
            if let Ok(mut p) = state.pending.lock() {
                *p = pending;
            }
            r
        }
        Err(_) => Err(NetError::Upstream),
    };
    state.chat_busy.store(false, Ordering::SeqCst);
    match result {
        Ok(end) => Ok(end),
        Err(NetError::Stopped) => Err("stopped".into()),
        Err(NetError::NotOffered) => {
            Err("this gateway or model does not offer tools; turn Tools off to chat without them".into())
        }
        Err(e) => Err(e.message()),
    }
}

/// The pending changes: path, new file or not, lines added and removed.
#[tauri::command]
pub fn changes_list(state: State<AppState>) -> Result<Vec<ChangeView>, String> {
    Ok(state.pending.lock().map_err(|_| "internal error".to_string())?.views())
}

/// One pending change's text before and after, for the diff view.
#[derive(Serialize)]
pub struct ChangeText {
    rel: String,
    before: String,
    after: String,
    created: bool,
}

#[tauri::command]
pub fn change_get(state: State<AppState>, rel: String) -> Result<ChangeText, String> {
    let p = state.pending.lock().map_err(|_| "internal error".to_string())?;
    let f = p.files.get(&rel).ok_or("that change is no longer pending")?;
    Ok(ChangeText { rel, before: f.before.clone(), after: f.after.clone(), created: f.base_version.is_none() })
}

/// Writes one pending change (the person pressed Accept). Returns the file's new version.
#[tauri::command]
pub fn change_accept(state: State<AppState>, rel: String) -> Result<String, String> {
    let ws = workspace(&state)?;
    let mut p = state.pending.lock().map_err(|_| "internal error".to_string())?;
    match p.accept(&ws, &rel) {
        Ok(v) => {
            audit("change", "accepted");
            Ok(v)
        }
        Err(p2p_ide_core::workspace::WsError::Conflict) => {
            Err("the file changed on disk after the assistant read it; reject this change and ask again".into())
        }
        Err(e) => Err(e.message().to_string()),
    }
}

/// Drops one pending change (the person pressed Reject).
#[tauri::command]
pub fn change_reject(state: State<AppState>, rel: String) -> Result<(), String> {
    if state.pending.lock().map_err(|_| "internal error".to_string())?.reject(&rel) {
        audit("change", "rejected");
    }
    Ok(())
}
