//! Integration test for phase I4: push the open folder to a fake gateway project and pull changes back.
//!
//! Purpose: exercises `projects::push` and `projects::pull` end to end over real HTTP on 127.0.0.1 against a tiny
//! in-process fake of the Go gateway's project API (`/v1/projects/{id}/workspace`, `/files`, `/commit`, `/sync`,
//! `/v1/files`). Security checks: `.env` is never uploaded or pulled back, commits carry `base_version`, a
//! download that does not match its digest is refused, and nothing is written locally before Accept.
//! Dependencies: std, serde_json, the core crate.

use p2p_ide_core::agent::Pending;
use p2p_ide_core::client::Client;
use p2p_ide_core::gateways::{Gateway, Kind};
use p2p_ide_core::projects::{pull, push};
use p2p_ide_core::workspace::{version_of, Workspace};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

/// The fake gateway's state: path -> content, version, uploads by id, and a log of requests.
#[derive(Default)]
struct Fake {
    files: BTreeMap<String, Vec<u8>>,
    version: i64,
    uploads: BTreeMap<String, Vec<u8>>,
    log: Vec<String>,
    /// Paths whose download is served with different bytes than listed (tamper test).
    tamper: Vec<String>,
    /// Someone else commits just before our commit arrives (lost-update test).
    race: bool,
}

fn respond(s: &mut impl Write, status: &str, body: &[u8]) {
    let _ = write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(body);
}

/// Serves requests until the test ends. One connection per request (Connection: close).
fn serve(state: Arc<Mutex<Fake>>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut c) = conn else { continue };
            let mut r = BufReader::new(c.try_clone().unwrap());
            let mut first = String::new();
            r.read_line(&mut first).unwrap();
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h == "\r\n" || h.is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0u8; len];
            r.read_exact(&mut body).unwrap();
            let mut parts = first.split_whitespace();
            let (m, path) = (parts.next().unwrap().to_string(), parts.next().unwrap().to_string());
            let mut st = state.lock().unwrap();
            st.log.push(format!("{m} {path}"));
            let base = "/v1/projects/prj_1/workspace";
            if m == "GET" && path == base {
                let v = st.version;
                respond(&mut c, "200 OK", json!({"version": v}).to_string().as_bytes());
            } else if m == "GET" && path == format!("{base}/files") {
                let data: Vec<Value> = st
                    .files
                    .iter()
                    .map(|(p, b)| json!({"path": p, "bytes": b.len(), "sha256": version_of(b)}))
                    .collect();
                respond(&mut c, "200 OK", json!({"data": data, "version": st.version}).to_string().as_bytes());
            } else if m == "GET" && path.starts_with(&format!("{base}/files/")) {
                let p = path
                    .trim_start_matches(&format!("{base}/files/"))
                    .trim_end_matches("?download=1")
                    .replace("%20", " ");
                let mut b = st.files.get(&p).cloned().unwrap_or_default();
                if st.tamper.contains(&p) {
                    b.extend_from_slice(b"# tampered\n");
                }
                let _ = write!(c, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len());
                let _ = c.write_all(&b);
            } else if m == "POST" && path == "/v1/files" {
                // Take the bytes between the file part's header and the closing boundary.
                let text = body.clone();
                let start = text.windows(4).enumerate().filter(|(_, w)| *w == b"\r\n\r\n").nth(1).unwrap().0 + 4;
                let tail = &text[start..];
                let cut = tail.windows(4).rposition(|w| w == b"\r\n--").unwrap();
                let id = format!("f{:031}", st.uploads.len());
                st.uploads.insert(id.clone(), tail[..cut].to_vec());
                respond(&mut c, "201 Created", json!({"id": id, "state": "available"}).to_string().as_bytes());
            } else if m == "POST" && path == format!("{base}/commit") {
                let v: Value = serde_json::from_slice(&body).unwrap();
                if st.race {
                    st.race = false;
                    st.version += 1;
                }
                if v["base_version"].as_i64() != Some(st.version) {
                    respond(&mut c, "409 Conflict", br#"{"error":{"message":"x","code":"version_conflict"}}"#);
                    continue;
                }
                for ch in v["changes"].as_array().unwrap() {
                    let p = ch["path"].as_str().unwrap().to_string();
                    if ch["delete"] == json!(true) {
                        st.files.remove(&p);
                    } else {
                        let b = st.uploads[ch["file_id"].as_str().unwrap()].clone();
                        st.files.insert(p, b);
                    }
                }
                st.version += 1;
                let v = st.version;
                respond(&mut c, "200 OK", json!({"version": v}).to_string().as_bytes());
            } else if m == "POST" && path == format!("{base}/sync") {
                respond(&mut c, "202 Accepted", br#"{"object":"workspace.sync","state":"pending"}"#);
            } else {
                respond(&mut c, "404 Not Found", br#"{"error":{"code":"not_found"}}"#);
            }
        }
    });
    url
}

fn client(url: &str) -> Client {
    let gw = Gateway {
        id: "0123456789abcdef".into(),
        name: "fake".into(),
        url: url.into(),
        kind: Kind::Manual,
        ca_pem: None,
    };
    Client::new(&gw, Some("p2p_test_key".into())).unwrap()
}

fn folder() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("p2pide-i4-{}", p2p_ide_core::store::new_id()));
    fs::create_dir_all(d.join("src")).unwrap();
    fs::write(d.join("src/calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
    fs::write(d.join("README.md"), "# demo\n").unwrap();
    fs::write(d.join(".env"), "API_KEY=do-not-send\n").unwrap();
    fs::create_dir_all(d.join("node_modules/x")).unwrap();
    fs::write(d.join("node_modules/x/i.js"), "skip me").unwrap();
    d
}

#[test]
fn push_sends_the_folder_without_secrets_and_pull_brings_changes_for_review() {
    let state = Arc::new(Mutex::new(Fake::default()));
    state.lock().unwrap().files.insert("old.txt".into(), b"gone locally\n".to_vec());
    let url = serve(state.clone());
    let c = client(&url);
    let d = folder();
    let ws = Workspace::open(&d).unwrap();

    let r = push(&c, &ws, "prj_1").unwrap();
    assert_eq!((r.uploaded, r.deleted, r.unchanged), (2, 1, 0), "{r:?}");
    assert_eq!(r.version, 1);
    assert_eq!(r.copy, "pending");
    assert!(r.skipped.iter().any(|s| s.rel == ".env"), "{r:?}");
    {
        let st = state.lock().unwrap();
        // SECURITY: .env never left the computer; node_modules is skipped like in search.
        assert!(st.uploads.values().all(|b| !String::from_utf8_lossy(b).contains("do-not-send")));
        assert_eq!(st.files.keys().cloned().collect::<Vec<_>>(), vec!["README.md", "src/calc.py"]);
        assert_eq!(st.files["src/calc.py"], b"def add(a, b):\n    return a - b\n");
    }
    // A second push changes nothing.
    let r2 = push(&c, &ws, "prj_1").unwrap();
    assert_eq!((r2.uploaded, r2.deleted, r2.unchanged), (0, 0, 2));

    // The mission fixed the file on the gateway, added one, and something tried to plant .env.
    {
        let mut st = state.lock().unwrap();
        st.files.insert("src/calc.py".into(), b"def add(a, b):\n    return a + b\n".to_vec());
        st.files.insert("src/test_calc.py".into(), b"from calc import add\n".to_vec());
        st.files.insert(".env".into(), b"API_KEY=planted\n".to_vec());
        st.files.insert("docs.md".into(), b"x\n".to_vec());
        st.tamper.push("docs.md".into());
    }
    let mut pending = Pending::default();
    let p = pull(&c, &ws, &mut pending, "prj_1").unwrap();
    assert_eq!(p.proposed, 2, "{p:?}");
    assert_eq!(p.same, 1);
    assert!(p.skipped.iter().any(|s| s.rel == ".env"));
    assert!(p.skipped.iter().any(|s| s.rel == "docs.md"), "a download that does not match its digest is refused");
    // Nothing is written before Accept.
    assert_eq!(fs::read_to_string(d.join("src/calc.py")).unwrap(), "def add(a, b):\n    return a - b\n");
    assert_eq!(fs::read_to_string(d.join(".env")).unwrap(), "API_KEY=do-not-send\n");
    assert!(!d.join("src/test_calc.py").exists());
    // Accepting writes the gateway's version.
    pending.accept(&ws, "src/calc.py").unwrap();
    assert_eq!(fs::read_to_string(d.join("src/calc.py")).unwrap(), "def add(a, b):\n    return a + b\n");
    let _ = fs::remove_dir_all(d);
}

#[test]
fn a_commit_on_a_changed_project_is_refused_not_overwritten() {
    let state = Arc::new(Mutex::new(Fake::default()));
    state.lock().unwrap().race = true;
    let url = serve(state.clone());
    let c = client(&url);
    let d = folder();
    let ws = Workspace::open(&d).unwrap();
    let e = push(&c, &ws, "prj_1").unwrap_err();
    assert_eq!(e.message(), "the gateway project changed meanwhile; try again");
    // SECURITY: the other writer's version stands; nothing of ours was committed (CWE-362).
    assert!(state.lock().unwrap().files.is_empty());
    let _ = fs::remove_dir_all(d);
}
