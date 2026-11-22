//! ext_net -- internet access for web extensions the person allowed (phase I6b, decision 2026-11-21 "Plus internet,
//! asked per extension"; docs/features/extensions.md).
//!
//! Purpose: web extensions run in a Web Worker whose Content-Security-Policy allows no network at all. When the
//! person has allowed internet for one extension, its `fetch` calls are carried to the Rust core and made here,
//! instead of opening the worker's CSP. This module checks each request and makes it.
//!
//! Inputs and trust boundaries: the request (method, URL, headers, body) is written by third-party extension code and
//! is untrusted. The extension id comes from the webview, which only forwards requests of extensions the person
//! allowed (checked again by the shell against the saved list).
//!
//! Security controls:
//!   * https only, TLS verified with the Mozilla roots, no proxy from the environment (CWE-319, CWE-295, NIST SC-8).
//!   * Public hosts only: no user info, localhost, `.local`/`.internal` names, private or special IPv4, IPv6
//!     literals or numeric IPv4 tricks in the URL, and [`crate::discovery::PublicOnly`] refuses names that resolve
//!     to a private address at connect time (CWE-918, DNS rebinding). So an extension can't reach the gateway, a
//!     local model, TandemForge, the router or cloud metadata.
//!   * Redirects followed by hand, at most 3, each one checked like the first (CWE-601).
//!   * Method allow-list; header names allow-listed; `Cookie`, `Host`, proxy and hop-by-hop headers dropped; no
//!     cookie jar; `Set-Cookie` never returned (CWE-20, CWE-113, CWE-565).
//!   * Caps: URL 4 KiB, 64 headers of 8 KiB, request body 4 MiB, response 16 MiB, 30 s per request (CWE-400).
//!   * Errors are fixed words (CWE-209). The caller audits the host only, never the path or query, which may hold
//!     an extension's tokens (CWE-532).
//!
//! Dependencies: ureq 3 and base64 0.22 (both already used), crate::discovery::PublicOnly.

use crate::discovery::{private_ip, PublicOnly};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

/// Longest URL accepted.
pub const MAX_URL: usize = 4096;
/// Most request headers.
pub const MAX_HEADERS: usize = 64;
/// Longest header value.
pub const MAX_HEADER_VALUE: usize = 8192;
/// Largest request body.
pub const MAX_REQUEST_BODY: usize = 4 * 1024 * 1024;
/// Largest response body.
pub const MAX_RESPONSE_BODY: u64 = 16 * 1024 * 1024;
/// Most redirects followed.
pub const MAX_REDIRECTS: usize = 3;

const METHODS: &[&str] = &["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
/// Request headers never passed on: the core sets them, or they would reach beyond this one request.
const DROPPED: &[&str] = &[
    "host",
    "cookie",
    "cookie2",
    "connection",
    "keep-alive",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "expect",
];

/// One request from an extension.
#[derive(Debug, Clone, Deserialize)]
pub struct NetRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Body bytes (base64 from the webview is decoded by the shell).
    #[serde(default, skip)]
    pub body: Vec<u8>,
}

/// The answer handed back to the extension.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NetResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    /// The final URL after redirects.
    pub url: String,
    #[serde(skip)]
    pub body: Vec<u8>,
}

/// Why a request was refused. Fixed words only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    /// The URL, method or a header is not allowed.
    NotAllowed,
    /// Too large (body, headers or answer).
    TooLarge,
    /// The site could not be reached, or TLS failed.
    Network,
}

impl NetError {
    pub fn message(self) -> &'static str {
        match self {
            NetError::NotAllowed => "extensions may reach public https sites only",
            NetError::TooLarge => "the request or the answer is too large",
            NetError::Network => "the site could not be reached",
        }
    }
}

/// The host of a URL this module accepts, lowercase; `None` when the URL is not https to a public host.
///
/// Security: deny by default; see the module header for every rule (CWE-918, CWE-601).
pub fn public_https_host(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > MAX_URL || raw.bytes().any(|b| b <= b' ' || b == 0x7f) {
        return None;
    }
    let rest = raw.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..end];
    // SECURITY: no user info ("good.com@evil") and no IPv6 literals ("[::1]") (CWE-601, CWE-918).
    if auth.is_empty() || auth.contains('@') || auth.contains('[') || auth.contains('\\') {
        return None;
    }
    let (host, port) = match auth.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (auth, None),
    };
    if host.is_empty() || host.len() > 253 || port.is_some_and(|p| p.parse::<u16>().map_or(true, |n| n == 0)) {
        return None;
    }
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    if !h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.') || h.starts_with('.') {
        return None;
    }
    if h == "localhost" || [".localhost", ".local", ".internal", ".home.arpa", ".lan"].iter().any(|s| h.ends_with(s)) {
        return None;
    }
    if let Ok(ip) = h.parse::<Ipv4Addr>() {
        return if private_ip(IpAddr::V4(ip)) { None } else { Some(h) };
    }
    // SECURITY: decimal, octal, hex or short IPv4 forms such as 2130706433, 0x7f.1 or 127.1 (CWE-918).
    if h.split('.').all(|part| part.bytes().all(|b| b.is_ascii_digit()) || part.starts_with("0x")) {
        return None;
    }
    if !h.contains('.') {
        return None; // a bare name resolves through the local search domain
    }
    Some(h)
}

/// Checks a header name: an HTTP token of 1-128 characters.
fn header_name_ok(n: &str) -> bool {
    !n.is_empty() && n.len() <= 128 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Checks the method, URL and headers, and returns the cleaned headers.
///
/// Security: header values may not hold CR, LF or NUL (CWE-113); dropped headers are listed in `DROPPED`.
pub fn check(req: &NetRequest) -> Result<(String, Vec<(String, String)>), NetError> {
    let method = req.method.to_ascii_uppercase();
    if !METHODS.contains(&method.as_str()) || public_https_host(&req.url).is_none() {
        return Err(NetError::NotAllowed);
    }
    if req.body.len() > MAX_REQUEST_BODY {
        return Err(NetError::TooLarge);
    }
    if req.headers.len() > MAX_HEADERS {
        return Err(NetError::TooLarge);
    }
    let mut out = Vec::new();
    for (n, v) in &req.headers {
        if !header_name_ok(n) || v.len() > MAX_HEADER_VALUE || v.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(NetError::NotAllowed);
        }
        let lower = n.to_ascii_lowercase();
        if DROPPED.contains(&lower.as_str()) || lower.starts_with("proxy-") || lower.starts_with("sec-") {
            continue;
        }
        out.push((lower, v.clone()));
    }
    Ok((method, out))
}

/// The network seam: makes one checked request (no redirects). Tests pass a fake; [`real_send`] is the real one.
pub type SendFn = dyn Fn(&str, &str, &[(String, String)], &[u8]) -> Result<NetResponse, NetError> + Sync;
/// A borrowed sender (it may borrow test state, hence the lifetime).
pub type Send<'a> = &'a (dyn Fn(&str, &str, &[Header], &[u8]) -> Result<NetResponse, NetError> + Sync + 'a);
/// One header: lowercase name and value.
pub type Header = (String, String);

/// Makes the request, following up to [`MAX_REDIRECTS`] redirects, each checked again.
///
/// A 303, or a 301/302 after a POST, continues as a GET without a body (as browsers do); 307 and 308 keep the
/// method and body. Returns the last answer.
pub fn fetch(req: &NetRequest, send: Send) -> Result<NetResponse, NetError> {
    let (mut method, headers) = check(req)?;
    let mut url = req.url.clone();
    let mut body = req.body.clone();
    for _ in 0..=MAX_REDIRECTS {
        if public_https_host(&url).is_none() {
            return Err(NetError::NotAllowed);
        }
        let resp = send(&method, &url, &headers, &body)?;
        if !matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
            return Ok(NetResponse { url, ..resp });
        }
        let Some(loc) = resp.headers.iter().find(|(n, _)| n == "location").map(|(_, v)| v.clone()) else {
            return Ok(NetResponse { url, ..resp });
        };
        url = resolve_location(&url, &loc).ok_or(NetError::NotAllowed)?;
        if resp.status == 303 || (matches!(resp.status, 301 | 302) && method == "POST") {
            method = "GET".into();
            body.clear();
        }
    }
    Err(NetError::Network)
}

/// A request as the webview sends it: the body is base64 (JSON has no bytes).
#[derive(Debug, Clone, Deserialize)]
pub struct NetCall {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body_b64: String,
}

/// The answer as the webview receives it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NetAnswer {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub url: String,
    pub body_b64: String,
}

/// Decodes the call, makes the request with [`fetch`] and encodes the answer.
///
/// Security: the encoded body is size-checked before it is decoded (CWE-400).
pub fn run_call(call: NetCall, send: Send) -> Result<NetAnswer, NetError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    if call.body_b64.len() > MAX_REQUEST_BODY / 3 * 4 + 8 {
        return Err(NetError::TooLarge);
    }
    let body = STANDARD.decode(call.body_b64.as_bytes()).map_err(|_| NetError::NotAllowed)?;
    let req = NetRequest { method: call.method, url: call.url, headers: call.headers, body };
    let r = fetch(&req, send)?;
    Ok(NetAnswer {
        status: r.status,
        status_text: r.status_text,
        headers: r.headers,
        url: r.url,
        body_b64: STANDARD.encode(&r.body),
    })
}

/// Resolves a `Location` header against the current URL: absolute https URLs, and paths starting with `/`.
fn resolve_location(current: &str, loc: &str) -> Option<String> {
    if loc.starts_with("https://") {
        return Some(loc.to_string());
    }
    if loc.starts_with('/') && !loc.starts_with("//") {
        let rest = current.strip_prefix("https://")?;
        let auth = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
        return Some(format!("https://{auth}{loc}"));
    }
    None
}

/// The real seam over ureq: https only, public addresses only, no proxy, no redirects (handled in [`fetch`]).
pub fn real_send() -> Box<SendFn> {
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .https_only(true)
        .http_status_as_error(false)
        .allow_non_standard_methods(false)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(30)))
        .max_response_header_size(64 * 1024)
        .user_agent("p2p-ai-ide-extension/0.1")
        .build();
    let agent = ureq::Agent::with_parts(config, ureq::unversioned::transport::DefaultConnector::default(), PublicOnly);
    Box::new(move |method: &str, url: &str, headers: &[(String, String)], body: &[u8]| {
        let mut b = ureq::http::Request::builder().method(method).uri(url);
        for (n, v) in headers {
            b = b.header(n.as_str(), v.as_str());
        }
        let sent = if body.is_empty() && matches!(method, "GET" | "HEAD" | "OPTIONS" | "DELETE") {
            agent.run(b.body(()).map_err(|_| NetError::NotAllowed)?)
        } else {
            agent.run(b.body(body.to_vec()).map_err(|_| NetError::NotAllowed)?)
        };
        let resp = sent.map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => NetError::TooLarge,
            _ => NetError::Network,
        })?;
        let status = resp.status();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            // SECURITY: no cookies back to extension code (CWE-565).
            .filter(|(n, _)| n.as_str() != "set-cookie" && n.as_str() != "set-cookie2")
            .filter_map(|(n, v)| v.to_str().ok().map(|v| (n.as_str().to_string(), v.to_string())))
            .collect();
        let mut out = vec![];
        resp.into_body()
            .into_reader()
            .take(MAX_RESPONSE_BODY + 1)
            .read_to_end(&mut out)
            .map_err(|_| NetError::Network)?;
        if out.len() as u64 > MAX_RESPONSE_BODY {
            return Err(NetError::TooLarge);
        }
        Ok(NetResponse {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            headers,
            url: url.to_string(),
            body: out,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn req(method: &str, url: &str) -> NetRequest {
        NetRequest { method: method.into(), url: url.into(), headers: vec![], body: vec![] }
    }

    #[test]
    fn only_public_https_hosts() {
        for ok in ["https://api.example.org/x?y=1", "https://example.org:8443/", "https://8.8.8.8/dns"] {
            assert!(public_https_host(ok).is_some(), "{ok}");
        }
        for bad in [
            "http://example.org/",
            "https://localhost/",
            "https://a.localhost/",
            "https://printer.local/",
            "https://db.internal/",
            "https://127.0.0.1/",
            "https://10.0.0.5/",
            "https://169.254.169.254/latest/meta-data",
            "https://192.168.1.1/",
            "https://[::1]/",
            "https://2130706433/",
            "https://127.1/",
            "https://0x7f.1/",
            "https://example.org@127.0.0.1/",
            "https://intranet/",
            "https://example.org:0/",
            "https://exa mple.org/",
            "https://example.org\\@evil.com/",
            "file:///etc/passwd",
            "",
        ] {
            assert!(public_https_host(bad).is_none(), "{bad}");
        }
        assert!(public_https_host(&format!("https://example.org/{}", "a".repeat(MAX_URL))).is_none());
    }

    #[test]
    fn methods_and_headers_are_checked() {
        assert_eq!(check(&req("TRACE", "https://example.org/")), Err(NetError::NotAllowed));
        assert_eq!(check(&req("CONNECT", "https://example.org/")), Err(NetError::NotAllowed));
        let mut r = req("post", "https://example.org/");
        r.headers = vec![
            ("Content-Type".into(), "application/json".into()),
            ("Cookie".into(), "a=b".into()),
            ("Host".into(), "evil".into()),
            ("Proxy-Authorization".into(), "x".into()),
            ("Authorization".into(), "Bearer token-of-the-extension".into()),
        ];
        let (m, h) = check(&r).unwrap();
        assert_eq!(m, "POST");
        let names: Vec<&str> = h.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["content-type", "authorization"]);
        r.headers = vec![("X-A".into(), "v\r\nInjected: 1".into())];
        assert_eq!(check(&r), Err(NetError::NotAllowed));
        r.headers = vec![("Bad Name".into(), "v".into())];
        assert_eq!(check(&r), Err(NetError::NotAllowed));
        r.headers = vec![];
        r.body = vec![0; MAX_REQUEST_BODY + 1];
        assert_eq!(check(&r), Err(NetError::TooLarge));
    }

    fn answer(status: u16, location: Option<&str>) -> NetResponse {
        NetResponse {
            status,
            status_text: String::new(),
            headers: location.map(|l| vec![("location".to_string(), l.to_string())]).unwrap_or_default(),
            url: String::new(),
            body: b"ok".to_vec(),
        }
    }

    #[test]
    fn calls_carry_bodies_as_base64() {
        let send = |m: &str, _: &str, _: &[(String, String)], b: &[u8]| {
            assert_eq!((m, b), ("POST", &b"hello"[..]));
            Ok(answer(200, None))
        };
        let call = NetCall {
            method: "POST".into(),
            url: "https://example.org/".into(),
            headers: vec![],
            body_b64: "aGVsbG8=".into(),
        };
        let a = run_call(call.clone(), &send).unwrap();
        assert_eq!((a.status, a.body_b64.as_str()), (200, "b2s="));
        let bad = NetCall { body_b64: "not base64!".into(), ..call.clone() };
        assert_eq!(run_call(bad, &send), Err(NetError::NotAllowed));
        let big = NetCall { body_b64: "A".repeat(MAX_REQUEST_BODY / 3 * 4 + 12), ..call };
        assert_eq!(run_call(big, &send), Err(NetError::TooLarge));
    }

    #[test]
    fn redirects_are_checked_each_time() {
        // A redirect to a private address is refused before it is followed.
        let seen = Mutex::new(vec![]);
        let send = |m: &str, u: &str, _: &[(String, String)], _: &[u8]| {
            seen.lock().unwrap().push(format!("{m} {u}"));
            Ok(if u.ends_with("/start") { answer(302, Some("https://127.0.0.1/admin")) } else { answer(200, None) })
        };
        assert_eq!(fetch(&req("GET", "https://example.org/start"), &send), Err(NetError::NotAllowed));
        assert_eq!(seen.lock().unwrap().len(), 1);

        // A relative redirect stays on the same host; POST after 303 becomes GET.
        let seen = Mutex::new(vec![]);
        let send = |m: &str, u: &str, _: &[(String, String)], b: &[u8]| {
            seen.lock().unwrap().push(format!("{m} {u} {}", b.len()));
            Ok(if u.ends_with("/form") { answer(303, Some("/done")) } else { answer(200, None) })
        };
        let mut r = req("POST", "https://example.org/form");
        r.body = b"x=1".to_vec();
        let got = fetch(&r, &send).unwrap();
        assert_eq!(got.url, "https://example.org/done");
        assert_eq!(*seen.lock().unwrap(), ["POST https://example.org/form 3", "GET https://example.org/done 0"]);

        // A loop stops after MAX_REDIRECTS.
        let send = |_: &str, _: &str, _: &[(String, String)], _: &[u8]| Ok(answer(307, Some("/again")));
        assert_eq!(fetch(&req("GET", "https://example.org/again"), &send), Err(NetError::Network));

        // A protocol-relative or http redirect is refused.
        let send = |_: &str, u: &str, _: &[(String, String)], _: &[u8]| {
            Ok(if u.ends_with("/a") { answer(301, Some("//evil.example/")) } else { answer(200, None) })
        };
        assert_eq!(fetch(&req("GET", "https://example.org/a"), &send), Err(NetError::NotAllowed));
        let send = |_: &str, u: &str, _: &[(String, String)], _: &[u8]| {
            Ok(if u.ends_with("/a") { answer(301, Some("http://example.org/")) } else { answer(200, None) })
        };
        assert_eq!(fetch(&req("GET", "https://example.org/a"), &send), Err(NetError::NotAllowed));
    }
}
