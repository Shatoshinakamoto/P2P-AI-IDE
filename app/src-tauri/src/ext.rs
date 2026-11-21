//! ext.rs -- the shell's Extensions commands (phases I6a-I6b, docs/features/extensions.md).
//!
//! Purpose: lets the Extensions view search Open VSX, install an extension the person picked, list, turn on or off and
//! remove installed ones, and read the files an installed extension contributes. The work is done by
//! `p2p_ide_core::extensions`; this file adds the app's data folder, the blocking pool and audit lines.
//!
//! Trust boundaries: every argument is webview input and is checked in the core (names, versions and file paths are
//! allow-listed there). Installs happen only from this command, which the webview calls when the person clicks Install.
//!
//! Security controls:
//!   * One install at a time (CWE-400); network work runs on the blocking pool, not the UI thread.
//!   * Extension files are returned as raw bytes only for installed, enabled extensions and only from inside their
//!     own folder (core: cap-std handle, CWE-22).
//!   * Audit lines with fixed words and the allow-listed extension id only (NIST AU-2/AU-3, CWE-117): install
//!     started, installed, refused (with the error kind), enabled, disabled, removed, internet allowed or stopped.
//!   * Phase I6b internet for web extensions (`ext_fetch`): only for an enabled web extension the person allowed;
//!     the request is checked and made by `p2p_ide_core::ext_net` (public https only); at most 8 at a time
//!     (CWE-400); each request is logged with the extension id and the host only, never the path or query (CWE-532).
//!
//! Dependencies: tauri, p2p-ide-core.

use crate::{audit, AppState};
use p2p_ide_core::ext_net::{self, NetAnswer, NetCall};
use p2p_ide_core::extensions::{self, ExtError, ExtStore, Installed, Kind, Summary};
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::ipc::Response;
use tauri::{AppHandle, State};

fn ext_store(app: &AppHandle) -> Result<ExtStore, String> {
    Ok(ExtStore::new(crate::net::data_dir(app)?.join("extensions")))
}

fn kind_word(e: &ExtError) -> &'static str {
    match e {
        ExtError::BadInput => "bad_input",
        ExtError::Network => "network",
        ExtError::TooLarge => "too_large",
        ExtError::BadPackage => "bad_package",
        ExtError::HashMismatch => "hash_mismatch",
        ExtError::BadSignature => "bad_signature",
        ExtError::UnsafePath => "unsafe_path",
        ExtError::NeedsHost(_) => "needs_host",
        ExtError::ProposedApi => "proposed_api",
        ExtError::NotFound => "not_found",
        ExtError::Io => "io",
    }
}

/// Searches Open VSX for `query`, from result `offset`.
#[tauri::command]
pub async fn ext_search(query: String, offset: usize) -> Result<Vec<Summary>, String> {
    tauri::async_runtime::spawn_blocking(move || extensions::search(&extensions::real_get(), &query, offset))
        .await
        .map_err(|_| "internal error".to_string())?
        .map_err(|e| e.message())
}

/// Downloads, verifies and installs `namespace.name` at `version` (or `latest`). Only on the person's click.
#[tauri::command]
pub async fn ext_install(
    app: AppHandle,
    state: State<'_, AppState>,
    namespace: String,
    name: String,
    version: String,
) -> Result<Installed, String> {
    // SECURITY: one install at a time (CWE-400).
    if state.ext_busy.swap(true, Ordering::SeqCst) {
        return Err("another extension is being installed; wait for it to finish".into());
    }
    let store = ext_store(&app);
    let result = match store {
        Err(e) => Err(e),
        Ok(store) => {
            audit("extension", "install_started");
            tauri::async_runtime::spawn_blocking(move || {
                store.install(&extensions::real_get(), &namespace, &name, &version)
            })
            .await
            .map_err(|_| "internal error".to_string())
            .and_then(|r| {
                r.map_err(|e| {
                    audit("extension_refused", kind_word(&e));
                    e.message()
                })
            })
        }
    };
    state.ext_busy.store(false, Ordering::SeqCst);
    if let Ok(e) = &result {
        audit("extension", "installed");
        eprintln!("p2p-ide extension id {}", e.id);
    }
    result
}

/// Every installed extension.
#[tauri::command]
pub fn ext_list(app: AppHandle) -> Result<Vec<Installed>, String> {
    ext_store(&app)?.list().map_err(|e| e.message())
}

/// Turns an installed extension on or off.
#[tauri::command]
pub fn ext_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<Installed, String> {
    let e = ext_store(&app)?.set_enabled(&id, enabled).map_err(|e| e.message())?;
    audit("extension", if enabled { "enabled" } else { "disabled" });
    Ok(e)
}

/// Removes an installed extension.
#[tauri::command]
pub fn ext_uninstall(app: AppHandle, id: String) -> Result<(), String> {
    ext_store(&app)?.uninstall(&id).map_err(|e| e.message())?;
    audit("extension", "removed");
    Ok(())
}

/// The `package.json` of an installed, enabled extension.
#[tauri::command]
pub fn ext_manifest(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    let store = ext_store(&app)?;
    enabled(&store, &id)?;
    store.manifest(&id).map_err(|e| e.message())
}

/// One file of an installed, enabled extension, as raw bytes.
#[tauri::command]
pub fn ext_file(app: AppHandle, id: String, path: String) -> Result<Response, String> {
    let store = ext_store(&app)?;
    enabled(&store, &id)?;
    Ok(Response::new(store.read_file(&id, &path).map_err(|e| e.message())?))
}

fn enabled(store: &ExtStore, id: &str) -> Result<(), String> {
    let list = store.list().map_err(|e| e.message())?;
    // SECURITY: a turned-off extension gives the editor nothing (deny by default).
    match list.iter().find(|e| e.id == id) {
        Some(e) if e.enabled => Ok(()),
        _ => Err(ExtError::NotFound.message()),
    }
}

/// Allows or stops internet for a web extension (phase I6b). Called only from the person's click on the consent
/// card or the Extensions view.
#[tauri::command]
pub fn ext_set_network(app: AppHandle, id: String, allowed: bool) -> Result<Installed, String> {
    let e = ext_store(&app)?.set_network(&id, allowed).map_err(|e| e.message())?;
    audit("extension", if allowed { "network_allowed" } else { "network_stopped" });
    eprintln!("p2p-ide extension network id {} allowed {allowed}", e.id);
    Ok(e)
}

/// Requests in flight for all extensions together.
static NET_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
const MAX_NET_IN_FLIGHT: usize = 8;

/// Makes one https request for an enabled web extension the person allowed (phase I6b).
///
/// Security: deny by default: the saved list must say enabled, kind web and internet allowed (NIST AC-3); the core
/// checks the URL (public https only), method, headers and sizes (CWE-918, CWE-113, CWE-400).
#[tauri::command]
pub async fn ext_fetch(app: AppHandle, id: String, request: NetCall) -> Result<NetAnswer, String> {
    let store = ext_store(&app)?;
    let list = store.list().map_err(|e| e.message())?;
    let Some(e) = list.iter().find(|e| e.id == id && e.enabled && e.kind == Kind::Web) else {
        return Err(ExtError::NotFound.message());
    };
    if !e.network {
        audit("extension_refused", "no_network");
        return Err("internet is not allowed for this extension".into());
    }
    let host = ext_net::public_https_host(&request.url).ok_or_else(|| ext_net::NetError::NotAllowed.message())?;
    // SECURITY: a bounded number of requests at once, so one extension can't tie up the app (CWE-400).
    if NET_IN_FLIGHT.fetch_add(1, Ordering::SeqCst) >= MAX_NET_IN_FLIGHT {
        NET_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        return Err("too many requests at once".into());
    }
    // SECURITY: id and host are allow-listed text; the path and query are never logged (CWE-117, CWE-532).
    eprintln!("p2p-ide extension network request id {} host {host}", e.id);
    let result = tauri::async_runtime::spawn_blocking(move || ext_net::run_call(request, &*ext_net::real_send()))
        .await
        .map_err(|_| "internal error".to_string());
    NET_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    result?.map_err(|e| e.message().to_string())
}
