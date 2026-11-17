//! gwapi -- JSON calls, file upload and file download on the active gateway's project API (phase I4).
//!
//! Purpose: missions (Autopilot and Auto Improve) run on a Go gateway, on a project that holds a copy of the open
//! folder. This module adds the three request shapes the IDE needs for that on top of [`Client`]:
//!
//! | Call | Request |
//! |---|---|
//! | [`Client::api`] | any `GET`/`POST`/`PUT` on a `/v1/...` path built by this crate, JSON in and out |
//! | [`Client::upload_file`] | `POST /v1/files` as `multipart/form-data` (one file) |
//! | [`Client::download`] | `GET` of a workspace file with `?download=1` |
//!
//! Inputs and trust boundaries: paths are built by `projects.rs` from checked ids and percent-encoded workspace
//! paths, never taken from the webview as a whole URL (CWE-918). Every gateway answer is untrusted: size-capped,
//! parsed as JSON into `Value` and then into fixed view shapes by `projects.rs`.
//!
//! Security controls:
//!   * Same transport as chat: TLS (pinned CA when saved), no redirects, no proxy, key only in `Authorization`
//!     (see client.rs; CWE-295, CWE-601, CWE-522).
//!   * Error text: the gateway's own message is never shown; its `code` is looked up in a fixed table and anything
//!     else becomes a status-only message (CWE-209, CWE-117).
//!   * Caps: JSON answers 2 MiB, JSON request bodies 1 MiB, uploads and downloads 5 MiB (CWE-400, CWE-770).
//!   * Multipart: a random boundary from the OS CSPRNG, and a file name allow-list so a name cannot break out of
//!     the header (CWE-93, CWE-113).
//!
//! Dependencies: ureq 3, serde_json, getrandom (all already used by the crate).

use crate::client::{check_status, map_io, map_ureq, Client, NetError};
use serde_json::Value;
use std::io::Read;

/// Largest JSON answer read.
pub const MAX_JSON_ANSWER: u64 = 2 << 20;
/// Largest JSON request body sent.
pub const MAX_JSON_BODY: usize = 1 << 20;
/// Largest file uploaded or downloaded.
pub const MAX_TRANSFER_BYTES: usize = 5 << 20;

/// Maps a gateway error `code` (from `{"error": {"code": ...}}`) to a fixed message. Unknown codes return `None`
/// and the caller falls back to the HTTP status. The table lists the Go gateway's codes for projects, workspace
/// sync, missions and approvals (P2P-AI `missions_api.go`, `projects_api.go`, `node_workspace*.go`).
pub fn code_message(code: &str) -> Option<&'static str> {
    Some(match code {
        "version_conflict" => "the gateway project changed meanwhile; try again",
        "project_toggle_off" => "the project owner has not switched this on for the project",
        "experimental_disabled" => "the gateway's administrator has switched missions off",
        "not_available" => "this gateway does not offer that feature",
        "exec_not_allowed" => "this gateway does not run code for your key, so Autopilot is not available",
        "mission_exists" => "this project already has a mission of this kind; stop it first",
        "mission_limit" => "the most missions allowed are already running; try later",
        "invalid_state" => "the mission cannot do that in its current state",
        "attachment_not_ready" => "a file is still being scanned on the gateway; try again shortly",
        "quota_exceeded" => "the project owner's storage quota on the gateway is used up",
        "unsupported_media_type" => "the gateway does not accept this file type",
        "too_large" => "too large for the gateway",
        "sync_in_progress" => "a copy of the project to a node is still running",
        "delta_in_progress" => "the node's changes are already being collected",
        "no_copy" => "no node holds a copy of this project yet; send the folder first",
        "copy_stale" => "the node that held the copy left; send the folder again",
        "delta_unsupported" => "the node that holds the copy cannot send changes back",
        "no_capacity" => "no node can take a copy of this project right now",
        "busy" => "the gateway is busy; try again shortly",
        "service_unavailable" => "this gateway has that feature switched off",
        "approval_not_pending" => "that approval was already decided or has expired",
        "arguments_mismatch" => "the approval changed; open it again",
        "auto_approve_not_allowed" => "this gateway does not allow auto-approve",
        "assist_model_not_allowed" => "this gateway does not allow that assist model",
        "limit_reached" => "your key has reached the gateway's project limit",
        "forbidden" => "your role in this project does not allow that",
        "not_found" => "not found on the gateway (it may have been deleted)",
        "invalid_request" => "the gateway refused the request as invalid",
        _ => return None,
    })
}

/// Turns a non-2xx answer into an error: a known code gives its fixed message, else the status mapping.
fn answer_error(status: u16, body: &[u8]) -> NetError {
    let code = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.get("code")).and_then(Value::as_str).map(str::to_string));
    if let Some(m) = code.as_deref().and_then(code_message) {
        // 401/403 for the key itself stays "refused the key"; role refusals have their own codes.
        if status != 401 {
            return NetError::Gateway(m);
        }
    }
    check_status(status).err().unwrap_or(NetError::Status(status))
}

/// Reads a whole body up to `max` bytes; more is `TooLarge`.
fn read_capped(r: impl Read, max: u64) -> Result<Vec<u8>, NetError> {
    let mut out = Vec::new();
    r.take(max + 1).read_to_end(&mut out).map_err(map_io)?;
    if out.len() as u64 > max {
        return Err(NetError::TooLarge);
    }
    Ok(out)
}

/// Percent-encodes one workspace path for a URL: unreserved characters and `/` stay, everything else is `%XX`.
///
/// Security: the path was checked against the workspace grammar first; encoding keeps `?`, `#`, `%` and spaces
/// from changing the request (CWE-74).
pub fn encode_path(rel: &str) -> String {
    let mut out = String::with_capacity(rel.len());
    for b in rel.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A file name that is safe inside a multipart header: the last path part, 1-200 bytes of letters, digits and
/// `-_.()+@,=~ `. Returns `None` otherwise (the caller skips the file with a reason).
pub fn upload_name(rel: &str) -> Option<&str> {
    let name = rel.rsplit('/').next()?;
    let ok = !name.is_empty()
        && name.len() <= 200
        && name.chars().all(|c| c.is_alphanumeric() || " -_.()+@,=~".contains(c))
        && !name.starts_with(' ');
    ok.then_some(name)
}

impl Client {
    /// Sends one JSON request to a `/v1/` path and returns the parsed answer (`Value::Null` for an empty body).
    ///
    /// * `method` -- `GET`, `POST`, `PUT` or `DELETE`.
    /// * `path` -- starts with `/v1/`, built by this crate (never a URL from the webview).
    /// * `body` -- JSON for `POST`/`PUT`; at most 1 MiB.
    ///
    /// Security: path prefix and characters are checked so a value cannot add a host or query (CWE-918, CWE-74).
    pub fn api(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, NetError> {
        self.api_on(&self.short, method, path, body)
    }

    /// As [`Client::api`], with the long timeout (10 minutes) for calls that run work, such as a sandbox command
    /// (phase I5). Same path, size and error rules.
    pub fn api_slow(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, NetError> {
        self.api_on(&self.long, method, path, body)
    }

    fn api_on(&self, agent: &ureq::Agent, method: &str, path: &str, body: Option<&Value>) -> Result<Value, NetError> {
        if !path.starts_with("/v1/") || path.contains("..") || path.chars().any(|c| c.is_control() || c == '#') {
            return Err(NetError::BadRequest("internal: bad gateway path"));
        }
        let url = format!("{}{}", self.base, path);
        let resp = match (method, body) {
            ("GET", None) => agent.get(&url).header("Authorization", self.auth()).call(),
            ("DELETE", None) => agent.delete(&url).header("Authorization", self.auth()).call(),
            ("POST" | "PUT", b) => {
                let text = b.map(Value::to_string).unwrap_or_else(|| "{}".into());
                if text.len() > MAX_JSON_BODY {
                    return Err(NetError::BadRequest("the request is too large"));
                }
                let req = if method == "POST" { agent.post(&url) } else { agent.put(&url) };
                req.header("Authorization", self.auth())
                    .header("Content-Type", "application/json")
                    .send(text.as_bytes())
            }
            _ => return Err(NetError::BadRequest("internal: bad method")),
        }
        .map_err(map_ureq)?;
        let status = resp.status().as_u16();
        let bytes = read_capped(resp.into_body().into_reader(), MAX_JSON_ANSWER)?;
        if !(200..300).contains(&status) {
            return Err(answer_error(status, &bytes));
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| NetError::BadAnswer)
    }

    /// Uploads one file to `POST /v1/files` and returns the gateway's file object (`id`, `state`, ...).
    ///
    /// Security: `name` must pass [`upload_name`]; the boundary is 32 random hex characters checked not to occur
    /// in the bytes (CWE-93); the body is capped at 5 MiB plus framing.
    pub fn upload_file(&self, name: &str, bytes: &[u8]) -> Result<Value, NetError> {
        if upload_name(name) != Some(name) {
            return Err(NetError::BadRequest("this file name cannot be sent to the gateway"));
        }
        if bytes.len() > MAX_TRANSFER_BYTES {
            return Err(NetError::BadRequest("files over 5 MiB are not sent"));
        }
        let boundary = loop {
            let mut r = [0u8; 16];
            getrandom::getrandom(&mut r).map_err(|_| NetError::BadRequest("no random source"))?;
            let b = format!("p2pide{}", r.iter().map(|x| format!("{x:02x}")).collect::<String>());
            if !bytes.windows(b.len()).any(|w| w == b.as_bytes()) {
                break b;
            }
        };
        let mut body = Vec::with_capacity(bytes.len() + 512);
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"purpose\"\r\n\r\nworkspace\r\n").as_bytes(),
        );
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let resp = self
            .long
            .post(format!("{}/v1/files", self.base))
            .header("Authorization", self.auth())
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
            .send(&body[..])
            .map_err(map_ureq)?;
        let status = resp.status().as_u16();
        let answer = read_capped(resp.into_body().into_reader(), MAX_JSON_ANSWER)?;
        if !(200..300).contains(&status) {
            return Err(answer_error(status, &answer));
        }
        serde_json::from_slice(&answer).map_err(|_| NetError::BadAnswer)
    }

    /// Downloads bytes from a `/v1/` path (a workspace file with `?download=1`), at most 5 MiB.
    pub fn download(&self, path: &str) -> Result<Vec<u8>, NetError> {
        if !path.starts_with("/v1/") || path.contains("..") || path.chars().any(|c| c.is_control() || c == '#') {
            return Err(NetError::BadRequest("internal: bad gateway path"));
        }
        let resp = self
            .long
            .get(format!("{}{}", self.base, path))
            .header("Authorization", self.auth())
            .call()
            .map_err(map_ureq)?;
        let status = resp.status().as_u16();
        let bytes = read_capped(resp.into_body().into_reader(), MAX_TRANSFER_BYTES as u64)?;
        if !(200..300).contains(&status) {
            return Err(answer_error(status, &bytes));
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_encoded_and_names_checked() {
        assert_eq!(encode_path("src/a b#?.py"), "src/a%20b%23%3F.py");
        assert_eq!(encode_path("docs/é.md"), "docs/%C3%A9.md");
        assert_eq!(upload_name("src/main.py"), Some("main.py"));
        assert_eq!(upload_name("a/my notes (1).md"), Some("my notes (1).md"));
        assert_eq!(upload_name("a/x\".py"), None);
        assert_eq!(upload_name("a/x\r\nInjected: 1"), None);
        assert_eq!(upload_name("a/"), None);
    }

    #[test]
    fn errors_use_fixed_messages_only() {
        let body = br#"{"error":{"message":"<img src=x> secret detail","code":"version_conflict"}}"#;
        let e = answer_error(409, body);
        assert_eq!(e, NetError::Gateway("the gateway project changed meanwhile; try again"));
        assert!(!e.message().contains("secret"));
        assert_eq!(answer_error(500, br#"{"error":{"code":"weird_new_code"}}"#), NetError::Status(500));
        assert_eq!(answer_error(401, br#"{"error":{"code":"forbidden"}}"#), NetError::Auth);
        assert_eq!(answer_error(404, b"not json"), NetError::NotOffered);
    }
}
