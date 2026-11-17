//! discovery -- the public gateway list (phase I2b, docs/features/public-gateways.md).
//!
//! Purpose: the IDE can list gateways that publish themselves for users. It fetches signed gateway advertisements
//! from the directories the person chose (`GET <directory>/v1/adverts`), verifies each one exactly as the P2P-AI
//! node does, keeps only adverts that carry a valid `client_access` block (ADR-028: the gateway serves people, not
//! only nodes), measures latency with the signed probe, and returns a list. Picking an entry adds it as a saved
//! gateway; listing gives no trust and sends no key.
//!
//! Format source of truth: P2P-AI `decentralized/protocol` (TypeScript). This file is a port of the node's
//! `P2P-AI-Nodes/Rust/src/discovery.rs` verifier and is tested against a copy of the shared vectors
//! (`core/tests/vectors/discovery-vectors.json`, provenance in docs/files/discovery-vectors.md).
//!
//! Inputs and trust boundaries: directory URLs come from the person (checked here); directory answers, adverts and
//! probe replies are untrusted network input. Nothing from an advert is run, rendered as HTML or used as a path.
//!
//! Security controls:
//!   * Signature verified before time and before anything is listed: ed25519 over `P2PAD-V1\n` + canonical JSON
//!     without `sig`; `gateway_id` must be the first 40 hex of SHA-256 of the key (CWE-347, CWE-345).
//!   * Strict JSON: duplicate keys, floats, big integers and deep nesting are refused, so two parsers cannot read
//!     one advert differently (CWE-694, CWE-20). Fixed field allow-list; closed value sets.
//!   * Every URL is `https://` to a public host: no user info, localhost, private, link-local, metadata or IPv6
//!     literals; relay names only where the protocol allows them (CWE-918, CWE-601).
//!   * Connections go through [`PublicOnly`], a resolver that drops private addresses, so a public name that
//!     resolves to a private address is refused at connect time (DNS rebinding, CWE-918). No redirects, no proxy
//!     from the environment, TLS verification on with the Mozilla roots (CWE-295, NIST SC-8).
//!   * Caps: 8 directories, 2 MiB and 500 adverts per answer, 4 KiB per advert, 24 h advert lifetime, 120 s clock
//!     skew, 100 listed gateways, 2 KiB probe replies, 10 s per request (CWE-400, CWE-770).
//!   * Probe nonces come from the OS CSPRNG (getrandom) and replies must echo them and verify under the advert's
//!     key, so a replayed or forged reply never yields a latency (CWE-294, CWE-330).
//!   * Rejections are counted by fixed reason code, never by advert text (CWE-117, CWE-209).
//!
//! Dependencies: ed25519-dalek 2 (signature check, the same crate and version the node uses), base64 0.22,
//! getrandom 0.2 (already in the tree through ring), sha2, serde_json, ureq 3.

use crate::store::{self, StoreError};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Most directories the person can list.
pub const MAX_DIRS: usize = 8;
/// Most adverts accepted from one directory answer.
const MAX_ADVERTS: usize = 500;
/// Largest directory answer read.
const MAX_LIST_BYTES: u64 = 2 << 20;
/// Largest probe reply read.
const PROBE_MAX_BYTES: u64 = 2048;
/// Largest single advert.
const AD_MAX_BYTES: usize = 4096;
/// Longest advert lifetime (expires_at - issued_at).
const AD_MAX_LIFETIME: i64 = 86400;
/// Allowed clock difference for issued_at and probe timestamps.
const AD_MAX_SKEW: i64 = 120;
/// Most gateways shown (and probed) in one listing.
pub const MAX_LISTED: usize = 100;
/// Probes in flight at once.
const PROBE_PARALLEL: usize = 16;
const SIG_PREFIX: &str = "P2PAD-V1\n";
const PROBE_PREFIX: &str = "P2PPROBE-V1\n";
/// The relay name suffix (ADR-026): a host `<gateway_id>.reach.p2p-ai.invalid` means "through a relay".
const REACH_SUFFIX: &str = ".reach.p2p-ai.invalid";

// ------------------------------------------------------------------ canonical JSON (protocol/src/canonical.ts)

fn canon_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Canonical JSON: keys sorted by UTF-8 bytes, safe integers only, JavaScript-style string escapes, depth <= 6.
/// This is the exact byte string the gateway signed.
pub fn canon_json(v: &Value, depth: usize) -> Result<String, &'static str> {
    if depth > 6 {
        return Err("too deep");
    }
    Ok(match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_i64() {
            Some(i) if i.unsigned_abs() <= 1 << 53 => i.to_string(),
            _ => return Err("only safe integers are allowed"),
        },
        Value::String(s) => {
            let mut o = String::new();
            canon_string(s, &mut o);
            o
        }
        Value::Array(a) => {
            let parts: Result<Vec<String>, _> = a.iter().map(|x| canon_json(x, depth + 1)).collect();
            format!("[{}]", parts?.join(","))
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            let mut parts = Vec::with_capacity(keys.len());
            for k in keys {
                let mut o = String::new();
                canon_string(k, &mut o);
                parts.push(format!("{o}:{}", canon_json(&m[k], depth + 1)?));
            }
            format!("{{{}}}", parts.join(","))
        }
    })
}

/// Scans JSON text for a key repeated inside one object.
/// SECURITY: serde_json keeps the last duplicate while another parser may keep the first, which would let one
/// signed advert mean two things (CWE-694), so the text is checked before parsing.
fn has_duplicate_keys(t: &str) -> bool {
    let b = t.as_bytes();
    let mut stack: Vec<Option<Vec<String>>> = vec![];
    let (mut i, mut expect_key) = (0, false);
    while i < b.len() {
        match b[i] {
            b'{' => {
                stack.push(Some(vec![]));
                expect_key = true;
                i += 1;
            }
            b'[' => {
                stack.push(None);
                expect_key = false;
                i += 1;
            }
            b'}' | b']' => {
                stack.pop();
                expect_key = false;
                i += 1;
            }
            b',' => {
                expect_key = matches!(stack.last(), Some(Some(_)));
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                if expect_key {
                    let key: String = serde_json::from_str(&t[start..i.min(b.len())]).unwrap_or_default();
                    if let Some(Some(set)) = stack.last_mut() {
                        if set.contains(&key) {
                            return true;
                        }
                        set.push(key);
                    }
                    expect_key = false;
                }
            }
            _ => i += 1,
        }
    }
    false
}

/// Parses JSON text, refusing duplicate keys, floats, big integers and deep nesting.
pub fn parse_strict(t: &str) -> Result<Value, &'static str> {
    if has_duplicate_keys(t) {
        return Err("duplicate key");
    }
    let v: Value = serde_json::from_str(t).map_err(|_| "bad json")?;
    canon_json(&v, 0)?;
    Ok(v)
}

// ------------------------------------------------------------------ endpoint rules (protocol/src/endpoint.ts)

/// True for an address a discovery or public-gateway connection must never reach. IPv6 is refused altogether, as
/// in the node (CWE-918).
pub fn private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V6(_) => true,
        IpAddr::V4(a) => {
            let o = a.octets();
            (o[0] == 100 && o[1] & 0xc0 == 64) // carrier-grade NAT 100.64/10
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || (o[0] == 198 && o[1] & 0xfe == 18) // benchmarking 198.18/15
                || o[0] >= 224
                || a.is_loopback()
                || a.is_private()
                || a.is_link_local() // includes 169.254.169.254 (cloud metadata)
                || a.is_unspecified()
                || a.is_broadcast()
        }
    }
}

/// Returns the URL when it is https without user info or fragment and its host is not localhost, a private IP, an
/// IPv6 literal or a numeric IPv4 trick. `allow_private` exists for this module's tests only.
pub fn valid_ad_url(raw: &str, allow_private: bool) -> Option<String> {
    if raw.is_empty() || raw.len() > 256 || raw.contains([' ', '\t', '\r', '\n']) {
        return None;
    }
    let rest = raw.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (auth, tail) = rest.split_at(end);
    if auth.is_empty() || auth.contains('@') || tail.contains('#') || raw.contains('[') {
        return None;
    }
    let (host, port) = match auth.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (auth, None),
    };
    if host.is_empty() || port.is_some_and(|p| p.is_empty() || p.parse::<u16>().is_err()) {
        return None;
    }
    if allow_private {
        return Some(raw.to_string());
    }
    let h = host.to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h.ends_with(".internal") {
        return None;
    }
    if let Ok(ip) = h.parse::<Ipv4Addr>() {
        return if private_ip(IpAddr::V4(ip)) { None } else { Some(raw.to_string()) };
    }
    if h.trim_matches(|c: char| c.is_ascii_digit() || c == '.').is_empty() {
        return None; // decimal, octal or short IPv4 forms such as 2130706433 or 127.1
    }
    Some(raw.to_string())
}

/// Lower-case host of a `scheme://host[:port]/...` URL; empty when there is none.
fn url_host(raw: &str) -> String {
    let Some((_, rest)) = raw.split_once("://") else {
        return String::new();
    };
    if raw.contains('[') {
        return String::new();
    }
    let auth = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    if auth.contains('@') {
        return String::new();
    }
    auth.rsplit_once(':').map_or(auth, |(h, _)| h).to_ascii_lowercase()
}

fn is_reach_host(raw: &str) -> bool {
    url_host(raw).ends_with(REACH_SUFFIX)
}

/// A public https URL, or (only when the advert lists relays) this advert's own relay name (CWE-345, CWE-601).
fn ad_endpoint(raw: &str, allow_private: bool, gateway_id: &str, has_reach: bool) -> Option<String> {
    if !is_reach_host(raw) {
        return valid_ad_url(raw, allow_private);
    }
    if has_reach && url_host(raw) == format!("{gateway_id}{REACH_SUFFIX}") {
        return valid_ad_url(raw, true);
    }
    None
}

/// The optional `reach` object: exactly `relays` (1-4) and `rendezvous` (0-4), distinct public https URLs.
fn ad_reach(v: &Value, allow_private: bool) -> Option<()> {
    let Value::Object(r) = v else { return None };
    if r.len() != 2 {
        return None;
    }
    let list = |k: &str, min: usize| -> Option<()> {
        let Value::Array(arr) = r.get(k)? else { return None };
        if arr.len() < min || arr.len() > 4 {
            return None;
        }
        let mut seen: Vec<&str> = Vec::new();
        for e in arr {
            let u = e.as_str()?;
            if seen.contains(&u) || is_reach_host(u) || valid_ad_url(u, allow_private).is_none() {
                return None;
            }
            seen.push(u);
        }
        Some(())
    };
    list("relays", 1)?;
    list("rendezvous", 0)
}

/// How people get access to a gateway that lists itself for users (ADR-028 `client_access`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ClientAccess {
    /// `key` (ask the operator for a key) or `signup` (get one at `signup_url`).
    pub mode: String,
    /// The OpenAI-style API base the IDE talks to.
    pub api_url: String,
    /// Sign-up page for mode `signup`; empty for `key`.
    pub signup_url: String,
    /// Subset of `chat`, `openai_api`, `agent_runs`.
    pub features: Vec<String>,
    /// The gateway's terms for users, or empty.
    pub terms_url: String,
    /// Whether the operator says prompts are logged.
    pub logs_prompts: bool,
    /// How long logged prompts are kept, in days (0 when not logged).
    pub retention_days: i64,
}

/// The `client_access` object: exactly its seven keys, closed value sets, public https URLs that are never relay
/// names, 1-3 distinct known features, retention 0-3650 days and 0 when prompts are not logged (CWE-20, CWE-918).
fn ad_client(v: &Value, allow_private: bool) -> Option<ClientAccess> {
    const KEYS: [&str; 7] =
        ["mode", "api_url", "signup_url", "features", "terms_url", "logs_prompts", "retention_days"];
    const FEATURES: [&str; 3] = ["chat", "openai_api", "agent_runs"];
    let Value::Object(c) = v else { return None };
    if c.len() != KEYS.len() || !KEYS.iter().all(|k| c.contains_key(*k)) {
        return None;
    }
    let url =
        |u: &str| !u.is_empty() && u.len() <= 256 && !is_reach_host(u) && valid_ad_url(u, allow_private).is_some();
    let mode = c["mode"].as_str()?;
    let api = c["api_url"].as_str()?;
    let signup = c["signup_url"].as_str()?;
    let terms = c["terms_url"].as_str()?;
    let mode_ok = match mode {
        "key" => signup.is_empty(),
        "signup" => url(signup),
        _ => false,
    };
    if !mode_ok || !url(api) || (!terms.is_empty() && !url(terms)) {
        return None;
    }
    let Value::Array(f) = &c["features"] else { return None };
    let mut features: Vec<String> = Vec::new();
    for e in f {
        match e.as_str() {
            Some(x) if FEATURES.contains(&x) && !features.iter().any(|s| s == x) => features.push(x.to_string()),
            _ => return None,
        }
    }
    if features.is_empty() {
        return None;
    }
    let logs = c["logs_prompts"].as_bool()?;
    let days = c["retention_days"].as_i64()?;
    if !(0..=3650).contains(&days) || (!logs && days != 0) {
        return None;
    }
    Some(ClientAccess {
        mode: mode.into(),
        api_url: api.into(),
        signup_url: signup.into(),
        features,
        terms_url: terms.into(),
        logs_prompts: logs,
        retention_days: days,
    })
}

const FIELDS: [&str; 19] = [
    "v",
    "gateway_id",
    "public_key",
    "endpoints",
    "seq",
    "issued_at",
    "expires_at",
    "node_gateway_id",
    "node_public_key",
    "enrollment",
    "offers_public",
    "hub_url",
    "hub_public_key",
    "summary",
    "terms_url",
    "reach",
    "contact",
    "client_access",
    "sig",
];

/// Base64 of exactly `n` bytes in canonical (padded, re-encodable) form only.
fn b64n(s: &str, n: usize) -> Option<Vec<u8>> {
    let b = B64.decode(s).ok()?;
    (b.len() == n && B64.encode(&b) == s).then_some(b)
}

fn int(m: &serde_json::Map<String, Value>, k: &str) -> Option<i64> {
    m.get(k)?.as_i64().filter(|v| *v >= 0)
}

/// A one-line string of at most `max` bytes with no control characters (CWE-117).
fn text(m: &serde_json::Map<String, Value>, k: &str, max: usize) -> Option<String> {
    let s = m.get(k)?.as_str()?;
    (s.len() <= max && !s.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f)).then(|| s.to_string())
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn node_id_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

fn hex_lower(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A verified advertisement: the parts the IDE uses.
#[derive(Clone, Debug)]
pub struct Advert {
    pub gateway_id: String,
    pub seq: i64,
    pub region: String,
    pub models: Vec<String>,
    pub enrollment: String,
    pub probe: String,
    /// Present only when the gateway lists itself for users.
    pub client: Option<ClientAccess>,
    key: [u8; 32],
}

/// Verifies one advertisement text in the protocol's fixed order and returns it, or a fixed reason code.
/// SECURITY: fails closed on every irregularity; the signature is checked before the time (CWE-347).
pub fn verify_advert(t: &str, now: i64, allow_private: bool) -> Result<Advert, &'static str> {
    if t.len() > AD_MAX_BYTES {
        return Err("too_large");
    }
    let Value::Object(mut m) = parse_strict(t).map_err(|_| "bad_json")? else {
        return Err("bad_json");
    };
    if m.keys().any(|k| !FIELDS.contains(&k.as_str())) || m.get("v").and_then(Value::as_i64) != Some(1) {
        return Err("bad_shape");
    }
    let (Some(Value::Object(ep)), Some(Value::Object(sm))) = (m.get("endpoints").cloned(), m.get("summary").cloned())
    else {
        return Err("bad_shape");
    };
    if ep.len() != 3 {
        return Err("bad_shape");
    }
    let has_reach = m.contains_key("reach");
    if let Some(r) = m.get("reach") {
        ad_reach(r, allow_private).ok_or("bad_shape")?;
    }
    let client = match m.get("client_access") {
        Some(c) => Some(ad_client(c, allow_private).ok_or("bad_shape")?),
        None => None,
    };
    // Checked against the key below; a mismatch fails closed there.
    let claimed_id = m.get("gateway_id").and_then(Value::as_str).unwrap_or("").to_string();
    let url =
        |k: &str| ep.get(k).and_then(Value::as_str).and_then(|s| ad_endpoint(s, allow_private, &claimed_id, has_reach));
    let (Some(_api), Some(_attach), Some(probe)) = (url("api"), url("attach"), url("probe")) else {
        return Err("bad_endpoint");
    };
    let (Some(seq), Some(issued), Some(expires)) = (int(&m, "seq"), int(&m, "issued_at"), int(&m, "expires_at")) else {
        return Err("bad_shape");
    };
    if expires <= issued || expires - issued > AD_MAX_LIFETIME {
        return Err("bad_time");
    }
    let enrollment = text(&m, "enrollment", 16).unwrap_or_default();
    if !["closed", "approval", "open"].contains(&enrollment.as_str()) {
        return Err("bad_enrollment");
    }
    let (Some(ngid), Some(npk)) = (text(&m, "node_gateway_id", 64), text(&m, "node_public_key", 64)) else {
        return Err("bad_shape");
    };
    if !node_id_ok(&ngid) || !hex64(&npk) || !m.get("offers_public").is_some_and(Value::is_boolean) {
        return Err("bad_shape");
    }
    let (Some(hub_url), Some(hub_key)) = (text(&m, "hub_url", 256), text(&m, "hub_public_key", 64)) else {
        return Err("bad_shape");
    };
    if hub_url.is_empty() != hub_key.is_empty()
        || (!hub_url.is_empty()
            && (valid_ad_url(&hub_url, allow_private).is_none() || is_reach_host(&hub_url) || !hex64(&hub_key)))
    {
        return Err("bad_shape");
    }
    let models: Vec<String> = match sm.get("models") {
        Some(Value::Array(a)) if a.len() <= 64 => {
            a.iter().filter_map(|x| x.as_str().filter(|s| s.len() <= 128).map(String::from)).collect()
        }
        _ => return Err("bad_shape"),
    };
    if sm.get("models").and_then(Value::as_array).map(Vec::len) != Some(models.len()) {
        return Err("bad_shape");
    }
    let Some(Value::Object(nodes)) = sm.get("nodes") else {
        return Err("bad_shape");
    };
    if ["private", "trusted", "public"].iter().any(|k| int(nodes, k).is_none()) {
        return Err("bad_shape");
    }
    let (Some(region), Some(terms), Some(_)) =
        (text(&sm, "region", 32), text(&m, "terms_url", 256), text(&m, "contact", 128))
    else {
        return Err("bad_shape");
    };
    if !terms.is_empty() && (valid_ad_url(&terms, allow_private).is_none() || is_reach_host(&terms)) {
        return Err("bad_shape");
    }
    let pk_text = m.get("public_key").and_then(Value::as_str).unwrap_or("").to_string();
    let pk = b64n(&pk_text, 32).ok_or("bad_key")?;
    // SECURITY: the id is bound to the key, so a directory cannot pass one gateway's advert off as another's.
    if claimed_id != hex_lower(&Sha256::digest(&pk))[..40] {
        return Err("bad_id");
    }
    let sig_text = m.get("sig").and_then(Value::as_str).unwrap_or("").to_string();
    m.remove("sig");
    let body = canon_json(&Value::Object(m), 0).map_err(|_| "bad_signature")?;
    let key: [u8; 32] = pk.try_into().map_err(|_| "bad_key")?;
    let ok = b64n(&sig_text, 64)
        .and_then(|s| Signature::from_slice(&s).ok())
        .zip(VerifyingKey::from_bytes(&key).ok())
        .is_some_and(|(sig, vk)| vk.verify(format!("{SIG_PREFIX}{body}").as_bytes(), &sig).is_ok());
    if !ok {
        return Err("bad_signature");
    }
    if now + AD_MAX_SKEW < issued {
        return Err("bad_time");
    }
    if now >= expires {
        return Err("expired");
    }
    Ok(Advert { gateway_id: claimed_id, seq, region, models, enrollment, probe, client, key })
}

/// True only for a probe reply that echoes the nonce, names the gateway, is fresh and verifies under the advert's
/// key (CWE-294, CWE-345).
pub fn verify_probe_reply(body: &str, nonce: &str, a: &Advert, now: i64) -> bool {
    let Ok(Value::Object(m)) = parse_strict(body) else {
        return false;
    };
    let Some(ts) = m.get("ts").and_then(Value::as_i64) else {
        return false;
    };
    if m.get("nonce").and_then(Value::as_str) != Some(nonce)
        || m.get("gateway_id").and_then(Value::as_str) != Some(a.gateway_id.as_str())
        || (ts - now).abs() > AD_MAX_SKEW
    {
        return false;
    }
    let sig =
        m.get("sig").and_then(Value::as_str).and_then(|s| b64n(s, 64)).and_then(|s| Signature::from_slice(&s).ok());
    match (sig, VerifyingKey::from_bytes(&a.key)) {
        (Some(sig), Ok(vk)) => {
            vk.verify(format!("{PROBE_PREFIX}{nonce}\n{ts}\n{}", a.gateway_id).as_bytes(), &sig).is_ok()
        }
        _ => false,
    }
}

// ------------------------------------------------------------------ network

/// ureq resolver that keeps only public IPv4 addresses.
/// SECURITY: ureq connects only to what this returns, so a public name that resolves to a private, loopback or
/// metadata address is refused at connect time, after DNS, which also defeats DNS rebinding (CWE-918).
#[derive(Debug, Default)]
pub struct PublicOnly;

impl ureq::unversioned::resolver::Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let all = ureq::unversioned::resolver::DefaultResolver::default().resolve(uri, config, timeout)?;
        let mut out = self.empty();
        for a in all.iter().filter(|a: &&SocketAddr| !private_ip(a.ip())) {
            out.push(*a);
        }
        if out.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(out)
    }
}

/// An agent for public hosts only: TLS on (Mozilla roots), https only, no redirects, no proxy, [`PublicOnly`].
pub fn public_agent(timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(timeout))
        .user_agent("p2p-ai-ide/0.1")
        .build();
    ureq::Agent::with_parts(config, ureq::unversioned::transport::DefaultConnector::default(), PublicOnly)
}

/// Fetcher seam: GET a URL and return at most `max` bytes of body as text. The real one is [`real_fetch`]; tests
/// pass a fake. Errors are short fixed texts.
pub type Fetch<'a> = &'a (dyn Fn(&str, u64) -> Result<String, String> + Sync);

/// The real fetcher over [`public_agent`] with a 10 s limit per request.
pub fn real_fetch() -> impl Fn(&str, u64) -> Result<String, String> + Sync {
    let ag = public_agent(Duration::from_secs(10));
    move |url: &str, max: u64| {
        let r = ag.get(url).call().map_err(|_| "request failed".to_string())?;
        let mut buf = vec![];
        r.into_body().into_reader().take(max + 1).read_to_end(&mut buf).map_err(|_| "unreadable".to_string())?;
        if buf.len() as u64 > max {
            return Err("answer too large".into());
        }
        String::from_utf8(buf).map_err(|_| "not utf-8".to_string())
    }
}

/// One gateway in the public list, as the webview sees it. Every string here came from a verified advert and is
/// shown as plain text only.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PublicGateway {
    /// 40 hex characters, bound to the gateway's signing key.
    pub gateway_id: String,
    pub region: String,
    pub models: Vec<String>,
    pub access: ClientAccess,
    /// Measured by the IDE with the signed probe; `None` when the probe failed or goes through a relay.
    pub latency_ms: Option<u64>,
    /// How many of the person's directories listed it.
    pub directories: usize,
}

/// The result of one listing: the gateways (fastest first, unprobed last) and rejection counts by reason code.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct PublicList {
    pub gateways: Vec<PublicGateway>,
    /// e.g. {"bad_signature": 2, "directory_unreachable": 1, "not_for_users": 5}.
    pub rejected: BTreeMap<String, usize>,
}

/// 16 random bytes from the OS CSPRNG as hex (CWE-330).
fn nonce() -> Option<String> {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).ok()?;
    Some(hex_lower(&b))
}

/// One signed probe; the round-trip time in ms when the reply verifies.
fn probe_once(a: &Advert, now: i64, fetch: Fetch) -> Option<u64> {
    if is_reach_host(&a.probe) {
        return None; // through a relay: the IDE does not speak the relay protocol
    }
    let n = nonce()?;
    let t0 = Instant::now();
    let body = fetch(&format!("{}?nonce={n}", a.probe), PROBE_MAX_BYTES).ok()?;
    let ms = t0.elapsed().as_millis() as u64;
    verify_probe_reply(body.trim(), &n, a, now).then_some(ms)
}

/// Fetches every directory, verifies the adverts, keeps the newest per gateway that lists itself for users, probes
/// them and returns the list. It never sends a key and never adds anything.
pub fn list_public(directories: &[String], now: i64, fetch: Fetch) -> PublicList {
    let mut rejected: BTreeMap<String, usize> = BTreeMap::new();
    let mut bump = |k: &str| *rejected.entry(k.to_string()).or_default() += 1;
    let mut best: HashMap<String, (Advert, usize)> = HashMap::new();
    for d in directories.iter().take(MAX_DIRS) {
        if check_directory(d).is_err() {
            bump("bad_directory");
            continue;
        }
        let list = fetch(&format!("{}/v1/adverts", d.trim_end_matches('/')), MAX_LIST_BYTES)
            .ok()
            .and_then(|b| serde_json::from_str::<Value>(&b).ok())
            .and_then(|v| v.get("adverts").cloned());
        let Some(Value::Array(list)) = list else {
            bump("directory_unreachable");
            continue;
        };
        if list.len() > MAX_ADVERTS {
            bump("directory_too_large");
            continue;
        }
        let mut seen_here: Vec<String> = Vec::new();
        for item in list {
            match item.as_str().ok_or("bad_shape").and_then(|t| verify_advert(t, now, false)) {
                Err(code) => bump(code),
                Ok(a) if a.client.is_none() => bump("not_for_users"),
                Ok(a) => {
                    let first_here = !seen_here.contains(&a.gateway_id);
                    if first_here {
                        seen_here.push(a.gateway_id.clone());
                    }
                    let e = best.entry(a.gateway_id.clone()).or_insert_with(|| (a.clone(), 0));
                    if first_here {
                        e.1 += 1;
                    }
                    if a.seq > e.0.seq {
                        e.0 = a;
                    }
                }
            }
        }
    }
    let mut ads: Vec<(Advert, usize)> = best.into_values().collect();
    ads.sort_by(|x, y| x.0.gateway_id.cmp(&y.0.gateway_id));
    if ads.len() > MAX_LISTED {
        *rejected.entry("over_limit".into()).or_default() += ads.len() - MAX_LISTED;
        ads.truncate(MAX_LISTED);
    }
    // Probe in small parallel batches so one slow gateway does not hold the list (10 s per request at most).
    let mut lat: Vec<Option<u64>> = Vec::with_capacity(ads.len());
    for chunk in ads.chunks(PROBE_PARALLEL) {
        let got: Vec<Option<u64>> = std::thread::scope(|s| {
            let hs: Vec<_> = chunk.iter().map(|(a, _)| s.spawn(move || probe_once(a, now, fetch))).collect();
            hs.into_iter().map(|h| h.join().ok().flatten()).collect()
        });
        lat.extend(got);
    }
    let mut gateways: Vec<PublicGateway> = ads
        .into_iter()
        .zip(lat)
        .filter_map(|((a, dirs), latency_ms)| {
            Some(PublicGateway {
                gateway_id: a.gateway_id,
                region: a.region,
                models: a.models,
                access: a.client?,
                latency_ms,
                directories: dirs,
            })
        })
        .collect();
    gateways.sort_by(|x, y| {
        (x.latency_ms.is_none(), x.latency_ms, &x.gateway_id).cmp(&(
            y.latency_ms.is_none(),
            y.latency_ms,
            &y.gateway_id,
        ))
    });
    PublicList { gateways, rejected }
}

/// The saved gateway URL for a public gateway's `api_url`: its origin, which is what the IDE's client and
/// gateways.json accept. An `api_url` with a path is refused for now (the client adds `/v1/...` to the origin).
pub fn api_origin(api_url: &str) -> Option<String> {
    let rest = api_url.strip_prefix("https://")?;
    let hostport = rest.strip_suffix('/').unwrap_or(rest);
    if hostport.is_empty() || hostport.contains(['/', '?', '#', '@']) {
        return None;
    }
    valid_ad_url(api_url, false)?;
    Some(format!("https://{}", hostport.to_ascii_lowercase()))
}

// ------------------------------------------------------------------ the person's directory list

/// Fixed errors for the directory list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirError {
    BadUrl,
    TooMany,
    Store(StoreError),
}

impl DirError {
    pub fn message(&self) -> &'static str {
        match self {
            DirError::BadUrl => "a directory must be a public https://host[:port] address with no user name",
            DirError::TooMany => "at most 8 directories can be listed",
            DirError::Store(e) => e.message(),
        }
    }
}

/// Checks one directory URL: public https, no user info, query or fragment, no relay name; returns it without a
/// trailing slash (CWE-918, CWE-20).
pub fn check_directory(u: &str) -> Result<String, DirError> {
    let u = u.trim();
    if u.contains(['?', '#']) || is_reach_host(u) {
        return Err(DirError::BadUrl);
    }
    valid_ad_url(u, false).map(|s| s.trim_end_matches('/').to_string()).ok_or(DirError::BadUrl)
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DirFile {
    #[serde(default)]
    directories: Vec<String>,
}

/// `directories.json` in the app data folder: the directories the person chose (empty by default: the IDE ships
/// with no directory, so nothing is fetched until the person adds one).
pub struct DirectoryStore {
    path: PathBuf,
}

impl DirectoryStore {
    pub fn new(dir: &Path) -> DirectoryStore {
        DirectoryStore { path: dir.join("directories.json") }
    }

    /// The saved list; a damaged or hand-edited file with a bad entry is refused as a whole (CWE-20).
    pub fn load(&self) -> Result<Vec<String>, DirError> {
        let bytes = match store::read_capped(&self.path, 64 << 10) {
            Ok(b) => b,
            Err(StoreError::Missing) => return Ok(vec![]),
            Err(e) => return Err(DirError::Store(e)),
        };
        let f: DirFile = serde_json::from_slice(&bytes).map_err(|_| DirError::Store(StoreError::Bad))?;
        if f.directories.len() > MAX_DIRS
            || f.directories.iter().any(|d| check_directory(d).as_deref() != Ok(d.as_str()))
        {
            return Err(DirError::Store(StoreError::Bad));
        }
        Ok(f.directories)
    }

    /// Replaces the list (checked, duplicates dropped, order kept). Returns the saved list.
    pub fn set(&self, list: &[String]) -> Result<Vec<String>, DirError> {
        let mut out: Vec<String> = Vec::new();
        for d in list {
            let d = check_directory(d)?;
            if !out.contains(&d) {
                out.push(d);
            }
        }
        if out.len() > MAX_DIRS {
            return Err(DirError::TooMany);
        }
        let dir = self.path.parent().ok_or(DirError::Store(StoreError::Io))?;
        store::ensure_dir(dir).map_err(DirError::Store)?;
        let body = serde_json::to_vec_pretty(&DirFile { directories: out.clone() })
            .map_err(|_| DirError::Store(StoreError::Io))?;
        store::write_atomic(&self.path, &body).map_err(DirError::Store)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_refuse_private_and_tricky_hosts() {
        for bad in [
            "http://gw.example.org",
            "https://localhost",
            "https://127.0.0.1",
            "https://10.0.0.5",
            "https://169.254.169.254",
            "https://[::1]",
            "https://2130706433",
            "https://127.1",
            "https://user@gw.example.org",
            "https://gw.local",
            "https://x.internal",
        ] {
            assert!(valid_ad_url(bad, false).is_none(), "{bad}");
        }
        assert!(valid_ad_url("https://gw.example.org/v1/probe", false).is_some());
        assert!(valid_ad_url("https://8.8.8.8:443", false).is_some());
    }

    #[test]
    fn api_origin_only_takes_an_origin() {
        assert_eq!(api_origin("https://GW.example.org/").as_deref(), Some("https://gw.example.org"));
        assert_eq!(api_origin("https://gw.example.org:8443").as_deref(), Some("https://gw.example.org:8443"));
        assert_eq!(api_origin("https://gw.example.org/api/"), None);
        assert_eq!(api_origin("https://10.1.2.3/"), None);
        assert_eq!(api_origin("http://gw.example.org/"), None);
    }

    #[test]
    fn directories_are_checked_and_saved() {
        let tmp = std::env::temp_dir().join(format!("p2pide-dirs-{}", store::new_id()));
        let s = DirectoryStore::new(&tmp);
        assert_eq!(s.load().unwrap(), Vec::<String>::new());
        let saved = s
            .set(&[
                "https://dir.example.org/".into(),
                "https://dir.example.org".into(),
                " https://b.example.org ".into(),
            ])
            .unwrap();
        assert_eq!(saved, vec!["https://dir.example.org", "https://b.example.org"]);
        assert_eq!(s.load().unwrap(), saved);
        assert_eq!(s.set(&["https://192.168.1.2".into()]), Err(DirError::BadUrl));
        assert_eq!(s.set(&["https://dir.example.org/?x=1".into()]), Err(DirError::BadUrl));
        let nine: Vec<String> = (0..9).map(|i| format!("https://d{i}.example.org")).collect();
        assert_eq!(s.set(&nine), Err(DirError::TooMany));
        std::fs::write(tmp.join("directories.json"), r#"{"directories":["https://127.0.0.1"]}"#).unwrap();
        assert_eq!(s.load(), Err(DirError::Store(StoreError::Bad)));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn public_only_never_connects_to_loopback() {
        // A listener on loopback: the public-only agent must refuse at resolve time and never connect to it.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.set_nonblocking(true).unwrap();
        let port = l.local_addr().unwrap().port();
        let fetch = real_fetch();
        assert!(fetch(&format!("https://localhost:{port}/v1/adverts"), 1024).is_err());
        assert!(l.accept().is_err(), "no connection may reach a private address");
    }

    #[test]
    fn strict_json_refuses_duplicates_and_floats() {
        assert!(parse_strict(r#"{"a":1,"a":2}"#).is_err());
        assert!(parse_strict(r#"{"a":1.5}"#).is_err());
        assert!(parse_strict(r#"{"a":{"b":{"c":{"d":{"e":{"f":{"g":{}}}}}}}}"#).is_err());
        assert!(parse_strict(r#"{"a":[1,2],"b":"x"}"#).is_ok());
    }
}
