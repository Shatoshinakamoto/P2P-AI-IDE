//! net.rs -- the shell's gateway and chat commands (phase I2, docs/features/gateways-and-chat.md).
//!
//! Purpose: lets the webview manage saved gateways, check one (latency and models), stream a chat answer from the
//! active gateway and keep chat history, all through typed commands. The work is done by p2p-ide-core
//! (`gateways`, `client`, `chats`); this file adds the OS keychain, the OS file pickers and the async plumbing.
//!
//! Trust boundaries: every argument is webview input and is checked by the core. Gateway answers are untrusted text,
//! passed to the webview as plain strings over a Tauri channel (the webview renders them as text, CWE-79).
//!
//! Security controls:
//!   * Keys: written to the OS keychain (Windows Credential Manager, macOS Keychain, Secret Service on Linux) via the
//!     `keyring` crate, read only in Rust to build the Authorization header, and never returned, logged or
//!     included in errors (CWE-522, CWE-532, NIST IA-5). The webview only sees `has_key`.
//!   * CA files and local gateway setup folders are chosen in the OS dialog, not typed by the webview (CWE-73); the
//!     core checks them and never reads the setup folder's `secrets/` (NIST AC-6).
//!   * One chat turn at a time; Stop cancels it (CWE-400). Network calls run on the blocking pool, not the UI thread.
//!   * Public gateway list (phase I2b): adverts are fetched and verified in the core; the address of a picked public
//!     gateway is taken from the cached verified list, never from the webview (CWE-345), and such a gateway is only
//!     ever reached at a public address (CWE-918).
//!   * Audit lines (stderr, fixed words only): gateway added, removed, key set, chat sent, public list fetched,
//!     refusals (NIST AU-3, CWE-117).
//!
//! Dependencies: keyring 4 (OS keychain, default "v1" stores), tauri, tauri-plugin-dialog, p2p-ide-core.

use crate::{audit, AppState};
use p2p_ide_core::chats::{scope_for, Chat, ChatError, ChatHit, ChatStore, ChatSummary};
use p2p_ide_core::client::{Client, Msg, NetError, StreamEnd};
use p2p_ide_core::discovery::{self, DirError, DirectoryStore, PublicList};
use p2p_ide_core::gateways::{self, GatewayStore, GatewayView, GwError, Kind, SecretStore, KEY_SERVICE};
use p2p_ide_core::store;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// The OS keychain as a [`SecretStore`]. Account name = gateway id (16 hex), service = [`KEY_SERVICE`].
pub struct Keychain;

impl Keychain {
    fn entry(id: &str) -> Result<keyring::Entry, GwError> {
        // SECURITY: ids are allow-listed before they name a keychain entry (CWE-20).
        if !store::id_ok(id) {
            return Err(GwError::NotFound);
        }
        keyring::Entry::new(KEY_SERVICE, id).map_err(|_| GwError::Keychain)
    }
}

impl SecretStore for Keychain {
    fn get(&self, id: &str) -> Result<Option<String>, GwError> {
        match Self::entry(id)?.get_password() {
            Ok(k) => Ok(Some(k)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(GwError::Keychain),
        }
    }
    fn set(&self, id: &str, secret: &str) -> Result<(), GwError> {
        Self::entry(id)?.set_password(secret).map_err(|_| GwError::Keychain)
    }
    fn delete(&self, id: &str) -> Result<(), GwError> {
        match Self::entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(GwError::Keychain),
        }
    }
}

/// The app's data folder (per user, from the OS). SECURITY: never from the webview.
pub(crate) fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|_| "the app's data folder is not available".to_string())
}

fn gw_store(app: &AppHandle) -> Result<GatewayStore, String> {
    Ok(GatewayStore::new(&data_dir(app)?))
}

fn gw_err(e: GwError) -> String {
    if matches!(e, GwError::BadUrl | GwError::BadCa | GwError::BadKey) {
        audit("refused", "gateway_input");
    }
    e.message().to_string()
}

/// The saved gateways and the active one.
#[derive(Serialize)]
pub struct GatewayListView {
    gateways: Vec<GatewayView>,
    active: Option<String>,
}

#[tauri::command]
pub fn gateways_list(app: AppHandle) -> Result<GatewayListView, String> {
    let (gateways, active) = gw_store(&app)?.views(&Keychain).map_err(gw_err)?;
    Ok(GatewayListView { gateways, active })
}

/// Adds a manual gateway: name, URL and optionally a key (empty = add the key later).
#[tauri::command]
pub fn gateway_add(app: AppHandle, name: String, url: String, key: Option<String>) -> Result<String, String> {
    let id = gw_store(&app)?.add(&Keychain, &name, &url, Kind::Manual, None, key.as_deref()).map_err(gw_err)?;
    audit("gateway", "added_manual");
    Ok(id)
}

/// Adds the gateway set up on this computer: the person picks its setup folder in the OS dialog; the port and CA
/// come from there. Returns `None` when the dialog is cancelled.
#[tauri::command]
pub async fn gateway_add_local(app: AppHandle, name: String, key: Option<String>) -> Result<Option<String>, String> {
    let Some(picked) = app.dialog().file().set_title("Choose the gateway's setup folder").blocking_pick_folder() else {
        return Ok(None);
    };
    let dir = picked.into_path().map_err(|_| GwError::BadSetup.message().to_string())?;
    let (url, ca) = gateways::read_setup(&dir).map_err(gw_err)?;
    let name = if name.trim().is_empty() { "This computer".to_string() } else { name };
    let id = gw_store(&app)?.add(&Keychain, &name, &url, Kind::Local, Some(&ca), key.as_deref()).map_err(gw_err)?;
    audit("gateway", "added_local");
    Ok(Some(id))
}

/// Pins a CA file for a gateway, chosen in the OS file dialog. Returns `false` when cancelled.
#[tauri::command]
pub async fn gateway_pick_ca(app: AppHandle, id: String) -> Result<bool, String> {
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Choose the gateway's CA certificate")
        .add_filter("PEM certificate", &["pem", "crt", "cer"])
        .blocking_pick_file()
    else {
        return Ok(false);
    };
    let path = picked.into_path().map_err(|_| GwError::BadCa.message().to_string())?;
    let bytes = store::read_capped(&path, gateways::MAX_CA_BYTES as u64).map_err(|_| gw_err(GwError::BadCa))?;
    let pem = String::from_utf8(bytes).map_err(|_| gw_err(GwError::BadCa))?;
    gw_store(&app)?.set_ca(&id, Some(&pem)).map_err(gw_err)?;
    audit("gateway", "ca_pinned");
    Ok(true)
}

/// Removes a gateway's pinned CA (it then uses the public web roots).
#[tauri::command]
pub fn gateway_clear_ca(app: AppHandle, id: String) -> Result<(), String> {
    gw_store(&app)?.set_ca(&id, None).map_err(gw_err)
}

/// Stores a new key in the OS keychain.
#[tauri::command]
pub fn gateway_set_key(app: AppHandle, id: String, key: String) -> Result<(), String> {
    gw_store(&app)?.set_key(&Keychain, &id, &key).map_err(gw_err)?;
    audit("gateway", "key_set");
    Ok(())
}

#[tauri::command]
pub fn gateway_remove(app: AppHandle, id: String) -> Result<(), String> {
    gw_store(&app)?.remove(&Keychain, &id).map_err(gw_err)?;
    audit("gateway", "removed");
    Ok(())
}

#[tauri::command]
pub fn gateway_set_active(app: AppHandle, id: String) -> Result<(), String> {
    gw_store(&app)?.set_active(&id).map_err(gw_err)
}

/// Result of checking a gateway.
#[derive(Serialize)]
pub struct GatewayCheck {
    latency_ms: u64,
    models: Vec<String>,
}

fn client_for(app: &AppHandle, id: &str) -> Result<(Client, String), String> {
    let gw = gw_store(app)?.get(id).map_err(gw_err)?;
    let key = Keychain.get(id).map_err(gw_err)?;
    let name = gw.name.clone();
    Ok((Client::new(&gw, key).map_err(|e| e.message())?, name))
}

/// A client for the active gateway (used by chat and by the agent turn in agent.rs).
pub(crate) fn active_client(app: &AppHandle) -> Result<Client, String> {
    Ok(active_gateway(app)?.0)
}

/// The active gateway's client, id and name (phase I4: the missions view links a folder per gateway).
pub(crate) fn active_gateway(app: &AppHandle) -> Result<(Client, String, String), String> {
    let active = gw_store(app)?.load().map_err(gw_err)?.active.ok_or("add a gateway in the Gateways view first")?;
    let (c, name) = client_for(app, &active)?;
    Ok((c, active, name))
}

/// Asks a gateway for its models and times the answer.
#[tauri::command]
pub async fn gateway_check(app: AppHandle, id: String) -> Result<GatewayCheck, String> {
    let (client, _) = client_for(&app, &id)?;
    let r = tauri::async_runtime::spawn_blocking(move || client.models())
        .await
        .map_err(|_| "internal error".to_string())?;
    let (models, latency_ms) = r.map_err(|e| e.message())?;
    Ok(GatewayCheck { latency_ms, models })
}

/// Events streamed to the chat panel during one turn.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ChatEvent {
    /// More answer text (plain text; the webview never renders it as HTML).
    Delta { text: String },
}

/// Sends the conversation to the active gateway and streams the answer through `on_event`.
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
    messages: Vec<Msg>,
    on_event: Channel<ChatEvent>,
) -> Result<StreamEnd, String> {
    let active = gw_store(&app)?.load().map_err(gw_err)?.active.ok_or("add a gateway in the Gateways view first")?;
    let (client, _) = client_for(&app, &active)?;
    // SECURITY: one turn at a time per window (CWE-400).
    if state.chat_busy.swap(true, Ordering::SeqCst) {
        return Err("a chat answer is still coming in; stop it first".into());
    }
    state.chat_stop.store(false, Ordering::SeqCst);
    audit("chat", "sent");
    let stop = state.chat_stop.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        client.chat_stream(&model, &messages, &mut |t| {
            if stop.load(Ordering::SeqCst) {
                return false;
            }
            on_event.send(ChatEvent::Delta { text: t.to_string() }).is_ok()
        })
    })
    .await;
    state.chat_busy.store(false, Ordering::SeqCst);
    match result {
        Ok(Ok(end)) => Ok(end),
        Ok(Err(NetError::Stopped)) => Err("stopped".into()),
        Ok(Err(e)) => Err(e.message()),
        Err(_) => Err("internal error".into()),
    }
}

/// Stops the answer that is coming in.
#[tauri::command]
pub fn chat_stop(state: State<AppState>) {
    state.chat_stop.store(true, Ordering::SeqCst);
}

/// The chat history for the open folder (or for "no folder").
fn chat_store(app: &AppHandle, state: &State<AppState>) -> Result<ChatStore, String> {
    let scope = {
        let ws = state.workspace.lock().map_err(|_| "internal error".to_string())?;
        scope_for(ws.as_ref().map(|w| w.root()))
    };
    ChatStore::new(&data_dir(app)?, &scope).map_err(|e| e.message().to_string())
}

fn chat_err(e: ChatError) -> String {
    e.message().to_string()
}

#[tauri::command]
pub fn chats_list(app: AppHandle, state: State<AppState>) -> Result<Vec<ChatSummary>, String> {
    Ok(chat_store(&app, &state)?.list())
}

#[tauri::command]
pub fn chat_load(app: AppHandle, state: State<AppState>, id: String) -> Result<Chat, String> {
    chat_store(&app, &state)?.load(&id).map_err(chat_err)
}

/// Saves a chat. A chat without an id gets a new one.
#[tauri::command]
pub fn chat_save(app: AppHandle, state: State<AppState>, mut chat: Chat) -> Result<Chat, String> {
    if chat.id.is_empty() {
        chat.id = store::new_id();
    }
    chat_store(&app, &state)?.save(chat).map_err(chat_err)
}

#[tauri::command]
pub fn chat_delete(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    chat_store(&app, &state)?.delete(&id).map_err(chat_err)
}

#[tauri::command]
pub fn chats_search(app: AppHandle, state: State<AppState>, query: String) -> Result<Vec<ChatHit>, String> {
    chat_store(&app, &state)?.search(&query).map_err(chat_err)
}

// ------------------------------------------------------------------ public gateway list (phase I2b)

fn dir_err(e: DirError) -> String {
    if e == DirError::BadUrl {
        audit("refused", "directory_input");
    }
    e.message().to_string()
}

/// The directories the person chose for the public list (empty until they add one).
#[tauri::command]
pub fn directories_list(app: AppHandle) -> Result<Vec<String>, String> {
    DirectoryStore::new(&data_dir(&app)?).load().map_err(dir_err)
}

/// Replaces the directory list (each entry checked in the core). Returns the saved list.
#[tauri::command]
pub fn directories_set(app: AppHandle, directories: Vec<String>) -> Result<Vec<String>, String> {
    if directories.len() > 64 {
        return Err(dir_err(DirError::TooMany));
    }
    let saved = DirectoryStore::new(&data_dir(&app)?).set(&directories).map_err(dir_err)?;
    audit("directories", "set");
    Ok(saved)
}

/// Fetches and verifies the public list from the saved directories, probes each gateway and caches the result for
/// `gateway_add_public`. Sends no key anywhere.
#[tauri::command]
pub async fn public_list(app: AppHandle, state: State<'_, AppState>) -> Result<PublicList, String> {
    let dirs = DirectoryStore::new(&data_dir(&app)?).load().map_err(dir_err)?;
    if dirs.is_empty() {
        return Err("add a directory first".into());
    }
    let list = tauri::async_runtime::spawn_blocking(move || {
        let fetch = discovery::real_fetch();
        discovery::list_public(&dirs, store::now_s(), &fetch)
    })
    .await
    .map_err(|_| "internal error".to_string())?;
    audit("public_list", "fetched");
    *state.public.lock().map_err(|_| "internal error".to_string())? = list.gateways.clone();
    Ok(list)
}

/// Saves a gateway from the last public list by its advert id, optionally with a key. The address comes from the
/// verified advert in the cache. Returns the new gateway id.
#[tauri::command]
pub fn gateway_add_public(
    app: AppHandle,
    state: State<AppState>,
    gateway_id: String,
    key: Option<String>,
) -> Result<String, String> {
    let picked = {
        let cache = state.public.lock().map_err(|_| "internal error".to_string())?;
        cache.iter().find(|g| g.gateway_id == gateway_id).cloned()
    };
    let g = picked.ok_or("that gateway is not in the last list; refresh it")?;
    let url = discovery::api_origin(&g.access.api_url)
        .ok_or("this gateway's API address has a path, which the IDE cannot use yet")?;
    // SECURITY: the name is built from checked advert fields only (region is one short line, id is hex).
    let region: String = g.region.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(16).collect();
    let name = if region.is_empty() {
        format!("Public {}", &g.gateway_id[..8])
    } else {
        format!("Public {} ({region})", &g.gateway_id[..8])
    };
    let id = gw_store(&app)?.add(&Keychain, &name, &url, Kind::Public, None, key.as_deref()).map_err(gw_err)?;
    audit("gateway", "added_public");
    Ok(id)
}
