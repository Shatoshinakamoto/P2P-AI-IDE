//! node_host_serve.rs -- development check: installs a locally built Node extension host server (a `.tar.gz` from
//! `scripts/build_node_host.py`) through the core's own checks and starts it for a folder, then prints where to
//! connect as one JSON line and runs until stdin closes.
//!
//! Purpose: lets the UI end-to-end check (app/tests/ui-smoke.mjs with `NODE_HOST_REAL`) and a person on a PC talk
//! to a real server built from the editor's VS Code version before it is published and pinned.
//!
//! Inputs: `--tgz <file> --sha256 <64 hex> --data <dir> --folder <dir>` (all from the developer's command line).
//! Phase I6d: add `--electron-zip <file> --electron-sha256 <64 hex>` to run the server with that Electron in Node mode
//! (the compatibility host), through the same checks as `electron_host::ensure_installed` (hash, safe unzip).
//! Output: `{"port":…,"token":…,"folder":…,"commit":…,"vscode":…}` on stdout. SECURITY-WARNING: the connection token is
//! printed for the test harness on this machine only; never run this where stdout is logged or shared (CWE-532).
//! Security: the archive goes through the same SHA-256 check, safe unpack, folder trust and scrubbed start as a real
//! download (`node_host::ensure_installed`, `node_host::start`; NIST SI-7, CWE-494, CWE-22, CWE-829). Nothing is
//! downloaded. Not part of the app.
//!
//! Run: `cargo run --example node_host_serve -- --tgz out/p2pide-node-host-linux-x64-1.145.2.tar.gz --sha256 <hex>
//! --data /tmp/nh-data --folder /tmp/nh-folder`
use p2p_ide_core::extensions::ExtStore;
use p2p_ide_core::node_host::{self, HostError, Launcher, Pin, TrustStore};
use std::io::Read;
use std::path::PathBuf;

fn opt(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn arg(name: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn main() {
    let tgz = PathBuf::from(arg("--tgz"));
    let sha: &'static str = Box::leak(arg("--sha256").into_boxed_str());
    let data = PathBuf::from(arg("--data"));
    let folder = std::fs::canonicalize(arg("--folder")).expect("folder");
    // A pin for this one local file. The URL is only a label: the local `get` below never touches the network.
    let pin = Pin {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        version: "local-build",
        url: "https://github.com/local/node-host.tar.gz",
        sha256: sha,
        commit: "0000000000000000000000000000000000000000",
    };
    let get = |_url: &str, max: u64| -> Result<Vec<u8>, HostError> {
        let mut f = std::fs::File::open(&tgz).map_err(|_| HostError::Io)?;
        let mut b = Vec::new();
        // SECURITY: the same size limit as a real download (CWE-400).
        f.by_ref().take(max + 1).read_to_end(&mut b).map_err(|_| HostError::Io)?;
        if b.len() as u64 > max {
            return Err(HostError::TooLarge);
        }
        Ok(b)
    };
    let root = node_host::ensure_installed(&get, &data, &pin).expect("install (hash check and unpack)");
    let product: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("product.json")).expect("product.json")).expect("json");
    let trust = TrustStore::new(&data);
    trust.set(&folder, true).expect("trust");
    node_host::stage_extensions(&ExtStore::new(data.join("extensions")), &data.join("node-extensions")).expect("stage");
    let launcher = match (opt("--electron-zip"), opt("--electron-sha256")) {
        (Some(zip), Some(sha)) => {
            let zip = PathBuf::from(zip);
            let epin = p2p_ide_core::electron_host::ElectronPin {
                sha256: Box::leak(sha.into_boxed_str()),
                ..p2p_ide_core::electron_host::this_pin().expect("an Electron pin for this computer")
            };
            let eget = |_url: &str, max: u64| -> Result<Vec<u8>, HostError> {
                let mut f = std::fs::File::open(&zip).map_err(|_| HostError::Io)?;
                let mut b = Vec::new();
                f.by_ref().take(max + 1).read_to_end(&mut b).map_err(|_| HostError::Io)?;
                if b.len() as u64 > max {
                    return Err(HostError::TooLarge);
                }
                Ok(b)
            };
            let exe = p2p_ide_core::electron_host::ensure_installed(&eget, &data, &epin).expect("electron install");
            p2p_ide_core::electron_host::launcher(&exe, &root)
        }
        _ => Launcher::for_root(&root),
    };
    let run = node_host::start(&launcher, &data, &trust, &folder).expect("start");
    println!(
        "{}",
        serde_json::json!({
            "port": run.port,
            "token": run.token,
            "folder": node_host::editor_folder(&run.folder).expect("folder"),
            "commit": product["commit"],
            "vscode": product["version"],
        })
    );
    // Runs until the harness closes stdin, then stops the server.
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    run.stop();
}
