//! store -- small JSON files in the app's own data folder (saved gateways, chat history), phase I2.
//!
//! Purpose: one place that reads and writes the IDE's own state files, so every caller gets the same size caps,
//! atomic replace and file permissions. The folder comes from the Tauri shell (the OS app-data folder), never from
//! the webview.
//! Inputs: a folder chosen by the shell and a file name built by this crate's callers from allow-listed parts.
//! Security controls:
//!   * Writes go to a `create_new` temp file in the same folder, are flushed and synced, then renamed over the
//!     target, so a crash never leaves a half-written file (CWE-362) and a planted file or link at the temp name
//!     makes the write fail instead of being followed (CWE-59).
//!   * On Unix the files are created 0600 and the folders 0700: chat history can hold private code (CWE-276,
//!     NIST AC-3). On Windows the per-user app-data folder already has a per-user ACL.
//!   * Reads refuse links and anything larger than the caller's cap (CWE-59, CWE-400).
//!   * Errors are fixed strings with no paths or OS text (CWE-209).
//!
//! Dependencies: std only.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fixed error kinds for the store; `message()` is safe to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The file is missing (callers usually treat this as "empty").
    Missing,
    /// The file is larger than allowed, is a link, or is not valid for its format.
    Bad,
    /// Any other file-system failure.
    Io,
}

impl StoreError {
    /// A short message with no path or OS detail (CWE-209).
    pub fn message(&self) -> &'static str {
        match self {
            StoreError::Missing => "the saved file does not exist",
            StoreError::Bad => "a saved file is damaged or too large",
            StoreError::Io => "could not read or write the app's data folder",
        }
    }
}

/// Seconds since 1970, for timestamps in saved records.
pub fn now_s() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Makes `dir` (and its parents) if missing. On Unix the folder is 0700.
pub fn ensure_dir(dir: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(dir).map_err(|_| StoreError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // SECURITY: only the owner may list or read the app's state (CWE-276).
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Reads a whole file of at most `max` bytes. Links are refused.
pub fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>, StoreError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::Missing),
        Err(_) => return Err(StoreError::Io),
    };
    // SECURITY: a regular file only, never a link (CWE-59), and bounded (CWE-400).
    if !meta.is_file() || meta.len() > max {
        return Err(StoreError::Bad);
    }
    let mut buf = Vec::with_capacity(meta.len() as usize);
    fs::File::open(path).and_then(|f| f.take(max + 1).read_to_end(&mut buf)).map_err(|_| StoreError::Io)?;
    if buf.len() as u64 > max {
        return Err(StoreError::Bad);
    }
    Ok(buf)
}

/// Replaces `path` with `bytes` atomically (temp file in the same folder, then rename).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let dir = path.parent().ok_or(StoreError::Io)?;
    ensure_dir(dir)?;
    let tmp = temp_name(path);
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // SECURITY: owner read/write only (CWE-276).
        opts.mode(0o600);
    }
    let result = (|| {
        // SECURITY: create_new fails if the name exists, also when it is a link, so the temp file is ours (CWE-59).
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::Io);
    }
    Ok(())
}

/// Removes a file; a missing file is not an error.
pub fn remove(path: &Path) -> Result<(), StoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(StoreError::Io),
    }
}

/// A temp name next to `path`. It only has to be unique (create_new refuses a clash), so no CSPRNG is needed.
fn temp_name(path: &Path) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let name = format!(".p2pide-{}-{}-{}.tmp", std::process::id(), nanos, n);
    path.with_file_name(name)
}

/// An unguessable id: 32 lowercase hex characters from the OS CSPRNG (phase I5b run requests: only the waiting
/// request's own id answers it, CWE-330, CWE-639). Falls back to [`new_id`] twice over if the CSPRNG fails, which
/// keeps ids unique; the approval still needs the person's click.
pub fn random_id() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_ok() {
        return b.iter().map(|x| format!("{x:02x}")).collect();
    }
    format!("{}{}", new_id(), new_id())
}

/// A new record id: 16 lowercase hex characters. Ids name files and must be unique, not secret, so a SHA-256 of
/// time, process id and a counter is enough (no CSPRNG needed; not used for any security decision).
pub fn new_id() -> String {
    use sha2::{Digest, Sha256};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(N.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// True when `id` has the shape [`new_id`] makes. SECURITY: ids become file names, so this allow-list keeps them
/// from naming anything else (CWE-22, CWE-73).
pub fn id_ok(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("p2pide-store-{tag}-{}", new_id()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn write_read_replace_and_caps() {
        let d = tmpdir("rw");
        let p = d.join("a.json");
        assert_eq!(read_capped(&p, 10), Err(StoreError::Missing));
        write_atomic(&p, b"hello").unwrap();
        assert_eq!(read_capped(&p, 10).unwrap(), b"hello");
        write_atomic(&p, b"hello again").unwrap();
        assert_eq!(read_capped(&p, 100).unwrap(), b"hello again");
        assert_eq!(read_capped(&p, 5), Err(StoreError::Bad));
        // No temp files left behind.
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        remove(&p).unwrap();
        remove(&p).unwrap();
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn ids_are_unique_and_checked() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert!(id_ok(&a) && id_ok(&b));
        for bad in ["", "../etc/passwd0000", "ABCDEF0123456789", "0123456789abcde", "0123456789abcdefg"] {
            assert!(!id_ok(bad), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn files_are_owner_only_and_links_refused() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmpdir("perm");
        let p = d.join("a.json");
        write_atomic(&p, b"x").unwrap();
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        let l = d.join("link.json");
        std::os::unix::fs::symlink(&p, &l).unwrap();
        assert_eq!(read_capped(&l, 10), Err(StoreError::Bad));
        fs::remove_dir_all(&d).unwrap();
    }
}
