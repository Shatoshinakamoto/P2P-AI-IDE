//! End-to-end test for phase I7: the IDE core against a real, running TandemForge backend.
//!
//! Purpose: proves that the IDE finds TandemForge through its `engine.json`, that a pairing code TandemForge made is
//! accepted and a wrong one refused, and that chat and an agent step (with a tool call and its result) work through
//! TandemForge's `/v1` engine API exactly as through a gateway.
//!
//! How to run: start TandemForge's backend (its models may come from a fake Ollama), connect an app in it, then
//!   TF_ENGINE_DIR=<folder holding engine.json> TF_CODE=<pairing code> \
//!     cargo test --test tandemforge_engine -- --ignored --nocapture
//! The test is `#[ignore]` because it needs that backend; `cargo test` alone skips it.
//!
//! Security checks: only `http://127.0.0.1:<port>` is used (from `tandemforge::find_in`); a wrong code gets
//! `NetError::Auth`; the code is read from the environment and never printed (CWE-532).
//! Dependencies: serde_json and the core crate.

use p2p_ide_core::client::{Client, Msg, NetError};
use p2p_ide_core::gateways::{Gateway, Kind};
use p2p_ide_core::tandemforge;
use serde_json::json;
use std::path::PathBuf;

fn engine() -> (String, String) {
    let dir = PathBuf::from(std::env::var("TF_ENGINE_DIR").expect("set TF_ENGINE_DIR"));
    let code = tandemforge::check_code(&std::env::var("TF_CODE").expect("set TF_CODE")).expect("a tfa_ code");
    let url = tandemforge::find_in(&[dir]).expect("engine.json found");
    assert!(tandemforge::is_engine_url(&url), "loopback engine URL only");
    (url, code)
}

fn client(url: &str, code: &str) -> Client {
    let gw = Gateway {
        id: "00000000000000aa".into(),
        name: "TandemForge (this computer)".into(),
        url: url.into(),
        kind: Kind::TandemForge,
        ca_pem: None,
    };
    Client::new(&gw, Some(code.into())).unwrap()
}

#[test]
#[ignore = "needs a running TandemForge backend; see the file header"]
fn ide_core_uses_tandemforge_as_its_engine() {
    let (url, code) = engine();
    println!("engine at {url}");

    // A wrong code is refused before any model is reached.
    let wrong = format!("tfa_{}", "W".repeat(43));
    match client(&url, &wrong).models() {
        Err(NetError::Auth) => println!("PASS wrong pairing code refused (401)"),
        other => panic!("a wrong code must be refused, got {other:?}"),
    }

    // Models: chat models only (TandemForge leaves embedding models out).
    let c = client(&url, &code);
    let (models, ms) = c.models().expect("models");
    println!("PASS models {models:?} in {ms} ms");
    assert!(models.iter().any(|m| m == "tiny-chat:1b"), "{models:?}");
    assert!(!models.iter().any(|m| m.contains("embed")), "{models:?}");

    // Chat, streamed; text stays text.
    let mut text = String::new();
    let end = c
        .chat_stream("tiny-chat:1b", &[Msg { role: "user".into(), content: "hello".into() }], &mut |d| {
            text.push_str(d);
            true
        })
        .expect("chat");
    println!("PASS chat {text:?} finish {:?}", end.finish_reason);
    assert!(text.contains("Hello from tiny-chat"));
    assert_eq!(end.finish_reason.as_deref(), Some("stop"));

    // A slow model: TandemForge sends keep-alive comments while waiting; the answer still arrives.
    let mut slow = String::new();
    c.chat_stream("tiny-chat:1b", &[Msg { role: "user".into(), content: "slow please".into() }], &mut |d| {
        slow.push_str(d);
        true
    })
    .expect("slow chat");
    assert!(slow.contains("Hello"), "{slow}");
    println!("PASS slow reply after keep-alives");

    // Agent step 1: the model asks for a tool.
    let tools = json!([{"type": "function", "function": {"name": "read_file", "description": "Read a file",
        "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}}]);
    let mut msgs = vec![json!({"role": "user", "content": "what does the readme say?"})];
    let (end, calls) = c.agent_step("tiny-chat:1b", &msgs, &tools, &mut |_| true).expect("agent step 1");
    assert_eq!(end.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
    let args: serde_json::Value = serde_json::from_str(&calls[0].arguments).expect("arguments are JSON text");
    assert_eq!(args["path"], "README.md");
    println!("PASS agent asked for read_file {}", calls[0].arguments);

    // Agent step 2: the tool result goes back (arguments as the JSON string OpenAI uses) and the model answers.
    msgs.push(json!({"role": "assistant", "content": "", "tool_calls": [{"id": calls[0].id, "type": "function",
        "function": {"name": "read_file", "arguments": calls[0].arguments}}]}));
    msgs.push(json!({"role": "tool", "tool_call_id": calls[0].id, "content": "# Shop API\nA tiny shop."}));
    let mut answer = String::new();
    let (end, calls) = c
        .agent_step("tiny-chat:1b", &msgs, &tools, &mut |d| {
            answer.push_str(d);
            true
        })
        .expect("agent step 2");
    assert!(calls.is_empty());
    assert_eq!(end.finish_reason.as_deref(), Some("stop"));
    assert!(answer.contains("# Shop API"), "{answer}");
    println!("PASS agent answered with the tool result {answer:?}");

    // A model on another provider is refused: TandemForge never forwards to a gateway.
    let r = c.chat_stream("p2pai:llama3", &[Msg { role: "user".into(), content: "hi".into() }], &mut |_| true);
    assert!(r.is_err(), "a gateway model must be refused");
    println!("PASS gateway model refused: {:?}", r.err().map(|e| e.message()));
}
