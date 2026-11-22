//! Phase I6c-3: the real Node extension host (development check, ignored by default; run on the PC).
//!
//! Downloads the pinned VSCodium server for this computer from GitHub, checks its SHA-256, unpacks it, trusts a temp
//! folder, starts the server through `node_host::start` and checks, over 127.0.0.1 only:
//!   * the server reports the pinned commit (`/version`), so the editor's announced commit matches;
//!   * a request without the connection token is refused, and the same request with it is accepted;
//!   * stopping it closes the port.
//!
//! Security: needs the network only for the pinned download (through `node_host::real_get`: https, GitHub hosts only,
//! public addresses only). Nothing is written outside a temp folder. Run with
//! `cargo test --test node_host_real -- --ignored --nocapture`.
use p2p_ide_core::extensions::ExtStore;
use p2p_ide_core::node_host::{self, Launcher, TrustStore};
use std::io::Read;
use std::time::Duration;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("p2pide-reh-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A plain HTTP GET to the local server (status, body). Test only: 127.0.0.1, no proxy.
fn get(url: &str) -> (u16, String) {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .proxy(None)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(10)))
            .build(),
    );
    let r = agent.get(url).call().expect("local request");
    let status = r.status().as_u16();
    let mut body = String::new();
    let _ = r.into_body().into_reader().take(1 << 16).read_to_string(&mut body);
    (status, body)
}

#[test]
#[ignore = "downloads the real pinned server (about 90 MB); run on the PC"]
fn the_pinned_server_downloads_starts_and_needs_its_token() {
    let pin = node_host::pin_for(node_host::PINS, std::env::consts::OS, std::env::consts::ARCH)
        .expect("a pin for this computer");
    let data = tmp("data");
    let folder = tmp("folder");
    std::fs::write(folder.join("hello.py"), "print('hi')\n").unwrap();
    let t0 = std::time::Instant::now();
    let root = node_host::ensure_installed(&node_host::real_get(), &data, &pin).expect("download, check, unpack");
    println!("installed {} in {:?} at <data>/node-host", pin.version, t0.elapsed());
    let trust = TrustStore::new(&data);
    trust.set(&folder, true).unwrap();
    assert_eq!(
        node_host::stage_extensions(&ExtStore::new(data.join("extensions")), &data.join("node-extensions")).unwrap(),
        0
    );
    let t1 = std::time::Instant::now();
    let run = node_host::start(&Launcher::for_root(&root), &data, &trust, &folder).expect("server starts");
    println!("listening on 127.0.0.1:<port> after {:?}", t1.elapsed());
    let base = format!("http://127.0.0.1:{}", run.port);

    let (st, body) = get(&format!("{base}/version"));
    println!("/version -> {st} {body}");
    assert_eq!(st, 200);
    assert_eq!(body.trim(), node_host::SERVER_COMMIT, "the server's commit is the pinned one");

    let (no_token, _) = get(&format!("{base}/"));
    let (with_token, _) = get(&format!("{base}/?tkn={}", run.token));
    println!("/ without token -> {no_token}, with token -> {with_token}");
    assert_eq!(no_token, 403, "no token, no access");
    assert_ne!(with_token, 403, "the token is accepted");

    let port = run.port;
    run.stop();
    std::thread::sleep(Duration::from_millis(500));
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "stopping closes the port");
    let _ = std::fs::remove_dir_all(&data);
    let _ = std::fs::remove_dir_all(&folder);
}
