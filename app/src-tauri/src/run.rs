//! run.rs -- the shell's command for the Run and test view (phase I5b, docs/features/run-and-test.md).
//!
//! Purpose: the person runs the project's tests (a framework from a fixed list) or one program on the open folder.
//! Phase I5b runs it on the active gateway's sandbox (p2p_ide_core::runner::gateway_run): the folder is sent to the
//! linked project, the node's copy is brought up to date, and the step runs on a snapshot with no network.
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
use crate::{audit, AppState};
use p2p_ide_core::runner::{self, RunInput, RunResult};
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

/// Runs tests or a command for the person.
///
/// * `runner` -- `gateway` (phase I5b); `local-os` and `container` come in I5c and I5d and are refused until then.
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
    // SECURITY: deny by default; only built runners are accepted.
    if runner != "gateway" {
        return Err("this runner is not available yet; use the gateway sandbox".into());
    }
    let (client, pid, ws) = linked(&app, &state)?;
    let req = runner::check(&ws, &input).map_err(str::to_string)?;
    let busy = RunBusy::take(&state.sync_busy)?;
    audit("run", "started");
    tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        runner::gateway_run(&client, &ws, &pid, &req, &mut |s| {
            let _ = on_stage.send(s.to_string());
        })
        .map_err(|e| e.message())
    })
    .await
    .map_err(|_| "internal error".to_string())?
}
