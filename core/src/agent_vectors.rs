//! agent_vectors -- runs the shared agent-tool test cases against the IDE's agent (tests only).
//!
//! Purpose: the VS Code / VSCodium extension (side track X2, ADR-029) has a TypeScript port of these tools
//! (`vscode-extension/src/agent/`), because extensions run in VS Code's Node. Both read the same cases from
//! `core/tests/vectors/agent_tools.json` -- tool definitions, system prompt, path rules, deny list, edit rules, line
//! ranges and argument checks -- so a change to one that the other doesn't match fails a test.
//! Inputs: that file and a temp folder per case. No network.
//! Security: the cases cover path confinement (CWE-22, CWE-73), the secrets deny list (CWE-200), strict tool
//! arguments (CWE-20) and that tools which are not offered are never run (OWASP LLM06).

use super::*;
use std::fs;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../tests/vectors/agent_tools.json")).expect("vectors are JSON")
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_else(|| panic!("{k} in {v}")).to_string()
}

/// A fresh workspace with one file `f.txt` holding `text`.
fn ws_with(text: &str) -> (Workspace, std::path::PathBuf) {
    let d = std::env::temp_dir().join(format!("p2pide-agentvec-{}", crate::store::new_id()));
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("f.txt"), text).unwrap();
    fs::write(d.join("README.md"), "hi\n").unwrap();
    (Workspace::open(&d).unwrap(), d)
}

fn call(name: &str, a: Value) -> ToolCall {
    ToolCall { id: "c1".into(), name: name.into(), arguments: a.to_string() }
}

#[test]
fn tool_defs_and_prompt_match() {
    let v = vectors();
    assert_eq!(v["system_prompt"].as_str().unwrap(), SYSTEM_PROMPT);
    assert_eq!(v["tool_defs"], tool_defs());
}

#[test]
fn rel_path_cases() {
    let (ws, d) = ws_with("");
    for c in vectors()["rel_paths"].as_array().unwrap() {
        let ok = ws.check_rel(&s(c, "path")).is_ok();
        assert_eq!(ok, c["ok"].as_bool().unwrap(), "{}", s(c, "why"));
    }
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn deny_and_norm_cases() {
    let v = vectors();
    for c in v["denied"].as_array().unwrap() {
        assert_eq!(denied(&s(c, "path")), c["denied"].as_bool().unwrap(), "{}", s(c, "why"));
    }
    for c in v["norm_paths"].as_array().unwrap() {
        let mut m = Map::new();
        m.insert("path".into(), Value::String(s(c, "raw")));
        assert_eq!(path_arg(&m).unwrap(), s(c, "want"), "{}", s(c, "why"));
    }
}

#[test]
fn edit_range_and_delta_cases() {
    let v = vectors();
    for c in v["edits"].as_array().unwrap() {
        let why = s(c, "why");
        let (ws, d) = ws_with(&s(c, "text"));
        let mut p = Pending::default();
        let (_, out, ok) = run_tool(
            &ws,
            &mut p,
            &call("edit_file", json!({"path": "f.txt", "old_text": c["old"], "new_text": c["new"]})),
        );
        match c.get("want") {
            Some(w) => {
                assert!(ok, "{why}: {out}");
                assert_eq!(p.files["f.txt"].after, w.as_str().unwrap(), "{why}");
            }
            None => assert_eq!(out, format!("error: {}", s(c, "error")), "{why}"),
        }
        fs::remove_dir_all(d).unwrap();
    }
    for c in v["ranges"].as_array().unwrap() {
        let why = s(c, "why");
        let (ws, d) = ws_with(&s(c, "text"));
        let mut a = json!({"path": "f.txt"});
        for (k, n) in [("start", "start_line"), ("end", "end_line")] {
            if let Some(x) = c.get(k) {
                a[n] = x.clone();
            }
        }
        let (_, out, ok) = run_tool(&ws, &mut Pending::default(), &call("read_file", a));
        match c.get("want") {
            Some(w) => assert_eq!((ok, out.as_str()), (true, w.as_str().unwrap()), "{why}"),
            None => assert_eq!(out, format!("error: {}", s(c, "error")), "{why}"),
        }
        fs::remove_dir_all(d).unwrap();
    }
    for c in v["line_delta"].as_array().unwrap() {
        let (a, r) = line_delta(&s(c, "before"), &s(c, "after"));
        assert_eq!(
            (a as u64, r as u64),
            (c["added"].as_u64().unwrap(), c["removed"].as_u64().unwrap()),
            "{}",
            s(c, "why")
        );
    }
}

#[test]
fn argument_cases() {
    let (ws, d) = ws_with("");
    for c in vectors()["args"].as_array().unwrap() {
        let t = ToolCall { id: "c".into(), name: s(c, "tool"), arguments: s(c, "arguments") };
        let (_, out, ok) = run_tool(&ws, &mut Pending::default(), &t);
        assert!(!ok, "{}", s(c, "why"));
        assert_eq!(out, format!("error: {}", s(c, "error")), "{}", s(c, "why"));
    }
    fs::remove_dir_all(d).unwrap();
}
