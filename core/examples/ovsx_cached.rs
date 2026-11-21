//! ovsx_cached -- runs the real install path (hash, Ed25519 signature, manifest, unpack) on Open VSX files that
//! were downloaded beforehand into a cache folder (one file per URL, named by the URL's SHA-256). Used where the
//! machine reaches the internet only through a proxy, which the IDE's own client does not use.
//! Run: cargo run --example ovsx_cached -- <cache folder>
use p2p_ide_core::extensions::{search, ExtError, ExtStore, Kind};
use sha2::{Digest, Sha256};

fn main() {
    let cache = std::path::PathBuf::from(std::env::args().nth(1).expect("cache folder"));
    let get = move |url: &str, max: u64| -> Result<Vec<u8>, ExtError> {
        let name: String = Sha256::digest(url.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let b = std::fs::read(cache.join(name)).map_err(|_| ExtError::Network)?;
        if b.len() as u64 > max {
            return Err(ExtError::TooLarge);
        }
        Ok(b)
    };
    let found = search(&get, "dracula", 0).unwrap();
    println!(
        "search: {} results, dracula-theme listed: {}",
        found.len(),
        found.iter().any(|s| s.namespace == "dracula-theme")
    );
    let dir = std::env::temp_dir().join(format!("ovsx-cached-{}", std::process::id()));
    let store = ExtStore::new(dir.clone());
    let e = store.install(&get, "dracula-theme", "theme-dracula", "latest").unwrap();
    let m = store.manifest(&e.id).unwrap();
    let theme = m["contributes"]["themes"][0]["path"].as_str().unwrap().to_string();
    println!(
        "installed {} {} kind {:?} verified_publisher {} sha256 {} theme file {} bytes",
        e.id,
        e.version,
        e.kind,
        e.verified_publisher,
        e.sha256,
        store.read_file(&e.id, &theme).unwrap().len()
    );
    let icons = store.install(&get, "PKief", "material-icon-theme", "latest");
    println!("material-icon-theme: {:?}", icons.as_ref().map(|e| (&e.id, &e.version, e.kind)).map_err(|e| e.message()));
    let yaml = store.install(&get, "redhat", "vscode-yaml", "latest").unwrap_err();
    assert_eq!(yaml, ExtError::NeedsHost(Kind::Node));
    println!("redhat.vscode-yaml refused: {}", yaml.message());
    let _ = std::fs::remove_dir_all(dir);
}
