//! TandemForge on this computer as a model engine (phase I7, docs/features/tandemforge-engine.md).
//!
//! Purpose: a running TandemForge desktop app can lend its local models to the IDE. The owner connects the IDE in
//! TandemForge (Settings > Connected apps), which shows a pairing code once; the person pastes it here. TandemForge
//! then answers the same OpenAI-style API the IDE uses for gateways (`/v1/models`, `/v1/chat/completions`), so chat
//! and the agent work unchanged. TandemForge picks a free port at each launch and writes it to `engine.json` in its
//! data folder while an app is connected; this module finds that file and checks the pairing code's shape.
//!
//! Inputs and trust boundaries: `engine.json` is written by another program and is untrusted (read capped, parsed
//! strictly, only a port taken from it). The pairing code is typed by the person; it is stored only in the OS
//! keychain (see gateways.rs) and sent only to 127.0.0.1.
//!
//! Security: the URL built here is always `http://127.0.0.1:<port>` (loopback only; nothing leaves the computer,
//! NIST SC-7); a port below 1024 is refused (CWE-20); the file read is capped at 4 KiB (CWE-400); errors never
//! include paths (CWE-209).
//! Dependencies: serde_json (already used by the crate).

use std::path::{Path, PathBuf};

use crate::store::{self, StoreError};

/// TandemForge's desktop app identifier; its data folder is named after it on every OS.
pub const APP_ID: &str = "com.tandemforge.desktop";
/// The file TandemForge writes while an app is connected.
pub const ENGINE_FILE: &str = "engine.json";
/// The engine file format this IDE understands.
pub const ENGINE_API: u64 = 1;
/// Prefix of TandemForge pairing codes.
pub const CODE_PREFIX: &str = "tfa_";
const MAX_FILE: u64 = 4096;

/// Fixed errors; `message()` is safe to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TfError {
    /// No engine file: TandemForge is not running, or no app is connected in it.
    NotRunning,
    /// The engine file is damaged or from a newer TandemForge.
    BadFile,
    /// The pairing code does not look like one.
    BadCode,
}

impl TfError {
    pub fn message(&self) -> &'static str {
        match self {
            TfError::NotRunning => {
                "TandemForge is not running on this computer, or no app is connected in it (Settings > Connected apps)"
            }
            TfError::BadFile => "TandemForge's engine file could not be read; update TandemForge or restart it",
            TfError::BadCode => {
                "paste the pairing code from TandemForge (Settings > Connected apps); it starts with tfa_"
            }
        }
    }
}

/// The folders where TandemForge may keep its data, most likely first. `home`, `xdg` and `appdata` come from the
/// environment in [`data_dirs`]; split out so tests can pass their own.
pub fn data_dirs_from(home: Option<&Path>, xdg_data: Option<&Path>, appdata: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(windows) {
        if let Some(a) = appdata {
            out.push(a.join(APP_ID));
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = home {
            out.push(h.join("Library").join("Application Support").join(APP_ID));
        }
    } else {
        if let Some(x) = xdg_data.filter(|p| p.is_absolute()) {
            out.push(x.join(APP_ID));
        }
        if let Some(h) = home {
            out.push(h.join(".local").join("share").join(APP_ID));
        }
    }
    out
}

/// [`data_dirs_from`] with the real environment.
pub fn data_dirs() -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let home = env(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    data_dirs_from(home.as_deref(), env("XDG_DATA_HOME").as_deref(), env("APPDATA").as_deref())
}

/// Reads the port from `dir/engine.json`.
pub fn read_engine(dir: &Path) -> Result<u16, TfError> {
    let bytes = match store::read_capped(&dir.join(ENGINE_FILE), MAX_FILE) {
        Ok(b) => b,
        Err(StoreError::Missing) => return Err(TfError::NotRunning),
        Err(_) => return Err(TfError::BadFile),
    };
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| TfError::BadFile)?;
    if v.get("api").and_then(serde_json::Value::as_u64) != Some(ENGINE_API) {
        return Err(TfError::BadFile);
    }
    let port = v.get("port").and_then(serde_json::Value::as_u64).ok_or(TfError::BadFile)?;
    // SECURITY: an unprivileged port only; anything else is not TandemForge's backend (CWE-20).
    match u16::try_from(port) {
        Ok(p) if p >= 1024 => Ok(p),
        _ => Err(TfError::BadFile),
    }
}

/// The URL of the TandemForge engine running now, from the first folder that has an engine file.
pub fn find_in(dirs: &[PathBuf]) -> Result<String, TfError> {
    let mut last = TfError::NotRunning;
    for d in dirs {
        match read_engine(d) {
            Ok(port) => return Ok(url_for(port)),
            Err(TfError::NotRunning) => {}
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// [`find_in`] over [`data_dirs`].
pub fn find() -> Result<String, TfError> {
    find_in(&data_dirs())
}

/// SECURITY: loopback IPv4 only, so the pairing code never leaves this computer (NIST SC-7).
pub fn url_for(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// True for a URL [`url_for`] could have made.
pub fn is_engine_url(url: &str) -> bool {
    url.strip_prefix("http://127.0.0.1:")
        .and_then(|p| p.parse::<u16>().ok())
        .is_some_and(|p| p >= 1024 && url == url_for(p))
}

/// Checks a pairing code: `tfa_` and 43 base64url characters (TandemForge makes 32 random bytes).
pub fn check_code(code: &str) -> Result<String, TfError> {
    let c = code.trim();
    let body = c.strip_prefix(CODE_PREFIX).ok_or(TfError::BadCode)?;
    if body.len() < 40 || body.len() > 100 || !body.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(TfError::BadCode);
    }
    Ok(c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tf-engine-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_the_port_strictly() {
        let d = tmp("port");
        assert_eq!(read_engine(&d), Err(TfError::NotRunning));
        std::fs::write(d.join(ENGINE_FILE), r#"{"api":1,"port":41234,"pid":7}"#).unwrap();
        assert_eq!(read_engine(&d), Ok(41234));
        assert_eq!(find_in(&[tmp("none"), d.clone()]), Ok("http://127.0.0.1:41234".to_string()));
        for bad in [r#"{"api":2,"port":41234}"#, r#"{"api":1,"port":80}"#, r#"{"api":1,"port":70000}"#, "nope", "{}"] {
            std::fs::write(d.join(ENGINE_FILE), bad).unwrap();
            assert_eq!(read_engine(&d), Err(TfError::BadFile), "{bad}");
        }
        std::fs::write(d.join(ENGINE_FILE), vec![b' '; 5000]).unwrap();
        assert_eq!(read_engine(&d), Err(TfError::BadFile));
    }

    #[test]
    fn codes_and_urls() {
        let good = format!("tfa_{}", "A".repeat(43));
        assert_eq!(check_code(&format!("  {good}\n")), Ok(good.clone()));
        for bad in
            ["", "tfa_short", "sk-123456789012345678901234567890123456789012345", &format!("tfa_{}!", "A".repeat(43))]
        {
            assert_eq!(check_code(bad), Err(TfError::BadCode), "{bad}");
        }
        assert!(is_engine_url("http://127.0.0.1:41234"));
        assert!(!is_engine_url("http://127.0.0.1:80") && !is_engine_url("http://localhost:41234"));
        assert!(!is_engine_url("https://127.0.0.1:41234") && !is_engine_url("http://127.0.0.1:41234/x"));
    }

    #[test]
    fn data_dirs_per_os() {
        let dirs =
            data_dirs_from(Some(Path::new("/home/u")), Some(Path::new("/x/data")), Some(Path::new("C:/AppData")));
        assert!(dirs.iter().all(|d| d.ends_with(APP_ID)));
        if cfg!(target_os = "linux") {
            assert_eq!(dirs[0], Path::new("/x/data").join(APP_ID));
            assert_eq!(dirs[1], Path::new("/home/u/.local/share").join(APP_ID));
            // A relative XDG_DATA_HOME is ignored, as the XDG spec says.
            assert_eq!(data_dirs_from(Some(Path::new("/h")), Some(Path::new("rel")), None).len(), 1);
        }
    }
}
