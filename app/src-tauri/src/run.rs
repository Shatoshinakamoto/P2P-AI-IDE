//! run.rs -- the shell's commands for the Run and test view (phases I5b and I5c, docs/features/run-and-test.md).
//!
//! Purpose: the person runs the project's tests (a framework from a fixed list) or one program on the open folder,
//! on a runner they pick: the active gateway's sandbox (I5b, p2p_ide_core::runner::gateway_run: the folder is sent
//! to the linked project and the step runs on the node's copy) or the local OS sandbox (I5c,
//! p2p_ide_core::localbox::run: a copy of the folder on this computer, confined by namespaces, Landlock and seccomp
//! on Linux). Both run on a snapshot with no network, and the result has the same shape.
//!
//! Trust boundaries: the request comes from the webview (checked by `runner::check`); the gateway's answer is
//! untrusted text, cleaned and capped in the core and rendered by React as text (CWE-79, CWE-150).
//!
//! Security controls:
//!   * The person's click is the intent; no shell, an argument list or a framework name only (CWE-78).
//!   * Shares the one-at-a-time flag with push, collect and pull, so two uploads never race (CWE-400, CWE-362).
//!   * The runner is chosen here from a fixed list; runners not built yet are refused, never guessed (deny by
//!     default).
//!   * Audit lines with fixed words only, never commands or output: run started (NIST AU-3, CWE-117, CWE-532).
//!
//! Dependencies: tauri, p2p-ide-core.

use crate::missions::linked;
use crate::net::data_dir;
use crate::{audit, AppState};
use p2p_ide_core::localbox;
use p2p_ide_core::runner::{self, RunInput, RunResult};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

/// Holds the shared push-and-run flag until dropped (works from a worker thread too).
pub(crate) struct RunBusy(std::sync::Arc<AtomicBool>);

impl RunBusy {
    /// Takes the flag, or says that another send, pull or run is going on.
    pub(crate) fn take(flag: &std::sync::Arc<AtomicBool>) -> Result<RunBusy, String> {
        if flag.swap(true, Ordering::SeqCst) {
            return Err("a send, pull or run is still going on; wait for it to end".into());
        }
        Ok(RunBusy(flag.clone()))
    }
}

impl Drop for RunBusy {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// One runner the view can offer.
#[derive(Serialize)]
pub struct RunnerView {
    /// `gateway`, `local-os` or `container`.
    pub id: &'static str,
    pub available: bool,
    /// Fixed words when not available.
    pub reason: String,
}

/// Which runners can be used for the open folder now.
#[tauri::command]
pub async fn runners_list(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<RunnerView>, String> {
    let gateway = linked(&app, &state).map(|_| ()).map_err(|e| e.to_string());
    let local =
        tauri::async_runtime::spawn_blocking(localbox::available).await.map_err(|_| "internal error".to_string())?;
    Ok(vec![
        RunnerView { id: "gateway", available: gateway.is_ok(), reason: gateway.err().unwrap_or_default() },
        RunnerView { id: localbox::RUNNER, available: local.available, reason: local.reason },
        RunnerView { id: "container", available: false, reason: "coming next (Docker or Podman)".into() },
    ])
}

/// A runner ready to use: takes a checked request and a stage callback, gives the result or a fixed message.
pub(crate) type RunFnBox =
    Box<dyn FnMut(&runner::RunRequest, &mut dyn FnMut(&'static str)) -> Result<RunResult, String> + Send>;

/// Runs a checked request on `runner` (`gateway` or `local-os`); `stage` gets each stage's word. Blocking: call it
/// from a worker thread. Used by the view and by the chat's approved runs.
pub(crate) fn run_on(app: &AppHandle, state: &State<'_, AppState>, runner: &str) -> Result<RunFnBox, String> {
    match runner {
        "gateway" => {
            let (client, pid, ws) = linked(app, state)?;
            Ok(Box::new(move |req, stage| runner::gateway_run(&client, &ws, &pid, req, stage).map_err(|e| e.message())))
        }
        localbox::RUNNER => {
            let ws = state
                .workspace
                .lock()
                .map_err(|_| "internal error".to_string())?
                .clone()
                .ok_or("open a folder first")?;
            let dir = data_dir(app)?;
            Ok(Box::new(move |req, stage| localbox::run(&ws, req, &dir, stage)))
        }
        // SECURITY: deny by default; only built runners are accepted (NIST CM-7).
        _ => Err("this runner is not available yet".into()),
    }
}

/// Runs tests or a command for the person.
///
/// * `runner` -- `gateway` (I5b) or `local-os` (I5c); `container` comes in I5d and is refused until then.
/// * `input` -- what to run (see `runner::RunInput`).
/// * `on_stage` -- gets `sending`, `copying`, `running` as each stage starts.
///
/// Returns the cleaned result; errors are fixed messages.
#[tauri::command]
pub async fn run_start(
    app: AppHandle,
    state: State<'_, AppState>,
    runner: String,
    input: RunInput,
    on_stage: Channel<String>,
) -> Result<RunResult, String> {
    let ws = state.workspace.lock().map_err(|_| "internal error".to_string())?.clone().ok_or("open a folder first")?;
    let req = runner::check(&ws, &input).map_err(str::to_string)?;
    let mut run = run_on(&app, &state, &runner)?;
    let busy = RunBusy::take(&state.sync_busy)?;
    audit("run", "started");
    tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        run(&req, &mut |s| {
            let _ = on_stage.send(s.to_string());
        })
    })
    .await
    .map_err(|_| "internal error".to_string())?
}
