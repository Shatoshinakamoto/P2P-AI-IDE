//! nodehost.rs -- the shell's commands for the Node extension host (phase I6c-1,
//! docs/features/node-extensions-design.md).
//!
//! Purpose: lets the UI ask whether Node extensions can run here, trust or stop trusting the open folder, and start
//! or stop the local VSCodium server that runs Node extensions. The work is done by `p2p_ide_core::node_host`; this
//! file adds the app's data folder, the open folder, the blocking pool, one running server at most, and audit lines.
//!
//! Trust boundaries: the webview sends only booleans. The folder is always the one the person opened through the OS
//! dialog (held in `AppState`), never a path from the webview (CWE-22).
//!
//! Security controls:
//!   * Workspace trust is changed only by `node_host_trust`, which the UI calls from the person's click; the core
//!     checks it again on every start (CWE-829, NIST AC-3).
//!   * The download happens only when the UI passes `allow_download` after the person agreed to it; the core checks
//!     the pinned SHA-256 (CWE-494).
//!   * At most one server runs; it is stopped when another folder is opened, when trust is taken back, and when the
//!     app exits (NIST AC-12 style session end).
//!   * The connection token goes to the webview only (it connects to the server); it is never logged (CWE-532).
//!   * Audit lines with fixed words only (NIST AU-2/AU-3, CWE-117).
//!
//! Dependencies: tauri, p2p-ide-core.

use crate::{audit, AppState};
use p2p_ide_core::extensions::ExtStore;
use p2p_ide_core::node_host::{self, HostError, Launcher, Running, TrustStore};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// The running server, if any (one per app).
static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// What the UI learns about the Node extension host.
#[derive(Serialize)]
pub struct HostStatus {
    /// A server is pinned for this OS and CPU (otherwise Node extensions can't run here yet).
    pinned: bool,
    /// The pinned server is downloaded and unpacked.
    installed: bool,
    /// The open folder is trusted (false when no folder is open).
    trusted: bool,
    /// The port on 127.0.0.1 while a server runs.
    port: Option<u16>,
}

/// Where the server connects: SECURITY: the token is for the webview only, never logged (CWE-532).
#[derive(Serialize)]
pub struct HostConnection {
    port: u16,
    token: String,
    /// The open folder's absolute path, which the editor needs for the server's `vscode-remote://` workspace.
    /// SECURITY-WARNING: elsewhere the webview only learns the folder's name; the full path is shared only here,
    /// once the person trusted the folder for Node extensions, which see it anyway (CWE-200, accepted).
    folder: String,
    /// The server's VS Code commit, which the editor announces so the server accepts it.
    commit: String,
}

fn pinned_commit() -> String {
    node_host::pin_for(node_host::PINS, std::env::consts::OS, std::env::consts::ARCH)
        .map(|p| p.commit.to_string())
        .unwrap_or_default()
}

fn folder(state: &State<AppState>) -> Result<Option<std::path::PathBuf>, String> {
    let guard = state.workspace.lock().map_err(|_| "internal error".to_string())?;
    Ok(guard.as_ref().map(|w| w.root().to_path_buf()))
}

fn word(e: &HostError) -> &'static str {
    match e {
        HostError::NotPinned => "not_pinned",
        HostError::Network => "network",
        HostError::TooLarge => "too_large",
        HostError::HashMismatch => "hash_mismatch",
        HostError::BadArchive => "bad_archive",
        HostError::NotTrusted => "not_trusted",
        HostError::NotInstalled => "not_installed",
        HostError::StartFailed => "start_failed",
        HostError::BadInput => "bad_input",
        HostError::Io => "io",
        HostError::FolderNotLocal => "folder_not_local",
    }
}

/// Stops the server if one runs. Called when another folder opens, trust is taken back, and on exit.
pub fn stop_all() {
    if let Ok(mut g) = RUNNING.lock() {
        if let Some(r) = g.take() {
            r.stop();
            audit("node_host", "stopped");
        }
    }
}

/// The host's state for the open folder.
#[tauri::command]
pub fn node_host_status(app: AppHandle, state: State<AppState>) -> Result<HostStatus, String> {
    let data = crate::net::data_dir(&app)?;
    let pin = node_host::pin_for(node_host::PINS, std::env::consts::OS, std::env::consts::ARCH);
    let installed = pin.as_ref().is_some_and(|p| node_host::server_root(&node_host::install_dir(&data, p)).is_some());
    let trusted = folder(&state)?.is_some_and(|f| TrustStore::new(&data).is_trusted(&f));
    let mut g = RUNNING.lock().map_err(|_| "internal error".to_string())?;
    let port = g.as_mut().and_then(|r| if r.alive() { Some(r.port) } else { None });
    Ok(HostStatus { pinned: pin.is_some(), installed, trusted, port })
}

/// Trusts or stops trusting the open folder. Only from the person's click in the trust prompt or the view.
#[tauri::command]
pub fn node_host_trust(app: AppHandle, state: State<AppState>, trusted: bool) -> Result<(), String> {
    let data = crate::net::data_dir(&app)?;
    let f = folder(&state)?.ok_or_else(|| "open a folder first".to_string())?;
    TrustStore::new(&data).set(&f, trusted).map_err(|e| e.message().to_string())?;
    audit("node_host", if trusted { "folder_trusted" } else { "folder_untrusted" });
    if !trusted {
        // SECURITY: taking trust back ends the host at once (CWE-613 style: no session outlives its grant).
        stop_all();
    }
    Ok(())
}

/// Starts the host for the open, trusted folder (downloading it first only when `allow_download` is true) and
/// returns where to connect. A server that already runs for this folder is reused.
#[tauri::command]
pub async fn node_host_start(
    app: AppHandle,
    state: State<'_, AppState>,
    allow_download: bool,
) -> Result<HostConnection, String> {
    let data = crate::net::data_dir(&app)?;
    let f = folder(&state)?.ok_or_else(|| "open a folder first".to_string())?;
    {
        let mut g = RUNNING.lock().map_err(|_| "internal error".to_string())?;
        if let Some(r) = g.as_mut() {
            if r.folder == f && r.alive() {
                return Ok(HostConnection {
                    port: r.port,
                    token: r.token.clone(),
                    folder: node_host::editor_folder(&r.folder).map_err(|e| e.message().to_string())?,
                    commit: pinned_commit(),
                });
            }
        }
    }
    stop_all();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<Running, HostError> {
        let pin = node_host::pin_for(node_host::PINS, std::env::consts::OS, std::env::consts::ARCH)
            .ok_or(HostError::NotPinned)?;
        // The editor must be able to name the folder for the server; refused before anything is downloaded.
        node_host::editor_folder(&f)?;
        let trust = TrustStore::new(&data);
        // SECURITY: trust is checked before anything is downloaded, and again by start (CWE-829).
        if !trust.is_trusted(&f) {
            return Err(HostError::NotTrusted);
        }
        let root = match node_host::server_root(&node_host::install_dir(&data, &pin)) {
            Some(r) => r,
            None if allow_download => {
                audit("node_host", "download_started");
                node_host::ensure_installed(&node_host::real_get(), &data, &pin)?
            }
            None => return Err(HostError::NotInstalled),
        };
        // The server loads only the enabled Node extensions, copied fresh on every start.
        node_host::stage_extensions(&ExtStore::new(data.join("extensions")), &data.join("node-extensions"))?;
        node_host::start(&Launcher::for_root(&root), &data, &trust, &f)
    })
    .await
    .map_err(|_| "internal error".to_string())?;
    match result {
        Ok(r) => {
            audit("node_host", "started");
            let folder = match node_host::editor_folder(&r.folder) {
                Ok(f) => f,
                Err(e) => {
                    r.stop();
                    return Err(e.message().to_string());
                }
            };
            let c = HostConnection { port: r.port, token: r.token.clone(), folder, commit: pinned_commit() };
            *RUNNING.lock().map_err(|_| "internal error".to_string())? = Some(r);
            Ok(c)
        }
        Err(e) => {
            audit("node_host_refused", word(&e));
            Err(e.message().to_string())
        }
    }
}

/// Stops the host.
#[tauri::command]
pub fn node_host_stop() {
    stop_all();
}
