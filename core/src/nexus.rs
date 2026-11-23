//! nexus -- discovery and health probing for a NEXUS Engine running on this computer.
//!
//! Purpose: NEXUS Engine runs two optional local HTTP servers on `127.0.0.1` without API keys:
//!   - `nexus-proxy` (default port `11435`): Context Packer, Data Airlock, Failure Memory, and tool-enabled
//!     OpenAI `/v1/chat/completions` forwarding.
//!   - `nexus-runtime` (default port `11436`): native `.nexus` / Mamba-hybrid / Fourth Model Mode inference
//!     on `/v1/models` and `/v1/chat/completions` (chat-only; no OpenAI tool-call parsing).
//!
//! If NEXUS Studio has saved custom loopback ports in `~/.nexus-studio/studio-settings.json`, this module reads
//! that file read-only to find the configured ports and falls back to the defaults (`11435` / `11436`).
//!
//! Inputs and trust boundaries:
//!   - `~/.nexus-studio/studio-settings.json`: untrusted file outside our app-data folder; read bounded via
//!     [`crate::store::read_capped`] refusing symlinks (CWE-59, CWE-400), accepting loopback URLs only.
//!   - Local HTTP responses (`/health`, `/nexus/config`, `/nexus/state/summary`): bounded, no redirects, no proxy,
//!     and untrusted string fields are sanitized to short ASCII telemetry values (CWE-20, CWE-79, CWE-400).
//!
//! Security controls:
//!   * Loopback only (`127.0.0.1` unprivileged ports `1024..=65535`): refuses remote hosts, paths, queries, and
//!     user info so neither settings nor UI input can steer traffic off-box (NIST SC-7, CWE-918, CWE-20).
//!   * No redirects (`max_redirects(0)`) and no environment proxies (CWE-601, CWE-918).
//!   * Fixed error messages containing no file paths, URLs, or response content (CWE-209).
//!
//! Dependencies: std, serde, serde_json, ureq 3, crate::store (no new crates).

use crate::store;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default port for `nexus-proxy`.
pub const DEFAULT_PROXY_PORT: u16 = 11435;
/// Default port for `nexus-runtime`.
pub const DEFAULT_RUNTIME_PORT: u16 = 11436;
/// Settings file written by `nexus-desktop` inside `~/.nexus-studio/`.
const SETTINGS_DIR: &str = ".nexus-studio";
const SETTINGS_FILE: &str = "studio-settings.json";
/// Largest `studio-settings.json` read (64 KiB, CWE-400).
const MAX_SETTINGS_BYTES: u64 = 64 << 10;
/// Largest probe response body read from `/health` or `/nexus/*` (64 KiB, CWE-400).
const MAX_PROBE_BYTES: u64 = 64 << 10;
/// Probe timeout so offline ports fail fast without stalling the Gateways view.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Which NEXUS Engine component is being probed or connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceKind {
    #[serde(rename = "nexus-proxy")]
    Proxy,
    #[serde(rename = "nexus-runtime")]
    Runtime,
}

impl ServiceKind {
    /// Service string advertised in `GET /health` `"service"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            ServiceKind::Proxy => "nexus-proxy",
            ServiceKind::Runtime => "nexus-runtime",
        }
    }

    /// Default loopback port when no custom port is configured.
    pub const fn default_port(self) -> u16 {
        match self {
            ServiceKind::Proxy => DEFAULT_PROXY_PORT,
            ServiceKind::Runtime => DEFAULT_RUNTIME_PORT,
        }
    }

    /// Display name saved in the IDE's gateway list.
    pub fn display_name(self, url: &str) -> String {
        let base = match self {
            ServiceKind::Proxy => "NEXUS Proxy",
            ServiceKind::Runtime => "NEXUS Runtime",
        };
        let default_url = format!("http://127.0.0.1:{}", self.default_port());
        if url == default_url {
            format!("{base} (this computer)")
        } else if let Some(port) = url.rsplit(':').next() {
            format!("{base} (this computer :{port})")
        } else {
            format!("{base} (this computer)")
        }
    }
}

/// Safe, fixed errors returned during NEXUS Engine discovery or probing (CWE-209).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NexusError {
    BadPort,
    BadUrl,
    NotRunning,
    UnexpectedService,
    BadAnswer,
}

impl NexusError {
    /// Short person-facing message containing no paths, URLs, or response text (CWE-209).
    pub const fn message(self) -> &'static str {
        match self {
            NexusError::BadPort => "pick an unprivileged port between 1024 and 65535",
            NexusError::BadUrl => "NEXUS Engine addresses must be http://127.0.0.1:<port> with no path",
            NexusError::NotRunning => "this NEXUS Engine service is not responding on that local port",
            NexusError::UnexpectedService => {
                "something else is listening on that port instead of the selected NEXUS Engine service"
            }
            NexusError::BadAnswer => "the NEXUS Engine status response could not be read",
        }
    }
}

/// Checks an unprivileged local TCP port (`1024..=65535`, CWE-20).
pub fn check_port(port: u16) -> Result<u16, NexusError> {
    if port < 1024 {
        return Err(NexusError::BadPort);
    }
    Ok(port)
}

/// Builds the canonical loopback base URL for a NEXUS Engine port.
pub fn url_for(port: u16) -> Result<String, NexusError> {
    check_port(port)?;
    Ok(format!("http://127.0.0.1:{port}"))
}

/// True only for exact `http://127.0.0.1:<port>` where `<port>` is canonical digits in `1024..=65535`.
///
/// SECURITY: guarantees saved `Kind::Nexus` entries never point off-box or include paths/userinfo (NIST SC-7, CWE-918).
pub fn is_engine_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    if rest.is_empty() || rest.starts_with('0') || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    rest.parse::<u16>().is_ok_and(|p| p >= 1024)
}

/// Validates and normalizes an untrusted URL from `studio-settings.json` (`proxyUrl` / `runtimeUrl`).
///
/// Accepts loopback HTTP hosts (`127.0.0.1`, `localhost`, `[::1]`) with an explicit unprivileged port
/// and no path/query/fragment/userinfo, normalizing to `http://127.0.0.1:<port>`.
pub fn parse_settings_url(raw: &str) -> Option<String> {
    let s = raw.trim().strip_suffix('/').unwrap_or(raw.trim());
    let (scheme, rest) = s.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    if rest.contains(['@', '/', '?', '#']) {
        return None;
    }
    let port_str = if let Some(after) = rest.strip_prefix("[::1]:") {
        after
    } else if let Some((host, p)) = rest.split_once(':') {
        if !host.eq_ignore_ascii_case("127.0.0.1") && !host.eq_ignore_ascii_case("localhost") {
            return None;
        }
        p
    } else {
        return None;
    };
    if port_str.is_empty() || port_str.starts_with('0') || !port_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port = port_str.parse::<u16>().ok()?;
    url_for(port).ok()
}

/// Configured loopback endpoints discovered from `~/.nexus-studio/studio-settings.json` or defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredEndpoints {
    pub proxy_url: String,
    pub runtime_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StudioSettings {
    proxy_url: Option<String>,
    runtime_url: Option<String>,
}

/// Discovers NEXUS Proxy & Runtime endpoints for the current user, falling back to `11435` / `11436`.
pub fn discover_endpoints() -> DiscoveredEndpoints {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    discover_endpoints_in(home.as_deref())
}

/// Testable core of [`discover_endpoints`] accepting an explicit home directory.
pub fn discover_endpoints_in(home: Option<&Path>) -> DiscoveredEndpoints {
    let mut out = DiscoveredEndpoints {
        proxy_url: format!("http://127.0.0.1:{DEFAULT_PROXY_PORT}"),
        runtime_url: format!("http://127.0.0.1:{DEFAULT_RUNTIME_PORT}"),
    };
    let Some(home) = home else {
        return out;
    };
    let path = home.join(SETTINGS_DIR).join(SETTINGS_FILE);
    // SECURITY: bounded read refusing symlinks (CWE-59, CWE-400).
    let Ok(bytes) = store::read_capped(&path, MAX_SETTINGS_BYTES) else {
        return out;
    };
    let Ok(cfg) = serde_json::from_slice::<StudioSettings>(&bytes) else {
        return out;
    };
    if let Some(url) = cfg.proxy_url.as_deref().and_then(parse_settings_url) {
        out.proxy_url = url;
    }
    if let Some(url) = cfg.runtime_url.as_deref().and_then(parse_settings_url) {
        out.runtime_url = url;
    }
    out
}

/// Sanitizes an optional short ASCII telemetry tag (model mode, SIMD backend) from local engine JSON.
fn clean_tag(s: Option<&str>, max: usize) -> Option<String> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    let clean: String = s.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(max).collect();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

/// Live health and telemetry summary from one local NEXUS Engine service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProbeResult {
    pub service: ServiceKind,
    pub url: String,
    pub running: bool,
    pub mock_upstream: Option<bool>,
    pub airlock_threshold: Option<usize>,
    pub active_model_mode: Option<String>,
    pub artifacts: Option<u64>,
    pub episodic_memories: Option<u64>,
    pub models_loaded: Option<usize>,
    pub models_available: Option<usize>,
    pub simd: Option<String>,
}

impl ProbeResult {
    fn offline(service: ServiceKind, url: String) -> Self {
        Self {
            service,
            url,
            running: false,
            mock_upstream: None,
            airlock_threshold: None,
            active_model_mode: None,
            artifacts: None,
            episodic_memories: None,
            models_loaded: None,
            models_available: None,
            simd: None,
        }
    }
}

fn probe_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(PROBE_TIMEOUT))
        .timeout_connect(Some(Duration::from_secs(1)))
        // SECURITY: never follow redirects or send local traffic via an env proxy (CWE-601, CWE-918).
        .max_redirects(0)
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .into()
}

fn get_json_capped(agent: &ureq::Agent, url: &str) -> Result<Value, NexusError> {
    use std::io::Read;
    let resp = agent.get(url).call().map_err(|_| NexusError::NotRunning)?;
    if !(200..300).contains(&resp.status().as_u16()) {
        return Err(NexusError::BadAnswer);
    }
    let mut bytes = Vec::new();
    resp.into_body()
        .into_reader()
        .take(MAX_PROBE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| NexusError::BadAnswer)?;
    if bytes.len() as u64 > MAX_PROBE_BYTES {
        return Err(NexusError::BadAnswer);
    }
    serde_json::from_slice::<Value>(&bytes).map_err(|_| NexusError::BadAnswer)
}

/// Probes `url` for a running NEXUS Engine service matching `expected`.
///
/// Security: rejects any non-loopback URL before issuing network I/O (CWE-918, NIST SC-7).
pub fn probe_endpoint(url: &str, expected: ServiceKind) -> Result<ProbeResult, NexusError> {
    if !is_engine_url(url) {
        return Err(NexusError::BadUrl);
    }
    let agent = probe_agent();
    let health = get_json_capped(&agent, &format!("{url}/health"))?;
    let status = health.get("status").and_then(Value::as_str);
    let svc = health.get("service").and_then(Value::as_str);
    if status != Some("ok") {
        return Err(NexusError::BadAnswer);
    }
    match svc {
        Some(s) if s == expected.as_str() => {}
        Some("nexus-proxy" | "nexus-runtime") => return Err(NexusError::UnexpectedService),
        _ => return Err(NexusError::UnexpectedService),
    }

    let mut out = ProbeResult::offline(expected, url.to_string());
    out.running = true;

    match expected {
        ServiceKind::Runtime => {
            out.models_loaded = health.get("models_loaded").and_then(Value::as_u64).map(|n| n as usize);
            out.models_available = health.get("models_available").and_then(Value::as_u64).map(|n| n as usize);
            out.simd = clean_tag(health.get("simd").and_then(Value::as_str), 32);
        }
        ServiceKind::Proxy => {
            if let Ok(cfg) = get_json_capped(&agent, &format!("{url}/nexus/config")) {
                out.mock_upstream = cfg.pointer("/startup/mock_ollama").and_then(Value::as_bool);
                out.airlock_threshold =
                    cfg.pointer("/tuning/airlock_threshold").and_then(Value::as_u64).map(|n| n as usize);
                out.active_model_mode = clean_tag(cfg.pointer("/tuning/active_model_mode").and_then(Value::as_str), 32);
            }
            if let Ok(sum) = get_json_capped(&agent, &format!("{url}/nexus/state/summary")) {
                out.artifacts = sum.get("artifacts").and_then(Value::as_u64);
                out.episodic_memories = sum.get("episodic_memories").and_then(Value::as_u64);
            }
        }
    }

    Ok(out)
}

/// Probes both configured/default NEXUS Engine ports and returns their current status.
pub fn status() -> (ProbeResult, ProbeResult) {
    let ep = discover_endpoints();
    let proxy = probe_endpoint(&ep.proxy_url, ServiceKind::Proxy)
        .unwrap_or_else(|_| ProbeResult::offline(ServiceKind::Proxy, ep.proxy_url.clone()));
    let runtime = probe_endpoint(&ep.runtime_url, ServiceKind::Runtime)
        .unwrap_or_else(|_| ProbeResult::offline(ServiceKind::Runtime, ep.runtime_url.clone()));
    (proxy, runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn ports_and_engine_urls_are_checked() {
        assert_eq!(url_for(11435).as_deref(), Ok("http://127.0.0.1:11435"));
        assert_eq!(url_for(80), Err(NexusError::BadPort));
        assert_eq!(url_for(1023), Err(NexusError::BadPort));
        assert!(is_engine_url("http://127.0.0.1:11435"));
        assert!(is_engine_url("http://127.0.0.1:1024"));
        for bad in [
            "https://127.0.0.1:11435",
            "http://127.0.0.1:80",
            "http://127.0.0.1:011435",
            "http://127.0.0.1:11435/",
            "http://127.0.0.1:11435/v1",
            "http://127.0.0.1:11435?a=1",
            "http://user@127.0.0.1:11435",
            "http://10.0.0.5:11435",
            "http://localhost:11435",
        ] {
            assert!(!is_engine_url(bad), "{bad}");
        }
    }

    #[test]
    fn studio_settings_normalizes_loopback_and_refuses_links_and_remotes() {
        assert_eq!(parse_settings_url("http://127.0.0.1:12435/").as_deref(), Some("http://127.0.0.1:12435"));
        assert_eq!(parse_settings_url("HTTP://Localhost:12436").as_deref(), Some("http://127.0.0.1:12436"));
        assert_eq!(parse_settings_url("http://[::1]:12435").as_deref(), Some("http://127.0.0.1:12435"));
        for bad in [
            "https://127.0.0.1:11435",
            "http://example.org:11435",
            "http://127.0.0.1:11435/api",
            "http://user@127.0.0.1:11435",
            "http://127.0.0.1:80",
            "http://127.0.0.1:011435",
        ] {
            assert_eq!(parse_settings_url(bad), None, "{bad}");
        }

        let dir = std::env::temp_dir().join(format!("p2pide-nexus-{}", store::new_id()));
        let cfg_dir = dir.join(SETTINGS_DIR);
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let file = cfg_dir.join(SETTINGS_FILE);
        std::fs::write(&file, br#"{"proxyUrl":"http://localhost:12435","runtimeUrl":"https://evil.example.com:443"}"#)
            .unwrap();
        let ep = discover_endpoints_in(Some(&dir));
        assert_eq!(ep.proxy_url, "http://127.0.0.1:12435");
        assert_eq!(ep.runtime_url, "http://127.0.0.1:11436");

        #[cfg(unix)]
        {
            let real = dir.join("other.json");
            std::fs::write(&real, br#"{"proxyUrl":"http://127.0.0.1:15000"}"#).unwrap();
            std::fs::remove_file(&file).unwrap();
            std::os::unix::fs::symlink(&real, &file).unwrap();
            let linked = discover_endpoints_in(Some(&dir));
            assert_eq!(linked.proxy_url, "http://127.0.0.1:11435");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn serve_responses(routes: &'static [(&'static str, &'static str)]) -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            for _ in 0..routes.len() {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/");
                let body = routes.iter().find(|(p, _)| *p == path).map(|(_, b)| *b);
                let resp = match body {
                    Some(b) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        b.len(),
                        b
                    ),
                    None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                };
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (port, handle)
    }

    #[test]
    fn probes_proxy_and_runtime_and_refuses_wrong_service() {
        assert_eq!(probe_endpoint("https://127.0.0.1:11435", ServiceKind::Proxy), Err(NexusError::BadUrl));

        let (p_port, h1) = serve_responses(&[
            ("/health", r#"{"status":"ok","service":"nexus-proxy"}"#),
            (
                "/nexus/config",
                r#"{"ok":true,"startup":{"mock_ollama":false},"tuning":{"airlock_threshold":250,"active_model_mode":"standard_proxy"}}"#,
            ),
            ("/nexus/state/summary", r#"{"artifacts":4,"episodic_memories":7}"#),
        ]);
        let proxy = probe_endpoint(&url_for(p_port).unwrap(), ServiceKind::Proxy).unwrap();
        assert!(proxy.running);
        assert_eq!(proxy.mock_upstream, Some(false));
        assert_eq!(proxy.airlock_threshold, Some(250));
        assert_eq!(proxy.active_model_mode.as_deref(), Some("standard_proxy"));
        assert_eq!(proxy.artifacts, Some(4));
        assert_eq!(proxy.episodic_memories, Some(7));
        h1.join().unwrap();

        let (r_port, h2) = serve_responses(&[(
            "/health",
            r#"{"status":"ok","service":"nexus-runtime","models_loaded":1,"models_available":3,"simd":"avx2\n<script>"}"#,
        )]);
        let runtime = probe_endpoint(&url_for(r_port).unwrap(), ServiceKind::Runtime).unwrap();
        assert!(runtime.running);
        assert_eq!(runtime.models_loaded, Some(1));
        assert_eq!(runtime.models_available, Some(3));
        assert_eq!(runtime.simd.as_deref(), Some("avx2<script>"));
        h2.join().unwrap();

        let (w_port, h3) = serve_responses(&[("/health", r#"{"status":"ok","service":"nexus-proxy"}"#)]);
        assert_eq!(probe_endpoint(&url_for(w_port).unwrap(), ServiceKind::Runtime), Err(NexusError::UnexpectedService));
        h3.join().unwrap();
    }
}
