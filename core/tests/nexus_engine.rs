//! nexus_engine -- live end-to-end check against real NEXUS Proxy and NEXUS Runtime (not mocked in Rust).
//!
//! Starts no processes itself: `scripts/live_nexus_e2e.py` (or the developer) starts `nexus-proxy` and
//! `nexus-runtime` on loopback, optionally sets `NEXUS_PROXY_URL` (default `http://127.0.0.1:11435`),
//! `NEXUS_RUNTIME_URL` (default `http://127.0.0.1:11436`), and `NEXUS_AGENT_TOOLS=1` when `nexus-proxy` points at
//! an upstream with tool calling, then runs:
//!
//!   cargo test --test nexus_engine -- --ignored --nocapture
//!
//! Security: loopback URLs only (`127.0.0.1`, checked with `nexus::is_engine_url`); temporary workspace and state
//! folder are deleted when the test finishes; no keys are read or stored (CWE-918, CWE-532).

use p2p_ide_core::agent::{self, Pending};
use p2p_ide_core::client::{Client, Msg};
use p2p_ide_core::gateways::{GatewayStore, Kind};
use p2p_ide_core::nexus::{self, ServiceKind};
use p2p_ide_core::store;
use p2p_ide_core::workspace::Workspace;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("p2pide-nexus-e2e-{tag}-{}", store::new_id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn env_url(name: &str, default: &str) -> String {
    let url = std::env::var(name).unwrap_or_else(|_| default.to_string());
    assert!(nexus::is_engine_url(&url), "{name} must be http://127.0.0.1:<port>: {url}");
    url
}

#[test]
#[ignore = "needs running nexus-proxy and nexus-runtime backends; run via scripts/live_nexus_e2e.py"]
fn ide_core_uses_real_nexus_proxy_and_runtime() {
    let proxy_url = env_url("NEXUS_PROXY_URL", "http://127.0.0.1:11435");
    let runtime_url = env_url("NEXUS_RUNTIME_URL", "http://127.0.0.1:11436");
    let run_tools = std::env::var("NEXUS_AGENT_TOOLS").as_deref() == Ok("1");

    // 1. Discover & verify both services via /health + telemetry endpoints.
    let proxy_probe = nexus::probe_endpoint(&proxy_url, ServiceKind::Proxy).expect("proxy health");
    assert!(proxy_probe.running);
    assert!(proxy_probe.airlock_threshold.is_some(), "missing airlock_threshold: {proxy_probe:?}");

    let runtime_probe = nexus::probe_endpoint(&runtime_url, ServiceKind::Runtime).expect("runtime health");
    assert!(runtime_probe.running);
    assert!(runtime_probe.simd.is_some(), "missing runtime simd telemetry: {runtime_probe:?}");

    // Wrong service check must still fail closed.
    assert_eq!(nexus::probe_endpoint(&proxy_url, ServiceKind::Runtime), Err(nexus::NexusError::UnexpectedService));

    // 2. Save both entries keyless in GatewayStore and open Client without any API key.
    let state_dir = TempDir::new("state");
    let store = GatewayStore::new(&state_dir.0);
    let runtime_id = store.save_nexus(&runtime_url, &ServiceKind::Runtime.display_name(&runtime_url)).unwrap();
    let proxy_id = store.save_nexus(&proxy_url, &ServiceKind::Proxy.display_name(&proxy_url)).unwrap();

    let runtime_gw = store.get(&runtime_id).unwrap();
    assert_eq!(runtime_gw.kind, Kind::Nexus);
    assert!(!runtime_gw.dev());
    let runtime_client = Client::new(&runtime_gw, None).unwrap();
    let (runtime_models, _) = runtime_client.models().expect("runtime /v1/models");
    println!("PASS runtime health & /v1/models (models={runtime_models:?}, simd={:?})", runtime_probe.simd);

    let proxy_gw = store.get(&proxy_id).unwrap();
    assert_eq!(proxy_gw.kind, Kind::Nexus);
    assert!(!proxy_gw.dev());
    let proxy_client = Client::new(&proxy_gw, None).unwrap();
    let (proxy_models, latency_ms) = proxy_client.models().expect("proxy /v1/models");
    assert!(!proxy_models.is_empty(), "expected at least one model from nexus-proxy");
    println!("PASS proxy health & /v1/models ({latency_ms} ms, models={proxy_models:?})");

    // 3. Stream a plain chat turn through real nexus-proxy.
    let model = proxy_models[0].clone();
    let mut streamed = String::new();
    let end = proxy_client
        .chat_stream(
            &model,
            &[Msg { role: "user".into(), content: "Explain token packing briefly.".into() }],
            &mut |delta| {
                streamed.push_str(delta);
                true
            },
        )
        .expect("proxy chat stream");
    assert!(!streamed.trim().is_empty(), "expected non-empty stream from nexus-proxy");
    assert_eq!(end.finish_reason.as_deref(), Some("stop"));
    println!("PASS proxy SSE chat_stream ({} chars)", streamed.len());

    // 4. Optional full agent tool turn (read_file -> edit_file -> pending review) when live tool upstream is wired.
    if run_tools {
        let ws_dir = TempDir::new("ws");
        std::fs::write(ws_dir.0.join("app.py"), "def greeting():\n    return \"old\"\n").unwrap();
        std::fs::write(ws_dir.0.join(".env"), "SECRET_KEY=never_send_me\n").unwrap();
        let ws = Workspace::open(&ws_dir.0).unwrap();
        let mut pending = Pending::default();
        let events = Arc::new(Mutex::new(Vec::<agent::AgentEvent>::new()));
        let ev_copy = Arc::clone(&events);
        let history = [Msg { role: "user".into(), content: "Update greeting() in app.py to return \"fixed\".".into() }];
        let mut step = |msgs: &[serde_json::Value], on_delta: &mut dyn FnMut(&str) -> bool| {
            proxy_client.agent_step(&model, msgs, &agent::tool_defs_for(false), on_delta)
        };
        let mut cb = |e: agent::AgentEvent| {
            ev_copy.lock().unwrap().push(e);
            true
        };
        let turn = agent::run_turn(&ws, &mut pending, &history, &mut step, None, &mut cb).expect("agent run_turn");
        assert!(turn.tool_calls >= 2, "expected read_file + edit_file tool calls, got {turn:?}");
        let edit = pending.files.get("app.py").expect("pending edit for app.py");
        assert!(edit.after.contains("fixed"), "expected fixed content in pending edit: {:?}", edit.after);
        // Workspace on disk must still be untouched before Accept.
        assert_eq!(std::fs::read_to_string(ws_dir.0.join("app.py")).unwrap(), "def greeting():\n    return \"old\"\n");
        println!(
            "PASS proxy agent tool turn (steps={}, tool_calls={}, pending={})",
            turn.steps, turn.tool_calls, turn.pending
        );
    }
}
