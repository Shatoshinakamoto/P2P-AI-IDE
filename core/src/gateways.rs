//! gateways -- the gateways the IDE can talk to: saved list, checks on every field, and where keys live (phase I2).
//!
//! Purpose: people add a **local** gateway (the one their desktop app set up on this computer: its setup folder gives
//! the port and the stack's CA) or a **manual** one (URL, optional CA file, key). One gateway is active at a time.
//! The list is saved as `gateways.json` in the app's data folder; keys never are (see [`SecretStore`]).
//! Public gateways from discovery directories come in phase I2b.
//!
//! Inputs and trust boundaries: names, URLs, keys and ids come from the webview (untrusted); CA files and setup
//! folders are picked in the OS dialog by the shell and are still checked here; `gateways.json` is our own file but
//! is re-checked on load, so a damaged or edited file cannot smuggle in a bad URL.
//!
//! Security controls:
//!   * URLs: `https://host[:port]` only, no user info, path, query or fragment (CWE-20, NIST SC-8). Plain
//!     `http://` is accepted only for 127.0.0.1, localhost or [::1] and is flagged `dev`; the UI shows it as
//!     **INSECURE — DEV ONLY**.
//!   * Keys go to the OS keychain through [`SecretStore`] and are never returned to the webview, saved in files,
//!     logged or put in error text (CWE-522, CWE-532, NIST IA-5). The webview only learns `has_key`.
//!   * The IDE never reads a gateway's admin key, even from a setup folder (least privilege, NIST AC-6).
//!   * A CA file must be one or more PEM certificates and nothing else (no private keys), at most 64 KiB; it becomes
//!     the only trust root for that gateway (pinning, CWE-295).
//!   * Caps: 32 saved gateways, short names (CWE-400).
//!
//! Dependencies: serde/serde_json (the saved file), [`crate::store`].

use crate::store::{self, StoreError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Most saved gateways. SECURITY: bounds the file and the view (CWE-400).
pub const MAX_GATEWAYS: usize = 32;
/// Largest CA file accepted.
pub const MAX_CA_BYTES: usize = 64 << 10;
/// Largest saved list file.
const MAX_FILE: u64 = 4 << 20;
/// File name of the saved list inside the app data folder.
const FILE: &str = "gateways.json";
/// Keychain service name under which keys are stored (account = gateway id).
pub const KEY_SERVICE: &str = "org.p2p-ai.ide.gateway";

/// How the gateway was added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// From a setup folder on this computer (port + stack CA).
    Local,
    /// Typed in by the person.
    Manual,
    /// Picked from the public list (phase I2b): its address came from a signed advert. Connections to it go only
    /// to public addresses (discovery::PublicOnly, CWE-918).
    Public,
    /// TandemForge running on this computer (phase I7): always `http://127.0.0.1:<port>`, the port re-read from
    /// TandemForge's engine file before each use (see tandemforge.rs).
    TandemForge,
}

/// One saved gateway. `ca_pem` is the pinned trust root when present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gateway {
    pub id: String,
    pub name: String,
    pub url: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_pem: Option<String>,
}

impl Gateway {
    /// True for a plain-http loopback URL typed in by hand (INSECURE — DEV ONLY). TandemForge on this computer is
    /// plain http on loopback by design (a local process, nothing leaves the computer), so it is not flagged.
    pub fn dev(&self) -> bool {
        self.url.starts_with("http://") && self.kind != Kind::TandemForge
    }
}

/// What the webview sees of a gateway: no key, no CA body.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct GatewayView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub kind: Kind,
    /// Plain http on loopback: show **INSECURE — DEV ONLY**.
    pub dev: bool,
    /// A CA file is pinned for this gateway.
    pub pinned: bool,
    /// A key is stored in the OS keychain.
    pub has_key: bool,
}

/// The saved list plus which one is active.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayList {
    #[serde(default)]
    pub gateways: Vec<Gateway>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
}

/// Fixed errors; `message()` is safe to show (no URL, key or path in it, CWE-209).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GwError {
    BadName,
    BadUrl,
    BadKey,
    BadCa,
    BadSetup,
    NotFound,
    TooMany,
    Duplicate,
    Keychain,
    Store(StoreError),
}

impl GwError {
    pub fn message(&self) -> &'static str {
        match self {
            GwError::BadName => "the name must be 1 to 64 characters on one line",
            GwError::BadUrl => "use https://host[:port] (or http://127.0.0.1:port for a dev gateway), with no path",
            GwError::BadKey => "the key must be 8 to 512 visible characters with no spaces",
            GwError::BadCa => "the CA file must hold PEM certificates only (no private key), up to 64 KiB",
            GwError::BadSetup => "that folder has no gateway setup (.env and tls/ca/ca.pem were not found)",
            GwError::NotFound => "that gateway is not in the list",
            GwError::TooMany => "at most 32 gateways can be saved",
            GwError::Duplicate => "a gateway with that URL is already saved",
            GwError::Keychain => "the system keychain could not store or read the key",
            GwError::Store(e) => e.message(),
        }
    }
}

/// Where gateway keys live. The shell implements it with the OS keychain; tests use memory.
/// SECURITY: implementations must never log the secret (CWE-532).
pub trait SecretStore {
    fn get(&self, id: &str) -> Result<Option<String>, GwError>;
    fn set(&self, id: &str, secret: &str) -> Result<(), GwError>;
    fn delete(&self, id: &str) -> Result<(), GwError>;
}

/// Checks a display name: 1-64 chars, no control characters (CWE-20, CWE-117 for later logs).
pub fn check_name(name: &str) -> Result<String, GwError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 64 || n.chars().any(|c| c.is_control()) {
        return Err(GwError::BadName);
    }
    Ok(n.to_string())
}

/// Checks and normalises a gateway URL to `scheme://host[:port]` (no trailing slash).
///
/// SECURITY: an allow-list parser, not a general URL library: only the characters a host name, IPv4 or bracketed
/// IPv6 address and port can hold, so nothing can hide a different host (user info, `\`, `%`, spaces; CWE-20,
/// CWE-918). Plain http only for loopback (NIST SC-8).
pub fn check_url(url: &str) -> Result<String, GwError> {
    let u = url.trim();
    if u.len() > 300 {
        return Err(GwError::BadUrl);
    }
    let (scheme, rest) = if let Some(r) = u.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = u.strip_prefix("http://") {
        ("http", r)
    } else {
        return Err(GwError::BadUrl);
    };
    // One optional trailing slash; any other path, query or fragment is refused.
    let hostport = rest.strip_suffix('/').unwrap_or(rest);
    if hostport.is_empty() || hostport.contains(['/', '?', '#', '@', '\\', '%', ' ']) {
        return Err(GwError::BadUrl);
    }
    let (host, port) = split_host_port(hostport)?;
    let host_l = host.to_ascii_lowercase();
    let loopback = host_l == "127.0.0.1" || host_l == "localhost" || host_l == "[::1]";
    if scheme == "http" && !loopback {
        return Err(GwError::BadUrl);
    }
    Ok(match port {
        Some(p) => format!("{scheme}://{host_l}:{p}"),
        None => format!("{scheme}://{host_l}"),
    })
}

fn split_host_port(hp: &str) -> Result<(&str, Option<u16>), GwError> {
    let (host, port) = if hp.starts_with('[') {
        let end = hp.find(']').ok_or(GwError::BadUrl)?;
        let (h, rest) = hp.split_at(end + 1);
        let inner = &h[1..h.len() - 1];
        if inner.is_empty() || !inner.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.') {
            return Err(GwError::BadUrl);
        }
        (h, rest.strip_prefix(':'))
    } else {
        match hp.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (hp, None),
        }
    };
    if !host.starts_with('[') {
        let ok = !host.is_empty()
            && host.len() <= 253
            && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
            && !host.starts_with(['-', '.'])
            && !host.ends_with('-');
        if !ok {
            return Err(GwError::BadUrl);
        }
    }
    let port = match port {
        None => None,
        Some(p) => {
            if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return Err(GwError::BadUrl);
            }
            match p.parse::<u16>() {
                Ok(n) if n > 0 => Some(n),
                _ => return Err(GwError::BadUrl),
            }
        }
    };
    Ok((host, port))
}

/// Checks a key: 8-512 visible ASCII characters, no spaces. SECURITY: also keeps the key from breaking the
/// Authorization header (CWE-113).
pub fn check_key(key: &str) -> Result<String, GwError> {
    let k = key.trim();
    if k.len() < 8 || k.len() > 512 || !k.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(GwError::BadKey);
    }
    Ok(k.to_string())
}

/// Checks a CA file: PEM `CERTIFICATE` blocks only, at least one, no private key, at most 64 KiB.
/// SECURITY: a mistaken pick of a key file is refused so a private key never gets copied into our state (NIST SC-12).
pub fn check_ca(pem: &str) -> Result<String, GwError> {
    if pem.len() > MAX_CA_BYTES || pem.contains("PRIVATE KEY") {
        return Err(GwError::BadCa);
    }
    let mut certs = 0;
    let mut inside = false;
    for line in pem.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        if line == "-----BEGIN CERTIFICATE-----" && !inside {
            inside = true;
        } else if line == "-----END CERTIFICATE-----" && inside {
            inside = false;
            certs += 1;
        } else if inside {
            if !line.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=') {
                return Err(GwError::BadCa);
            }
        } else {
            return Err(GwError::BadCa);
        }
    }
    if certs == 0 || inside {
        return Err(GwError::BadCa);
    }
    Ok(pem.to_string())
}

/// Reads a gateway setup folder (made by the P2P-AI desktop app or setup.sh): the API port from `.env`
/// (`GATEWAY_API_PORT`, default 8080) and the stack's CA from `tls/ca/ca.pem`. Returns `(url, ca_pem)`.
/// SECURITY: reads only those two files, bounded and not through links; never `secrets/` (NIST AC-6).
pub fn read_setup(dir: &Path) -> Result<(String, String), GwError> {
    let env = store::read_capped(&dir.join(".env"), 256 << 10).map_err(|_| GwError::BadSetup)?;
    let env = String::from_utf8(env).map_err(|_| GwError::BadSetup)?;
    let port = env
        .lines()
        .rev()
        .filter_map(|l| l.trim().strip_prefix("GATEWAY_API_PORT="))
        .filter_map(|v| v.trim().trim_matches('"').parse::<u16>().ok())
        .find(|p| *p > 0)
        .unwrap_or(8080);
    let ca = store::read_capped(&dir.join("tls").join("ca").join("ca.pem"), MAX_CA_BYTES as u64)
        .map_err(|_| GwError::BadSetup)?;
    let ca = String::from_utf8(ca).map_err(|_| GwError::BadCa)?;
    Ok((format!("https://127.0.0.1:{port}"), check_ca(&ca)?))
}

/// The saved list in one app data folder.
pub struct GatewayStore {
    path: PathBuf,
}

impl GatewayStore {
    /// `dir` is the app's data folder, chosen by the shell.
    pub fn new(dir: &Path) -> GatewayStore {
        GatewayStore { path: dir.join(FILE) }
    }

    /// Loads the list; a missing file is an empty list. Every entry is re-checked.
    pub fn load(&self) -> Result<GatewayList, GwError> {
        let bytes = match store::read_capped(&self.path, MAX_FILE) {
            Ok(b) => b,
            Err(StoreError::Missing) => return Ok(GatewayList::default()),
            Err(e) => return Err(GwError::Store(e)),
        };
        let list: GatewayList = serde_json::from_slice(&bytes).map_err(|_| GwError::Store(StoreError::Bad))?;
        // SECURITY: our own file is still re-validated, so an edited file cannot add a bad URL or CA (CWE-20).
        if list.gateways.len() > MAX_GATEWAYS {
            return Err(GwError::Store(StoreError::Bad));
        }
        for g in &list.gateways {
            let ok = store::id_ok(&g.id)
                && check_name(&g.name).is_ok()
                && check_url(&g.url).as_deref() == Ok(g.url.as_str())
                // SECURITY: a TandemForge entry may only ever point at 127.0.0.1 (NIST SC-7).
                && (g.kind != Kind::TandemForge || crate::tandemforge::is_engine_url(&g.url))
                && g.ca_pem.as_deref().map(|c| check_ca(c).is_ok()).unwrap_or(true);
            if !ok {
                return Err(GwError::Store(StoreError::Bad));
            }
        }
        let mut list = list;
        if list.active.as_ref().is_some_and(|a| !list.gateways.iter().any(|g| &g.id == a)) {
            list.active = None;
        }
        Ok(list)
    }

    fn save(&self, list: &GatewayList) -> Result<(), GwError> {
        let bytes = serde_json::to_vec_pretty(list).map_err(|_| GwError::Store(StoreError::Bad))?;
        store::write_atomic(&self.path, &bytes).map_err(GwError::Store)
    }

    /// Adds a gateway (and its key, when given). The first one added becomes active. Returns the new id.
    pub fn add(
        &self,
        secrets: &dyn SecretStore,
        name: &str,
        url: &str,
        kind: Kind,
        ca_pem: Option<&str>,
        key: Option<&str>,
    ) -> Result<String, GwError> {
        let name = check_name(name)?;
        let url = check_url(url)?;
        let ca_pem = ca_pem.map(check_ca).transpose()?;
        let key = key.filter(|k| !k.trim().is_empty()).map(check_key).transpose()?;
        let mut list = self.load()?;
        if list.gateways.len() >= MAX_GATEWAYS {
            return Err(GwError::TooMany);
        }
        if list.gateways.iter().any(|g| g.url == url) {
            return Err(GwError::Duplicate);
        }
        let id = store::new_id();
        // Store the key first: if the keychain fails, nothing is saved half-way.
        if let Some(k) = key {
            secrets.set(&id, &k)?;
        }
        list.gateways.push(Gateway { id: id.clone(), name, url, kind, ca_pem });
        if list.active.is_none() {
            list.active = Some(id.clone());
        }
        if let Err(e) = self.save(&list) {
            let _ = secrets.delete(&id);
            return Err(e);
        }
        Ok(id)
    }

    /// Removes a gateway and its key.
    pub fn remove(&self, secrets: &dyn SecretStore, id: &str) -> Result<(), GwError> {
        let mut list = self.load()?;
        let before = list.gateways.len();
        list.gateways.retain(|g| g.id != id);
        if list.gateways.len() == before {
            return Err(GwError::NotFound);
        }
        if list.active.as_deref() == Some(id) {
            list.active = list.gateways.first().map(|g| g.id.clone());
        }
        self.save(&list)?;
        secrets.delete(id)
    }

    /// Makes `id` the active gateway.
    pub fn set_active(&self, id: &str) -> Result<(), GwError> {
        let mut list = self.load()?;
        if !list.gateways.iter().any(|g| g.id == id) {
            return Err(GwError::NotFound);
        }
        list.active = Some(id.to_string());
        self.save(&list)
    }

    /// Saves (or updates) the TandemForge-on-this-computer entry with its current URL and pairing code, and makes
    /// it active. There is at most one. Returns its id.
    ///
    /// Security: the URL must be one [`crate::tandemforge::url_for`] made (loopback only); the code goes to the
    /// keychain, never to the list file.
    pub fn save_tandemforge(&self, secrets: &dyn SecretStore, url: &str, code: &str) -> Result<String, GwError> {
        if !crate::tandemforge::is_engine_url(url) {
            return Err(GwError::BadUrl);
        }
        let code = crate::tandemforge::check_code(code).map_err(|_| GwError::BadKey)?;
        let mut list = self.load()?;
        // Another saved gateway at the same address (a manual dev entry) would be ambiguous.
        if list.gateways.iter().any(|g| g.url == url && g.kind != Kind::TandemForge) {
            return Err(GwError::Duplicate);
        }
        let id = match list.gateways.iter_mut().find(|g| g.kind == Kind::TandemForge) {
            Some(g) => {
                g.url = url.to_string();
                g.id.clone()
            }
            None => {
                if list.gateways.len() >= MAX_GATEWAYS {
                    return Err(GwError::TooMany);
                }
                let id = store::new_id();
                list.gateways.push(Gateway {
                    id: id.clone(),
                    name: "TandemForge (this computer)".into(),
                    url: url.to_string(),
                    kind: Kind::TandemForge,
                    ca_pem: None,
                });
                id
            }
        };
        secrets.set(&id, &code)?;
        list.active = Some(id.clone());
        self.save(&list)?;
        Ok(id)
    }

    /// Points the TandemForge entry `id` at TandemForge's port of this launch.
    pub fn set_tandemforge_url(&self, id: &str, url: &str) -> Result<(), GwError> {
        if !crate::tandemforge::is_engine_url(url) {
            return Err(GwError::BadUrl);
        }
        let mut list = self.load()?;
        let g =
            list.gateways.iter_mut().find(|g| g.id == id && g.kind == Kind::TandemForge).ok_or(GwError::NotFound)?;
        if g.url == url {
            return Ok(());
        }
        g.url = url.to_string();
        self.save(&list)
    }

    /// Replaces (or with `None`, removes) the pinned CA of a gateway.
    pub fn set_ca(&self, id: &str, ca_pem: Option<&str>) -> Result<(), GwError> {
        let ca = ca_pem.map(check_ca).transpose()?;
        let mut list = self.load()?;
        let g = list.gateways.iter_mut().find(|g| g.id == id).ok_or(GwError::NotFound)?;
        g.ca_pem = ca;
        self.save(&list)
    }

    /// Stores a new key for a gateway in the keychain (the list file does not change).
    pub fn set_key(&self, secrets: &dyn SecretStore, id: &str, key: &str) -> Result<(), GwError> {
        let key = check_key(key)?;
        self.get(id)?;
        secrets.set(id, &key)
    }

    /// One gateway by id.
    pub fn get(&self, id: &str) -> Result<Gateway, GwError> {
        self.load()?.gateways.into_iter().find(|g| g.id == id).ok_or(GwError::NotFound)
    }

    /// The list as the webview may see it.
    pub fn views(&self, secrets: &dyn SecretStore) -> Result<(Vec<GatewayView>, Option<String>), GwError> {
        let list = self.load()?;
        let views = list
            .gateways
            .iter()
            .map(|g| GatewayView {
                id: g.id.clone(),
                name: g.name.clone(),
                url: g.url.clone(),
                kind: g.kind,
                dev: g.dev(),
                pinned: g.ca_pem.is_some(),
                // A keychain read failure shows as "no key" rather than failing the whole list.
                has_key: secrets.get(&g.id).ok().flatten().is_some(),
            })
            .collect();
        Ok((views, list.active))
    }
}

#[cfg(test)]
mod tandemforge_tests {
    use super::*;

    #[test]
    fn one_tandemforge_entry_on_loopback_with_its_code_in_the_keychain() {
        let dir = std::env::temp_dir().join(format!("gw-tf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = GatewayStore::new(&dir);
        let sec = MemSecrets::default();
        let code = format!("tfa_{}", "b".repeat(43));
        assert_eq!(s.save_tandemforge(&sec, "http://10.0.0.5:41234", &code), Err(GwError::BadUrl));
        assert_eq!(s.save_tandemforge(&sec, "http://127.0.0.1:41234", "sk-nope"), Err(GwError::BadKey));
        let id = s.save_tandemforge(&sec, "http://127.0.0.1:41234", &code).unwrap();
        let again = s.save_tandemforge(&sec, "http://127.0.0.1:41999", &code).unwrap();
        assert_eq!(id, again);
        let list = s.load().unwrap();
        assert_eq!(list.gateways.len(), 1);
        assert_eq!(list.active.as_deref(), Some(id.as_str()));
        let g = s.get(&id).unwrap();
        assert_eq!((g.kind, g.url.as_str(), g.dev()), (Kind::TandemForge, "http://127.0.0.1:41999", false));
        assert_eq!(sec.get(&id).unwrap().as_deref(), Some(code.as_str()));
        assert!(!std::fs::read_to_string(dir.join(FILE)).unwrap().contains(&code));
        s.set_tandemforge_url(&id, "http://127.0.0.1:42000").unwrap();
        assert_eq!(s.set_tandemforge_url(&id, "https://example.org"), Err(GwError::BadUrl));
        // An edited list file cannot point the TandemForge entry elsewhere.
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap().replace("127.0.0.1:42000", "localhost:42000");
        std::fs::write(dir.join(FILE), text).unwrap();
        assert!(s.load().is_err());
    }
}

/// An in-memory [`SecretStore`] for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemSecrets(pub std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl SecretStore for MemSecrets {
    fn get(&self, id: &str) -> Result<Option<String>, GwError> {
        Ok(self.0.lock().unwrap().get(id).cloned())
    }
    fn set(&self, id: &str, secret: &str) -> Result<(), GwError> {
        self.0.lock().unwrap().insert(id.into(), secret.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), GwError> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CA: &str = "-----BEGIN CERTIFICATE-----\nMIIBszCCAVmgAwIBAgIU\nAbC+/=\n-----END CERTIFICATE-----\n";

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-gw-{tag}-{}", store::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn urls_are_allow_listed() {
        assert_eq!(check_url("https://GW.Example.org/").unwrap(), "https://gw.example.org");
        assert_eq!(check_url(" https://gw.example.org:8443 ").unwrap(), "https://gw.example.org:8443");
        assert_eq!(check_url("http://127.0.0.1:8080").unwrap(), "http://127.0.0.1:8080");
        assert_eq!(check_url("http://localhost:1").unwrap(), "http://localhost:1");
        assert_eq!(check_url("https://[::1]:8443").unwrap(), "https://[::1]:8443");
        for bad in [
            "",
            "gw.example.org",
            "ftp://gw.example.org",
            "http://gw.example.org",
            "http://192.168.1.5:8080",
            "https://user:pw@gw.example.org",
            "https://gw.example.org/v1",
            "https://gw.example.org?x=1",
            "https://gw.example.org#x",
            "https://gw.example.org:0",
            "https://gw.example.org:99999",
            "https://gw.example.org:",
            "https://gw example.org",
            "https://gw.example.org\\@evil.org",
            "https://gw%2eexample.org",
            "https://-gw.example.org",
            "https://[zz]:1",
            "javascript:alert(1)",
        ] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_keys_and_cas_are_checked() {
        assert!(check_name("  My gateway ").is_ok());
        assert!(check_name("").is_err() && check_name("a\nb").is_err() && check_name(&"x".repeat(65)).is_err());
        assert!(check_key("p2p_abcdef0123").is_ok());
        assert!(check_key("short").is_err() && check_key("has space inside").is_err());
        assert!(check_key("abc\r\nX-Evil: 1").is_err());
        assert!(check_ca(CA).is_ok());
        assert!(check_ca("").is_err());
        assert!(check_ca("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n").is_err());
        assert!(check_ca(&format!("{CA}junk\n")).is_err());
        assert!(check_ca("-----BEGIN CERTIFICATE-----\nAAAA\n").is_err());
    }

    #[test]
    fn add_list_activate_remove_keeps_keys_out_of_the_file() {
        let d = dir("crud");
        let s = GatewayStore::new(&d);
        let sec = MemSecrets::default();
        assert_eq!(s.load().unwrap(), GatewayList::default());
        let a = s.add(&sec, "Home", "https://gw.example.org", Kind::Manual, None, Some("p2p_secret_key_1")).unwrap();
        let b = s.add(&sec, "Dev", "http://127.0.0.1:8080", Kind::Manual, Some(CA), None).unwrap();
        assert_eq!(s.add(&sec, "Again", "https://gw.example.org/", Kind::Manual, None, None), Err(GwError::Duplicate));
        let (views, active) = s.views(&sec).unwrap();
        assert_eq!(active.as_deref(), Some(a.as_str()));
        assert!(views[0].has_key && !views[0].dev && !views[0].pinned);
        assert!(!views[1].has_key && views[1].dev && views[1].pinned);
        // SECURITY: the key is in the secret store only, never in the saved file.
        let file = std::fs::read_to_string(d.join(FILE)).unwrap();
        assert!(!file.contains("p2p_secret_key_1"));
        s.set_active(&b).unwrap();
        s.set_key(&sec, &b, "another_key_22").unwrap();
        assert_eq!(sec.get(&b).unwrap().as_deref(), Some("another_key_22"));
        s.remove(&sec, &b).unwrap();
        assert_eq!(sec.get(&b).unwrap(), None);
        assert_eq!(s.load().unwrap().active.as_deref(), Some(a.as_str()));
        assert_eq!(s.set_active("0123456789abcdef"), Err(GwError::NotFound));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn an_edited_file_with_a_bad_url_is_refused() {
        let d = dir("tamper");
        let bad = r#"{"gateways":[{"id":"0123456789abcdef","name":"x","url":"http://10.0.0.1","kind":"manual"}]}"#;
        std::fs::write(d.join(FILE), bad).unwrap();
        assert_eq!(GatewayStore::new(&d).load(), Err(GwError::Store(StoreError::Bad)));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn setup_folder_gives_port_and_ca_but_never_secrets() {
        let d = dir("setup");
        assert_eq!(read_setup(&d), Err(GwError::BadSetup));
        std::fs::write(d.join(".env"), "GATEWAY_API_PORT=9000\nOTHER=1\nGATEWAY_API_PORT=9443\n").unwrap();
        std::fs::create_dir_all(d.join("tls/ca")).unwrap();
        std::fs::write(d.join("tls/ca/ca.pem"), CA).unwrap();
        let (url, ca) = read_setup(&d).unwrap();
        assert_eq!(url, "https://127.0.0.1:9443");
        assert_eq!(ca, CA);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn error_messages_carry_no_input() {
        for e in [GwError::BadUrl, GwError::BadKey, GwError::BadCa, GwError::Keychain, GwError::BadSetup] {
            assert!(!e.message().contains("p2p_"));
        }
    }
}
