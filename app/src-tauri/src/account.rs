//! account.rs -- the shell's "Sign in with a P2P-AI account" commands (universal accounts, ADR-030 phase A3;
//! docs/features/account-sign-in.md).
//!
//! Purpose: lets the webview sign in at an account service in the system browser, see who is signed in, change an
//! operator-made one-time password, sign out, and link a saved gateway to an account (instead of a key). The work
//! is done by `p2p_ide_core::accounts`; this file adds the OS keychain, the system browser and the async plumbing.
//!
//! Trust boundaries: every argument is webview input and is checked by the core. The sign-in link is built by the
//! core from a checked address; the browser and the loopback return are outside the app.
//!
//! Security controls:
//!   * Refresh tokens: OS keychain only ([`TokenKeychain`], a service of their own); access tokens: memory only in
//!     the core's cache. Neither is returned to the webview, logged, or put in errors (CWE-522, CWE-532, NIST IA-5).
//!     They are never passed to the Node extension host (node_host::ENV_ALLOW is untouched).
//!   * The system browser is started with the URL as one argument, never through a shell (CWE-78), and only for a
//!     link that starts with the checked home address and `/authorize?` and holds only URL-safe characters
//!     (RFC 8252 8.12: no embedded web view, so the app never sees the password).
//!   * One sign-in at a time; Cancel stops the wait; the core gives up after 5 minutes (CWE-400).
//!   * Passwords for "change password" go straight to the core and the home, and are dropped after the call.
//!   * Audit lines (stderr, fixed words only): signed in, sign-in failed, signed out, password changed, account
//!     linked or unlinked (NIST AU-2, AU-3, CWE-117).
//!
//! Dependencies: p2p-ide-core, tauri, keyring (through net.rs).

use crate::net::{data_dir, TokenKeychain};
use crate::{audit, AppState};
use p2p_ide_core::accounts::{check_home, AccountLink, Accounts, AcctError, HomeView};
use p2p_ide_core::gateways::{GatewayStore, GwError};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, State};

static ACCOUNTS: OnceLock<Arc<Accounts>> = OnceLock::new();

/// The one [`Accounts`] (remembered homes + token cache) for this app run.
fn accounts(app: &AppHandle) -> Result<Arc<Accounts>, String> {
    if let Some(a) = ACCOUNTS.get() {
        return Ok(a.clone());
    }
    let dir = data_dir(app)?;
    Ok(ACCOUNTS.get_or_init(|| Arc::new(Accounts::new(&dir))).clone())
}

fn acct_err(e: AcctError) -> String {
    e.message()
}

/// A gateway token for `link` (net.rs calls this when building a client for a linked gateway).
pub(crate) fn token_for(app: &AppHandle, link: &AccountLink) -> Result<String, String> {
    accounts(app)?.token_for(&TokenKeychain, link).map_err(acct_err)
}

/// Opens `url` in the person's default browser.
/// SECURITY: no shell: the URL is one argument to a fixed program (CWE-78). On Windows, `rundll32
/// url.dll,FileProtocolHandler` is used because `cmd /c start` would parse `&` in the link.
fn open_browser(url: &str) -> bool {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

/// The remembered account services and whether each is signed in.
#[tauri::command]
pub fn accounts_list(app: AppHandle) -> Result<Vec<HomeView>, String> {
    accounts(&app)?.views(&TokenKeychain).map_err(acct_err)
}

/// Signs in at `home` in the system browser; returns when the browser comes back (or after Cancel / 5 minutes).
#[tauri::command]
pub async fn account_sign_in(app: AppHandle, state: State<'_, AppState>, home: String) -> Result<HomeView, String> {
    let issuer = check_home(&home).map_err(acct_err)?;
    if state.acct_busy.swap(true, Ordering::SeqCst) {
        return Err("a sign-in is already waiting for the browser".into());
    }
    state.acct_cancel.store(false, Ordering::SeqCst);
    let acc = accounts(&app);
    let cancel = state.acct_cancel.clone();
    let r = match acc {
        Ok(acc) => tauri::async_runtime::spawn_blocking(move || {
            let open = |url: &str| -> bool {
                // SECURITY: only the home's own sign-in page, with a URL-safe query (the core percent-encodes it).
                url.starts_with(&format!("{issuer}/authorize?"))
                    && url.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&b))
                    && open_browser(url)
            };
            acc.sign_in(&TokenKeychain, &issuer, &open, &cancel).map_err(acct_err)
        })
        .await
        .unwrap_or_else(|_| Err("the sign-in stopped unexpectedly".into())),
        Err(e) => Err(e),
    };
    state.acct_busy.store(false, Ordering::SeqCst);
    audit("account", if r.is_ok() { "signed_in" } else { "sign_in_failed" });
    r
}

/// Stops a sign-in that is waiting for the browser.
#[tauri::command]
pub fn account_sign_in_cancel(state: State<AppState>) {
    state.acct_cancel.store(true, Ordering::SeqCst);
}

/// Signs this app out of an account service (the session there ends too).
#[tauri::command]
pub async fn account_sign_out(app: AppHandle, id: String) -> Result<(), String> {
    let acc = accounts(&app)?;
    let r = tauri::async_runtime::spawn_blocking(move || acc.sign_out(&TokenKeychain, &id).map_err(acct_err))
        .await
        .unwrap_or_else(|_| Err("sign-out stopped unexpectedly".into()));
    if r.is_ok() {
        audit("account", "signed_out");
    }
    r
}

/// Replaces an operator-made one-time password (needed before any gateway token).
#[tauri::command]
pub async fn account_change_password(app: AppHandle, id: String, current: String, new: String) -> Result<(), String> {
    let acc = accounts(&app)?;
    let r = tauri::async_runtime::spawn_blocking(move || {
        acc.change_password(&TokenKeychain, &id, &current, &new).map_err(acct_err)
    })
    .await
    .unwrap_or_else(|_| Err("the password change stopped unexpectedly".into()));
    audit("account", if r.is_ok() { "password_changed" } else { "password_change_failed" });
    r
}

/// Links a saved gateway to an account (`home` + the gateway's account id), or with empty `home`, back to its key.
#[tauri::command]
pub fn gateway_set_account(app: AppHandle, id: String, home: String, audience: String) -> Result<(), String> {
    let link = if home.trim().is_empty() { None } else { Some(AccountLink::new(&home, &audience).map_err(acct_err)?) };
    let linked = link.is_some();
    GatewayStore::new(&data_dir(&app)?).set_account(&id, link).map_err(|e| match e {
        GwError::BadUrl => "TandemForge on this computer signs in with its pairing code, not an account".to_string(),
        e => e.message().to_string(),
    })?;
    audit("account", if linked { "gateway_linked" } else { "gateway_unlinked" });
    Ok(())
}
