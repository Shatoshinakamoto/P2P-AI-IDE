//! accounts -- sign in to gateways with a P2P-AI account (universal accounts, ADR-030 phase A3).
//!
//! Purpose: instead of pasting a key for every gateway, a person signs in once at an account service (the
//! "home", `p2p-accounts` in the P2P-AI repository) and the IDE asks the home for a short token for each gateway
//! that accepts the home. The gateway turns the account into its own ordinary user key (P2P-AI
//! `docs/features/gateway-universal-accounts.md`). Docs: `docs/features/account-sign-in.md`.
//!
//! Flow (OAuth 2.0 for native apps, RFC 8252, with PKCE, RFC 7636):
//!   1. [`Loopback::bind`] opens a one-shot listener on `127.0.0.1:<random port>`.
//!   2. [`Pkce::new`] makes a verifier, its S256 challenge and a `state` value; [`authorize_url`] builds the
//!      home's sign-in link, which the shell opens in the system browser (never inside the app, RFC 8252 8.12).
//!   3. The home redirects the browser to `http://127.0.0.1:<port>/cb?code&state&iss`; [`Loopback::wait`] checks
//!      `state` and `iss` and returns the code.
//!   4. [`Home::exchange`] trades the code plus the verifier for a refresh token (kept in the OS keychain) and an
//!      account token for the home's own API (`/v1/me`, to show the email).
//!   5. Before each gateway call, [`Accounts::token_for`] refreshes for `resource=<gateway id>` and keeps the
//!      5-minute access token in memory only. The home rotates the refresh token on every use, so the new one is
//!      written to the keychain before the access token is used.
//!
//! Inputs and trust boundaries: home URLs and gateway ids come from the webview (untrusted, checked here); the
//! browser redirect reaches the loopback listener from any local process (checked: path, `state`, `iss`); every home
//! answer is untrusted JSON parsed into fixed shapes with size caps.
//!
//! Security controls:
//!   * PKCE S256 with a 256-bit verifier from the OS CSPRNG; 256-bit `state` compared in constant time
//!     (RFC 7636, RFC 6749 10.12, CWE-352, CWE-208).
//!   * Redirect to the IP literal 127.0.0.1 only, random port, one path; the home itself refuses other redirects
//!     (RFC 8252 7.3, 8.3; CWE-601). Mix-up defence: `iss` must equal the home we started with (RFC 9207).
//!   * The listener answers at most 32 connections and 5 minutes, reads at most 8 KiB per request with a 5 s read
//!     timeout, and its pages carry `default-src 'none'` (CWE-400, CWE-79).
//!   * Refresh tokens live only in the OS keychain ([`SecretStore`], service [`TOKEN_SERVICE`]); access tokens only
//!     in memory, never in files, logs, errors or the webview; neither ever goes to a gateway's Node extension host
//!     or an environment variable (CWE-522, CWE-532, NIST IA-5). Refreshes for one person are serialised, so two
//!     calls can never send the same rotating refresh token twice (which the home treats as theft, RFC 9700 4.14).
//!   * Home connections: https with the Mozilla roots, plain http only on loopback (flagged INSECURE — DEV ONLY);
//!     no proxy, no redirects, 64 KiB answers, 10 s connect / 20 s total (NIST SC-8, CWE-295, CWE-400).
//!   * Least privilege: gateway tokens ask only for `chat projects:read projects:write missions`; the `account` scope
//!     is only ever asked for the home itself (NIST AC-6).
//!   * Errors are fixed messages (CWE-209).
//!
//! Dependencies: ureq 3 (same agent settings as client.rs), serde_json, sha2, base64, getrandom (all already used
//! by the core; no new crates).

use crate::gateways::{check_url, GwError, SecretStore};
use crate::store::{self, StoreError};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The IDE's built-in client id at every account service (`NativeClients` in the Go package).
pub const CLIENT_ID: &str = "p2p-ai-ide";
/// Keychain service name for refresh tokens (account = home id, 16 hex).
pub const TOKEN_SERVICE: &str = "org.p2p-ai.ide.account";
/// Scopes asked for at sign-in: what the IDE uses at gateways, plus the home's own account API.
pub const SIGN_IN_SCOPE: &str = "chat projects:read projects:write missions account";
/// Scopes asked for in a gateway token. SECURITY: never `account` (the home refuses it at a gateway anyway).
pub const GATEWAY_SCOPE: &str = "chat projects:read projects:write missions";
/// How long the browser sign-in may take.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
/// Most homes remembered (CWE-400).
pub const MAX_HOMES: usize = 16;
/// Path of the loopback redirect.
const CB_PATH: &str = "/cb";
/// Largest answer read from a home.
const MAX_ANSWER: u64 = 64 << 10;
/// Largest request read by the loopback listener.
const MAX_CB_REQUEST: usize = 8 << 10;
/// Most connections the listener answers before it gives up.
const MAX_CB_CONNECTIONS: usize = 32;
/// File with the remembered homes inside the app data folder.
const FILE: &str = "accounts.json";

/// Fixed errors; `message()` is safe to show (no URL, token or answer text, CWE-209).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcctError {
    BadHome,
    BadAudience,
    NotSignedIn,
    SignedOut,
    PasswordChange,
    NotForGateway,
    NoScope,
    Cancelled,
    Denied,
    TimedOut,
    Listener,
    Browser,
    Connect,
    Tls,
    Busy,
    BadAnswer,
    Status(u16),
    BadPassword,
    TooMany,
    Keychain,
    Store(StoreError),
}

impl AcctError {
    pub fn message(&self) -> String {
        match self {
            AcctError::BadHome => {
                "the account service address must be https://host[:port][/path] (or http://127.0.0.1:port for a dev service)".into()
            }
            AcctError::BadAudience => {
                "the gateway's account id must be 3 to 128 characters: a-z, 0-9, '.', '_', ':' or '-'".into()
            }
            AcctError::NotSignedIn => "sign in to this account service first".into(),
            AcctError::SignedOut => "the sign-in has ended; sign in again".into(),
            AcctError::PasswordChange => "this account must choose its own password first (Account settings)".into(),
            AcctError::NotForGateway => {
                "the account service does not issue tokens for this gateway (check the gateway's account id)".into()
            }
            AcctError::NoScope => "the sign-in was not granted what this gateway needs; sign in again".into(),
            AcctError::Cancelled => "sign-in cancelled".into(),
            AcctError::Denied => "the sign-in was refused or cancelled in the browser".into(),
            AcctError::TimedOut => "the sign-in took longer than 5 minutes; try again".into(),
            AcctError::Listener => "could not open the local sign-in return address".into(),
            AcctError::Browser => "could not open the system browser".into(),
            AcctError::Connect => "could not reach the account service".into(),
            AcctError::Tls => "the account service's certificate is not trusted".into(),
            AcctError::Busy => "the account service is limiting requests; try again shortly".into(),
            AcctError::BadAnswer => "the account service's answer could not be read".into(),
            AcctError::Status(s) => format!("the account service answered with an error (HTTP {s})"),
            AcctError::BadPassword => "the password was not accepted (check the current one; the new one needs 15+ characters and must not be a common password)".into(),
            AcctError::TooMany => "at most 16 account services can be remembered".into(),
            AcctError::Keychain => "the system keychain could not store or read the sign-in".into(),
            AcctError::Store(e) => e.message().into(),
        }
    }
}

impl From<GwError> for AcctError {
    fn from(e: GwError) -> Self {
        match e {
            GwError::Store(s) => AcctError::Store(s),
            _ => AcctError::Keychain,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------------------------------------------

/// Checks and normalises an account service address (its `ACCOUNTS_ISSUER`): `https://host[:port]` plus an optional
/// path of `[A-Za-z0-9._~-]` segments, no trailing slash, query, fragment, user info or `..`.
///
/// SECURITY: the host part goes through the same allow-list parser as gateway URLs (CWE-20, CWE-918); plain http
/// only for loopback (NIST SC-8). The result must equal the home's `iss` exactly (RFC 9207).
pub fn check_home(url: &str) -> Result<String, AcctError> {
    let u = url.trim().trim_end_matches('/');
    if u.len() > 300 {
        return Err(AcctError::BadHome);
    }
    let after = u.find("://").map(|i| i + 3).ok_or(AcctError::BadHome)?;
    let (origin, path) = match u[after..].find('/') {
        Some(i) => u.split_at(after + i),
        None => (u, ""),
    };
    let origin = check_url(origin).map_err(|_| AcctError::BadHome)?;
    if !path.is_empty() {
        let ok = path.split('/').skip(1).all(|seg| {
            !seg.is_empty()
                && seg != "."
                && seg != ".."
                && seg.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
        });
        if !ok {
            return Err(AcctError::BadHome);
        }
    }
    Ok(format!("{origin}{path}"))
}

/// Checks a gateway's account id (its `GATEWAY_ID`), with the same rule as the Go package's `ValidAudience`.
pub fn check_audience(aud: &str) -> Result<String, AcctError> {
    let a = aud.trim();
    let ok = (3..=128).contains(&a.len())
        && a.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || (i > 0 && matches!(b, b'.' | b'_' | b':' | b'-'))
        });
    if !ok {
        return Err(AcctError::BadAudience);
    }
    Ok(a.to_string())
}

/// The keychain / file id of a home: the first 16 hex of SHA-256 of its normalised address.
pub fn home_id(issuer: &str) -> String {
    let d = Sha256::digest(issuer.as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// True for a plain-http (loopback) home: shown as **INSECURE — DEV ONLY**.
pub fn home_dev(issuer: &str) -> bool {
    issuer.starts_with("http://")
}

/// Which account a gateway uses: saved on the gateway entry (no secret in it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountLink {
    /// The account service address (normalised by [`check_home`]).
    pub home: String,
    /// The gateway's `GATEWAY_ID`, the token audience.
    pub audience: String,
}

impl AccountLink {
    /// Checks both fields.
    pub fn new(home: &str, audience: &str) -> Result<AccountLink, AcctError> {
        Ok(AccountLink { home: check_home(home)?, audience: check_audience(audience)? })
    }
    /// True when both fields are already normalised (used when loading our own file).
    pub fn valid(&self) -> bool {
        check_home(&self.home).as_deref() == Ok(self.home.as_str())
            && check_audience(&self.audience).as_deref() == Ok(self.audience.as_str())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// PKCE and the sign-in link
// ---------------------------------------------------------------------------------------------------------------

/// One sign-in attempt's secrets. Never logged (no Debug).
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

fn random_b64(n: usize) -> String {
    let mut b = vec![0u8; n];
    // SECURITY: OS CSPRNG (CWE-338). A failing OS generator is not recoverable: refuse to continue.
    getrandom::getrandom(&mut b).expect("the OS random number generator failed");
    URL_SAFE_NO_PAD.encode(b)
}

impl Pkce {
    /// A fresh verifier (32 random bytes = 43 characters), its S256 challenge, and a 32-byte `state`.
    pub fn new() -> Pkce {
        let verifier = random_b64(32);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Pkce { verifier, challenge, state: random_b64(32) }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

/// Percent-encodes a query value (RFC 3986 unreserved characters kept).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decodes a query value; `None` on bad escapes or non-UTF-8.
fn dec(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(h, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// The home's sign-in link for this attempt.
pub fn authorize_url(issuer: &str, redirect: &str, p: &Pkce) -> String {
    format!(
        "{issuer}/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}&scope={}",
        enc(CLIENT_ID),
        enc(redirect),
        enc(&p.challenge),
        enc(&p.state),
        enc(SIGN_IN_SCOPE)
    )
}

// ---------------------------------------------------------------------------------------------------------------
// The loopback listener
// ---------------------------------------------------------------------------------------------------------------

/// The one-shot local return address for the browser.
pub struct Loopback {
    listener: TcpListener,
    /// `http://127.0.0.1:<port>/cb`, sent as `redirect_uri`.
    pub redirect: String,
}

/// What one request to the listener asked.
#[derive(Debug, PartialEq, Eq)]
pub enum Callback {
    /// The home sent a code.
    Code(String),
    /// The home (or the person) ended the sign-in (`error=`).
    Error,
    /// Not our callback (wrong path, method, state or iss): answer and keep waiting.
    Ignore,
}

/// Parses one HTTP request head sent to the listener. Pure, so it is tested without sockets.
/// SECURITY: only `GET /cb`; `state` compared in constant time; `iss` must equal the home (RFC 9207, CWE-352).
pub fn parse_callback(head: &str, state: &str, issuer: &str) -> Callback {
    let line = head.lines().next().unwrap_or("");
    let mut parts = line.split(' ');
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
        return Callback::Ignore;
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != CB_PATH {
        return Callback::Ignore;
    }
    let mut q: HashMap<&str, String> = HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        // SECURITY: a repeated parameter is ambiguous: refuse the request (CWE-235).
        let Some(v) = dec(v) else { return Callback::Ignore };
        if q.insert(k, v).is_some() {
            return Callback::Ignore;
        }
    }
    let st_ok = q.get("state").is_some_and(|s| ct_eq(s.as_bytes(), state.as_bytes()));
    if !st_ok {
        return Callback::Ignore;
    }
    if q.contains_key("error") {
        return Callback::Error;
    }
    // SECURITY: mix-up defence: the code must come from the home this sign-in started at (RFC 9207).
    if q.get("iss").map(String::as_str) != Some(issuer) {
        return Callback::Ignore;
    }
    match q.get("code") {
        Some(c) if (16..=512).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_graphic()) => {
            Callback::Code(c.clone())
        }
        _ => Callback::Ignore,
    }
}

/// Constant-time byte comparison (CWE-208).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn page(status: &str, title: &str, text: &str) -> Vec<u8> {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body style=\"font-family:sans-serif;max-width:32rem;margin:4rem auto\"><h1>{title}</h1><p>{text}</p></body></html>"
    );
    // SECURITY: fixed text only; no script or outside content may run on this page (CWE-79).
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nReferrer-Policy: no-referrer\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

impl Loopback {
    /// Binds `127.0.0.1:0`. SECURITY: the IP literal, never `localhost` (RFC 8252 8.3).
    pub fn bind() -> Result<Loopback, AcctError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| AcctError::Listener)?;
        let port = listener.local_addr().map_err(|_| AcctError::Listener)?.port();
        listener.set_nonblocking(true).map_err(|_| AcctError::Listener)?;
        Ok(Loopback { listener, redirect: format!("http://127.0.0.1:{port}{CB_PATH}") })
    }

    /// Waits for the browser to come back with a code, until `deadline` or `cancel`.
    pub fn wait(&self, state: &str, issuer: &str, deadline: Instant, cancel: &AtomicBool) -> Result<String, AcctError> {
        let mut served = 0;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(AcctError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(AcctError::TimedOut);
            }
            match self.listener.accept() {
                Ok((conn, _)) => {
                    served += 1;
                    if let Some(r) = serve_one(conn, state, issuer) {
                        return r;
                    }
                    if served >= MAX_CB_CONNECTIONS {
                        return Err(AcctError::Listener);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => return Err(AcctError::Listener),
            }
        }
    }
}

/// Reads one request and answers it. `Some` ends the wait.
fn serve_one(mut conn: TcpStream, state: &str, issuer: &str) -> Option<Result<String, AcctError>> {
    let _ = conn.set_nonblocking(false);
    let _ = conn.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = conn.set_write_timeout(Some(Duration::from_secs(5)));
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    // SECURITY: bounded read of the head only (CWE-400).
    while buf.len() < MAX_CB_REQUEST && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match conn.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let (reply, result) = match parse_callback(&head, state, issuer) {
        Callback::Code(c) => {
            (page("200 OK", "Signed in", "You are signed in to the P2P-AI IDE. You can close this tab."), Some(Ok(c)))
        }
        Callback::Error => (
            page("200 OK", "Sign-in not finished", "The sign-in was cancelled. You can close this tab."),
            Some(Err(AcctError::Denied)),
        ),
        Callback::Ignore => (page("404 Not Found", "Not found", "Nothing here."), None),
    };
    let _ = conn.write_all(&reply);
    let _ = conn.shutdown(Shutdown::Both);
    result
}

// ---------------------------------------------------------------------------------------------------------------
// The home's token API
// ---------------------------------------------------------------------------------------------------------------

/// What the code exchange returned. No Debug: holds a secret.
pub struct SignedIn {
    pub refresh_token: String,
    pub account_token: Option<String>,
    pub password_change_required: bool,
}

/// What a refresh returned. No Debug: holds secrets.
pub struct Refreshed {
    pub refresh_token: String,
    pub access_token: String,
    pub expires_in: u64,
    pub scope: String,
}

/// The profile shown in the app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub email: String,
    #[serde(default)]
    pub name: String,
}

/// A client for one account service.
pub struct Home {
    pub issuer: String,
    agent: ureq::Agent,
}

impl Home {
    /// `issuer` must already be checked by [`check_home`].
    pub fn new(issuer: &str) -> Result<Home, AcctError> {
        let issuer = check_home(issuer)?;
        let config = ureq::Agent::config_builder()
            // SECURITY: no proxy and no redirects, so tokens only reach this host; https except loopback dev (SC-8).
            .proxy(None)
            .max_redirects(0)
            .https_only(!home_dev(&issuer))
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent("p2p-ai-ide/0.1")
            .build();
        Ok(Home { issuer, agent: config.into() })
    }

    fn read(resp: ureq::http::Response<ureq::Body>) -> Result<(u16, Value), AcctError> {
        let status = resp.status().as_u16();
        let mut body = Vec::new();
        resp.into_body().into_reader().take(MAX_ANSWER + 1).read_to_end(&mut body).map_err(|_| AcctError::BadAnswer)?;
        if body.len() as u64 > MAX_ANSWER {
            return Err(AcctError::BadAnswer);
        }
        let v = if body.is_empty() { Value::Null } else { serde_json::from_slice(&body).unwrap_or(Value::Null) };
        Ok((status, v))
    }

    fn form(&self, path: &str, fields: &[(&str, &str)]) -> Result<(u16, Value), AcctError> {
        let body = fields.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&");
        let resp = self
            .agent
            .post(format!("{}{path}", self.issuer))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .send(body.as_bytes())
            .map_err(map_ureq)?;
        Self::read(resp)
    }

    /// Trades the browser's code for a session (and an account token for the home's own API).
    pub fn exchange(&self, code: &str, redirect: &str, verifier: &str) -> Result<SignedIn, AcctError> {
        let (status, v) = self.form(
            "/token",
            &[
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT_ID),
                ("code", code),
                ("redirect_uri", redirect),
                ("code_verifier", verifier),
                ("resource", &self.issuer),
            ],
        )?;
        if status != 200 {
            return Err(token_error(status, &v));
        }
        let rt = refresh_token(&v)?;
        Ok(SignedIn {
            refresh_token: rt,
            account_token: v.get("access_token").and_then(Value::as_str).filter(|t| token_shape(t)).map(String::from),
            password_change_required: v.get("password_change_required").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// Uses the refresh token for a token at `resource` (a gateway id, or the issuer for the account API).
    pub fn refresh(&self, rt: &str, resource: &str, scope: &str) -> Result<Refreshed, AcctError> {
        let (status, v) = self.form(
            "/token",
            &[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("refresh_token", rt),
                ("resource", resource),
                ("scope", scope),
            ],
        )?;
        if status != 200 {
            return Err(token_error(status, &v));
        }
        let refresh_token = refresh_token(&v)?;
        let access_token =
            v.get("access_token").and_then(Value::as_str).filter(|t| token_shape(t)).ok_or(AcctError::BadAnswer)?;
        // The audience must be the one asked for (a confused home must not hand us another gateway's token).
        if v.get("audience").and_then(Value::as_str).is_some_and(|a| a != resource) {
            return Err(AcctError::BadAnswer);
        }
        Ok(Refreshed {
            refresh_token,
            access_token: access_token.to_string(),
            expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(60).clamp(10, 600),
            scope: v.get("scope").and_then(Value::as_str).unwrap_or("").chars().take(200).collect(),
        })
    }

    /// Ends this app's session at the home (RFC 7009). Best effort.
    pub fn revoke(&self, rt: &str) -> Result<(), AcctError> {
        let (status, _) = self.form("/revoke", &[("token", rt), ("client_id", CLIENT_ID)])?;
        if status == 200 {
            Ok(())
        } else {
            Err(AcctError::Status(status))
        }
    }

    /// The signed-in person's profile (account token).
    pub fn me(&self, account_token: &str) -> Result<Profile, AcctError> {
        let resp = self
            .agent
            .get(format!("{}/v1/me", self.issuer))
            .header("Authorization", format!("Bearer {account_token}"))
            .header("Accept", "application/json")
            .call()
            .map_err(map_ureq)?;
        let (status, v) = Self::read(resp)?;
        if status != 200 {
            return Err(token_error(status, &v));
        }
        let text = |k: &str, n: usize| -> String {
            v.get(k).and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control()).take(n).collect()
        };
        let email = text("email", 254);
        if email.is_empty() {
            return Err(AcctError::BadAnswer);
        }
        Ok(Profile { email, name: text("name", 128) })
    }

    /// Changes the password (needed first for an operator-made one-time password). Ends the other sessions.
    pub fn change_password(&self, account_token: &str, current: &str, new: &str) -> Result<(), AcctError> {
        let body = serde_json::json!({"current": current, "new": new}).to_string();
        let resp = self
            .agent
            .post(format!("{}/v1/me/password", self.issuer))
            .header("Authorization", format!("Bearer {account_token}"))
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .map_err(map_ureq)?;
        let (status, v) = Self::read(resp)?;
        match status {
            200 | 204 => Ok(()),
            400 | 403 | 422 => Err(AcctError::BadPassword),
            _ => Err(token_error(status, &v)),
        }
    }
}

/// Our tokens are short visible ASCII (`p2prt_...` or a JWT). SECURITY: also keeps them out of header injection (CWE-113).
fn token_shape(t: &str) -> bool {
    (16..=4096).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_graphic())
}

fn refresh_token(v: &Value) -> Result<String, AcctError> {
    v.get("refresh_token")
        .and_then(Value::as_str)
        .filter(|t| token_shape(t))
        .map(String::from)
        .ok_or(AcctError::BadAnswer)
}

/// Maps a token-endpoint refusal to a fixed error (the home's text is never shown, CWE-209).
pub fn token_error(status: u16, v: &Value) -> AcctError {
    let code = v.get("error").and_then(Value::as_str).unwrap_or("");
    match (status, code) {
        (_, "invalid_grant") => AcctError::SignedOut,
        (_, "password_change_required") => AcctError::PasswordChange,
        (_, "invalid_target") => AcctError::NotForGateway,
        (_, "invalid_scope") => AcctError::NoScope,
        (429, _) => AcctError::Busy,
        (401, _) => AcctError::SignedOut,
        (s, _) => AcctError::Status(s),
    }
}

fn map_ureq(e: ureq::Error) -> AcctError {
    match e {
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => AcctError::Tls,
        ureq::Error::Io(ref io) if io.kind() == std::io::ErrorKind::InvalidData => AcctError::Tls,
        _ => AcctError::Connect,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Remembered homes and the token cache
// ---------------------------------------------------------------------------------------------------------------

/// One remembered home (no secret in it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeRecord {
    pub id: String,
    pub issuer: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password_change_required: bool,
    /// Unix seconds of the last sign-in.
    #[serde(default)]
    pub signed_in_at: u64,
}

/// What the webview sees of a home.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HomeView {
    pub id: String,
    pub issuer: String,
    pub email: String,
    pub dev: bool,
    pub signed_in: bool,
    pub password_change_required: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HomeFile {
    #[serde(default)]
    homes: Vec<HomeRecord>,
}

/// Remembered homes, the in-memory token cache, and the lock that serialises refreshes.
pub struct Accounts {
    path: PathBuf,
    /// (home id, audience) -> (access token, valid until). SECURITY: memory only.
    cache: Mutex<HashMap<(String, String), (String, Instant)>>,
    /// Held for every use of a refresh token (rotation must never race).
    refresh_lock: Mutex<()>,
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Accounts {
    /// `dir` is the app's data folder, chosen by the shell.
    pub fn new(dir: &Path) -> Accounts {
        Accounts { path: dir.join(FILE), cache: Mutex::new(HashMap::new()), refresh_lock: Mutex::new(()) }
    }

    fn load(&self) -> Result<HomeFile, AcctError> {
        let bytes = match store::read_capped(&self.path, 256 << 10) {
            Ok(b) => b,
            Err(StoreError::Missing) => return Ok(HomeFile::default()),
            Err(e) => return Err(AcctError::Store(e)),
        };
        let f: HomeFile = serde_json::from_slice(&bytes).map_err(|_| AcctError::Store(StoreError::Bad))?;
        // SECURITY: our own file is re-checked so an edited file cannot point a keychain entry elsewhere (CWE-20).
        let ok = f.homes.len() <= MAX_HOMES
            && f.homes.iter().all(|h| {
                check_home(&h.issuer).as_deref() == Ok(h.issuer.as_str())
                    && h.id == home_id(&h.issuer)
                    && h.email.len() <= 254
                    && !h.email.chars().any(char::is_control)
            });
        if !ok {
            return Err(AcctError::Store(StoreError::Bad));
        }
        Ok(f)
    }

    fn save(&self, f: &HomeFile) -> Result<(), AcctError> {
        let bytes = serde_json::to_vec_pretty(f).map_err(|_| AcctError::Store(StoreError::Bad))?;
        store::write_atomic(&self.path, &bytes).map_err(AcctError::Store)
    }

    /// The remembered homes for the webview.
    pub fn views(&self, secrets: &dyn SecretStore) -> Result<Vec<HomeView>, AcctError> {
        Ok(self
            .load()?
            .homes
            .into_iter()
            .map(|h| HomeView {
                signed_in: secrets.get(&h.id).ok().flatten().is_some(),
                dev: home_dev(&h.issuer),
                id: h.id,
                issuer: h.issuer,
                email: h.email,
                password_change_required: h.password_change_required,
            })
            .collect())
    }

    fn upsert(&self, rec: HomeRecord) -> Result<(), AcctError> {
        let mut f = self.load()?;
        if let Some(h) = f.homes.iter_mut().find(|h| h.id == rec.id) {
            *h = rec;
        } else {
            if f.homes.len() >= MAX_HOMES {
                return Err(AcctError::TooMany);
            }
            f.homes.push(rec);
        }
        self.save(&f)
    }

    /// The whole browser sign-in at `issuer`. `open` opens a URL in the system browser. Blocks up to 5 minutes;
    /// `cancel` stops it. Returns the home's view.
    pub fn sign_in(
        &self,
        secrets: &dyn SecretStore,
        issuer: &str,
        open: &dyn Fn(&str) -> bool,
        cancel: &AtomicBool,
    ) -> Result<HomeView, AcctError> {
        let home = Home::new(issuer)?;
        let id = home_id(&home.issuer);
        if !self.load()?.homes.iter().any(|h| h.id == id) && self.load()?.homes.len() >= MAX_HOMES {
            return Err(AcctError::TooMany);
        }
        let lb = Loopback::bind()?;
        let redirect = lb.redirect.clone();
        let p = Pkce::new();
        if !open(&authorize_url(&home.issuer, &redirect, &p)) {
            return Err(AcctError::Browser);
        }
        let code = lb.wait(&p.state, &home.issuer, Instant::now() + SIGN_IN_TIMEOUT, cancel)?;
        drop(lb);
        let _g = self.refresh_lock.lock().unwrap_or_else(|e| e.into_inner());
        let s = home.exchange(&code, &redirect, &p.verifier)?;
        // SECURITY: the refresh token goes to the keychain only (CWE-522).
        secrets.set(&id, &s.refresh_token)?;
        let email = match s.account_token.as_deref().map(|t| home.me(t)) {
            Some(Ok(pr)) => pr.email,
            _ => String::new(),
        };
        self.upsert(HomeRecord {
            id: id.clone(),
            issuer: home.issuer.clone(),
            email,
            password_change_required: s.password_change_required,
            signed_in_at: now_unix(),
        })?;
        self.clear_cache(&id);
        self.views(secrets)?.into_iter().find(|h| h.id == id).ok_or(AcctError::BadAnswer)
    }

    fn clear_cache(&self, id: &str) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).retain(|(h, _), _| h != id);
    }

    fn record(&self, id: &str) -> Result<HomeRecord, AcctError> {
        self.load()?.homes.into_iter().find(|h| h.id == id).ok_or(AcctError::NotSignedIn)
    }

    /// Signs this app out of a home: revokes the session (best effort), forgets the refresh token and cached tokens.
    pub fn sign_out(&self, secrets: &dyn SecretStore, id: &str) -> Result<(), AcctError> {
        if !store::id_ok(id) {
            return Err(AcctError::NotSignedIn);
        }
        let _g = self.refresh_lock.lock().unwrap_or_else(|e| e.into_inner());
        let rec = self.record(id)?;
        if let Some(rt) = secrets.get(id)? {
            if let Ok(h) = Home::new(&rec.issuer) {
                let _ = h.revoke(&rt);
            }
        }
        secrets.delete(id)?;
        self.clear_cache(id);
        let mut f = self.load()?;
        f.homes.retain(|h| h.id != id);
        self.save(&f)
    }

    /// One refresh under the lock; stores the rotated refresh token first. On a dead session the stored token is
    /// dropped so the app shows "signed out".
    fn refresh_locked(
        &self,
        secrets: &dyn SecretStore,
        rec: &HomeRecord,
        resource: &str,
        scope: &str,
    ) -> Result<Refreshed, AcctError> {
        let rt = secrets.get(&rec.id)?.ok_or(AcctError::NotSignedIn)?;
        let home = Home::new(&rec.issuer)?;
        match home.refresh(&rt, resource, scope) {
            Ok(r) => {
                // SECURITY: the old token is now retired at the home; keep only the new one (RFC 9700 4.14.2).
                secrets.set(&rec.id, &r.refresh_token)?;
                Ok(r)
            }
            Err(AcctError::SignedOut) => {
                let _ = secrets.delete(&rec.id);
                self.clear_cache(&rec.id);
                Err(AcctError::SignedOut)
            }
            Err(AcctError::PasswordChange) => {
                let mut r = rec.clone();
                r.password_change_required = true;
                let _ = self.upsert(r);
                Err(AcctError::PasswordChange)
            }
            Err(e) => Err(e),
        }
    }

    /// A gateway token for `link` (cached in memory until a minute before it runs out).
    pub fn token_for(&self, secrets: &dyn SecretStore, link: &AccountLink) -> Result<String, AcctError> {
        if !link.valid() {
            return Err(AcctError::BadHome);
        }
        let id = home_id(&link.home);
        let key = (id.clone(), link.audience.clone());
        if let Some((t, until)) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            if Instant::now() < *until {
                return Ok(t.clone());
            }
        }
        let _g = self.refresh_lock.lock().unwrap_or_else(|e| e.into_inner());
        // Another call may have refreshed while this one waited for the lock.
        if let Some((t, until)) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            if Instant::now() < *until {
                return Ok(t.clone());
            }
        }
        let rec = self.record(&id)?;
        let r = self.refresh_locked(secrets, &rec, &link.audience, GATEWAY_SCOPE)?;
        if !r.scope.split(' ').any(|s| s == "chat") {
            return Err(AcctError::NoScope);
        }
        let ttl = Duration::from_secs(r.expires_in.saturating_sub(60).max(5));
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, (r.access_token.clone(), Instant::now() + ttl));
        Ok(r.access_token)
    }

    /// Changes the password at the home (needed first for an operator-made one-time password). This session keeps
    /// working; the home ends the person's other sessions.
    pub fn change_password(
        &self,
        secrets: &dyn SecretStore,
        id: &str,
        current: &str,
        new: &str,
    ) -> Result<(), AcctError> {
        if current.is_empty() || new.chars().count() < 15 || new.len() > 256 || current.len() > 256 {
            return Err(AcctError::BadPassword);
        }
        let _g = self.refresh_lock.lock().unwrap_or_else(|e| e.into_inner());
        let rec = self.record(id)?;
        let r = self.refresh_locked(secrets, &rec, &rec.issuer, "account")?;
        let home = Home::new(&rec.issuer)?;
        home.change_password(&r.access_token, current, new)?;
        let mut rec = rec;
        rec.password_change_required = false;
        if let Ok(p) = home.me(&r.access_token) {
            rec.email = p.email;
        }
        self.upsert(rec)?;
        self.clear_cache(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateways::GwError;
    use std::cell::RefCell;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    #[derive(Default)]
    struct Mem(RefCell<HashMap<String, String>>);
    impl SecretStore for Mem {
        fn get(&self, id: &str) -> Result<Option<String>, GwError> {
            Ok(self.0.borrow().get(id).cloned())
        }
        fn set(&self, id: &str, s: &str) -> Result<(), GwError> {
            self.0.borrow_mut().insert(id.into(), s.into());
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), GwError> {
            self.0.borrow_mut().remove(id);
            Ok(())
        }
    }

    #[test]
    fn home_addresses_are_allow_listed() {
        assert_eq!(check_home("https://Accounts.Example.org/").unwrap(), "https://accounts.example.org");
        assert_eq!(check_home("https://example.org:8443/acct/v1").unwrap(), "https://example.org:8443/acct/v1");
        assert_eq!(check_home("http://127.0.0.1:9000").unwrap(), "http://127.0.0.1:9000");
        for bad in [
            "http://accounts.example.org",
            "ftp://x.org",
            "https://user@x.org",
            "https://x.org/a/../b",
            "https://x.org/a?b=1",
            "https://x.org/a#f",
            "https://x.org/a%2e",
            "https://x.org//a",
            "accounts.example.org",
        ] {
            assert_eq!(check_home(bad), Err(AcctError::BadHome), "{bad}");
        }
    }

    #[test]
    fn audiences_follow_the_go_rule() {
        assert!(check_audience("gw-berlin-01").is_ok());
        assert!(check_audience("gw.a_b:c").is_ok());
        for bad in ["gw", "-gw", "GW-1", "gw 1", "gw/1", &"a".repeat(129)] {
            assert_eq!(check_audience(bad), Err(AcctError::BadAudience), "{bad}");
        }
    }

    #[test]
    fn pkce_is_s256_of_a_43_char_verifier_and_fresh_each_time() {
        let a = Pkce::new();
        let b = Pkce::new();
        assert_eq!(a.verifier.len(), 43);
        assert_eq!(a.challenge.len(), 43);
        assert_eq!(a.challenge, URL_SAFE_NO_PAD.encode(Sha256::digest(a.verifier.as_bytes())));
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
        let u = authorize_url("https://h.example", "http://127.0.0.1:5000/cb", &a);
        assert!(u.starts_with("https://h.example/authorize?response_type=code&client_id=p2p-ai-ide&"));
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5000%2Fcb"));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("scope=chat%20projects%3Aread%20projects%3Awrite%20missions%20account"));
        assert!(!u.contains(&a.verifier));
    }

    #[test]
    fn query_codec_round_trips_and_refuses_bad_escapes() {
        let s = "a b/c?d=e&f%g~";
        assert_eq!(dec(&enc(s)).unwrap(), s);
        assert_eq!(dec("%zz"), None);
        assert_eq!(dec("%4"), None);
    }

    #[test]
    fn callback_checks_path_state_iss_and_duplicates() {
        let st = "S".repeat(43);
        let iss = "https://h.example";
        let ok = format!("GET /cb?code={}&state={st}&iss=https%3A%2F%2Fh.example HTTP/1.1\r\n\r\n", "c".repeat(40));
        assert_eq!(parse_callback(&ok, &st, iss), Callback::Code("c".repeat(40)));
        // Wrong state: a page on another site cannot finish our sign-in (CSRF).
        let wrong = ok.replace(&st, &"T".repeat(43));
        assert_eq!(parse_callback(&wrong, &st, iss), Callback::Ignore);
        // Mix-up: a code from another home.
        let other = ok.replace("h.example", "evil.example");
        assert_eq!(parse_callback(&other, &st, iss), Callback::Ignore);
        // Missing iss.
        let no_iss = format!("GET /cb?code={}&state={st} HTTP/1.1\r\n\r\n", "c".repeat(40));
        assert_eq!(parse_callback(&no_iss, &st, iss), Callback::Ignore);
        // Repeated code parameter.
        let dup = ok.replace("&state=", &format!("&code={}&state=", "d".repeat(40)));
        assert_eq!(parse_callback(&dup, &st, iss), Callback::Ignore);
        assert_eq!(parse_callback(&ok.replace("/cb?", "/other?"), &st, iss), Callback::Ignore);
        assert_eq!(parse_callback(&ok.replace("GET", "POST"), &st, iss), Callback::Ignore);
        let err = format!("GET /cb?error=access_denied&state={st} HTTP/1.1\r\n\r\n");
        assert_eq!(parse_callback(&err, &st, iss), Callback::Error);
        // An error with the wrong state does not end the wait.
        assert_eq!(parse_callback(&err.replace(&st, "x"), &st, iss), Callback::Ignore);
    }

    #[test]
    fn loopback_ignores_strangers_then_takes_the_code() {
        let lb = Loopback::bind().unwrap();
        assert!(lb.redirect.starts_with("http://127.0.0.1:") && lb.redirect.ends_with("/cb"));
        let port: u16 = lb.redirect.trim_start_matches("http://127.0.0.1:").trim_end_matches("/cb").parse().unwrap();
        let st = "Q".repeat(43);
        let st2 = st.clone();
        let t = std::thread::spawn(move || {
            let send = |req: String| {
                let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
                c.write_all(req.as_bytes()).unwrap();
                let mut out = String::new();
                let _ = c.read_to_string(&mut out);
                out
            };
            let a = send("GET /favicon.ico HTTP/1.1\r\n\r\n".into());
            let b = send(format!("GET /cb?code={}&state={} HTTP/1.1\r\n\r\n", "c".repeat(20), "x".repeat(43)));
            let c = send(format!(
                "GET /cb?code={}&state={st2}&iss=http%3A%2F%2F127.0.0.1%3A9 HTTP/1.1\r\n\r\n",
                "c".repeat(20)
            ));
            (a, b, c)
        });
        let code =
            lb.wait(&st, "http://127.0.0.1:9", Instant::now() + Duration::from_secs(10), &AtomicBool::new(false));
        let (a, b, c) = t.join().unwrap();
        assert_eq!(code.unwrap(), "c".repeat(20));
        assert!(a.starts_with("HTTP/1.1 404") && b.starts_with("HTTP/1.1 404"));
        assert!(c.starts_with("HTTP/1.1 200") && c.contains("default-src 'none'") && c.contains("no-store"));
    }

    #[test]
    fn loopback_wait_stops_on_cancel_and_deadline() {
        let lb = Loopback::bind().unwrap();
        let cancel = AtomicBool::new(true);
        assert_eq!(lb.wait("s", "i", Instant::now() + Duration::from_secs(5), &cancel), Err(AcctError::Cancelled));
        let no = AtomicBool::new(false);
        assert_eq!(lb.wait("s", "i", Instant::now() + Duration::from_millis(150), &no), Err(AcctError::TimedOut));
    }

    #[test]
    fn token_errors_map_to_fixed_kinds() {
        let e = |s, c: &str| token_error(s, &serde_json::json!({"error": c, "error_description": "secret detail"}));
        assert_eq!(e(400, "invalid_grant"), AcctError::SignedOut);
        assert_eq!(e(403, "password_change_required"), AcctError::PasswordChange);
        assert_eq!(e(400, "invalid_target"), AcctError::NotForGateway);
        assert_eq!(e(429, ""), AcctError::Busy);
        assert_eq!(e(500, "x"), AcctError::Status(500));
        assert!(!AcctError::Status(500).message().contains("secret"));
    }

    /// A fake home: `/token` rotates refresh tokens like the real one (an old token kills the session).
    fn fake_home(refreshes: Arc<AtomicUsize>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        std::thread::spawn(move || {
            let mut current = "p2prt_first_token_0000".to_string();
            let mut dead = false;
            for conn in l.incoming() {
                let Ok(mut c) = conn else { continue };
                // Read the head, then exactly Content-Length body bytes (a request can arrive in pieces).
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = c.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len = text[..i]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_string())
                            })
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(0);
                        if buf.len() >= i + 4 + len {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&buf).to_string();
                let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                let f: HashMap<String, String> = body
                    .split('&')
                    .filter_map(|p| p.split_once('='))
                    .map(|(k, v)| (k.to_string(), dec(v).unwrap()))
                    .collect();
                let (status, out) = if req.starts_with("POST /token")
                    && f.get("grant_type").map(String::as_str) == Some("refresh_token")
                {
                    refreshes.fetch_add(1, Ordering::SeqCst);
                    if dead || f.get("refresh_token") != Some(&current) {
                        dead = true;
                        (400, serde_json::json!({"error": "invalid_grant"}))
                    } else {
                        current = format!("p2prt_rotated_{:010}", refreshes.load(Ordering::SeqCst));
                        let aud = f.get("resource").cloned().unwrap_or_default();
                        (
                            200,
                            serde_json::json!({"refresh_token": current, "access_token": format!("eyJ.access.for.{aud}"),
                            "expires_in": 300, "scope": "chat projects:read", "audience": aud}),
                        )
                    }
                } else if req.starts_with("POST /revoke") {
                    dead = true;
                    (200, serde_json::json!({}))
                } else {
                    (404, serde_json::json!({}))
                };
                let b = out.to_string();
                let _ = write!(c, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
            }
        });
        base
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-acct-{tag}-{}", store::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn tokens_are_cached_rotation_is_stored_and_a_dead_session_signs_out() {
        let n = Arc::new(AtomicUsize::new(0));
        let base = fake_home(n.clone());
        let dir = tmp("rot");
        let acc = Accounts::new(&dir);
        let sec = Mem::default();
        let id = home_id(&base);
        acc.upsert(HomeRecord {
            id: id.clone(),
            issuer: base.clone(),
            email: "a@b.c".into(),
            password_change_required: false,
            signed_in_at: 1,
        })
        .unwrap();
        let link = AccountLink::new(&base, "gw-one").unwrap();
        assert_eq!(acc.token_for(&sec, &link), Err(AcctError::NotSignedIn));
        sec.set(&id, "p2prt_first_token_0000").unwrap();
        let t1 = acc.token_for(&sec, &link).unwrap();
        assert_eq!(t1, "eyJ.access.for.gw-one");
        // The rotated token replaced the first one in the keychain.
        assert_eq!(sec.get(&id).unwrap().as_deref(), Some("p2prt_rotated_0000000001"));
        // Cached: no second refresh.
        assert_eq!(acc.token_for(&sec, &link).unwrap(), t1);
        assert_eq!(n.load(Ordering::SeqCst), 1);
        // Another gateway: one more refresh with the rotated token.
        let link2 = AccountLink::new(&base, "gw-two").unwrap();
        assert_eq!(acc.token_for(&sec, &link2).unwrap(), "eyJ.access.for.gw-two");
        assert_eq!(n.load(Ordering::SeqCst), 2);
        // A stale token (as if copied by someone) ends the session; the app then shows "signed out".
        sec.set(&id, "p2prt_rotated_0000000001").unwrap();
        acc.clear_cache(&id);
        assert_eq!(acc.token_for(&sec, &link), Err(AcctError::SignedOut));
        assert_eq!(sec.get(&id).unwrap(), None);
        let v = acc.views(&sec).unwrap();
        assert_eq!((v.len(), v[0].signed_in, v[0].dev), (1, false, true));
        // Nothing secret in the file.
        let file = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(!file.contains("p2prt_") && !file.contains("eyJ"));
        acc.sign_out(&sec, &id).unwrap();
        assert!(acc.views(&sec).unwrap().is_empty());
    }

    #[test]
    fn an_edited_file_cannot_point_an_id_at_another_home() {
        let dir = tmp("edit");
        std::fs::write(
            dir.join(FILE),
            r#"{"homes":[{"id":"0123456789abcdef","issuer":"https://h.example","email":"","password_change_required":false,"signed_in_at":0}]}"#,
        )
        .unwrap();
        assert_eq!(Accounts::new(&dir).views(&Mem::default()), Err(AcctError::Store(StoreError::Bad)));
    }
}
