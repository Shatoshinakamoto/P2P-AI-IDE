//! gateway_vectors -- runs the shared gateway-client test cases against the Rust client (tests only).
//!
//! Purpose: the VS Code extension (side track X, ADR-029) has its own TypeScript gateway client, because VS Code
//! extensions run in VS Code's Node, not in this Rust core. Both clients read the same cases from
//! `core/tests/vectors/gateway_client.json`, so a change to one that the other doesn't match fails a test.
//! Inputs: that file only (part of the repository). No network.
//! Security: the cases cover the checks that keep untrusted gateway answers as plain text (CWE-79, CWE-20), keys
//! out of headers they could break (CWE-113) and keys off plain http to other hosts (CWE-319).

use crate::client::{check_model, check_status, parse_models, parse_sse_line, NetError, SseItem};
use crate::gateways::{check_key, check_name, check_url};
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../tests/vectors/gateway_client.json")).expect("vectors are JSON")
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_else(|| panic!("{k} in {v}")).to_string()
}

#[test]
fn sse_cases() {
    for c in vectors()["sse"].as_array().unwrap() {
        let why = s(c, "why");
        let want = &c["want"];
        let got = parse_sse_line(&s(c, "line"));
        match want["kind"].as_str().unwrap() {
            "delta" => assert_eq!(got, Ok(SseItem::Delta(s(want, "text"))), "{why}"),
            "finish" => assert_eq!(got, Ok(SseItem::Finish(s(want, "text"))), "{why}"),
            "done" => assert_eq!(got, Ok(SseItem::Done), "{why}"),
            "skip" => assert_eq!(got, Ok(SseItem::Skip), "{why}"),
            "error" => assert_eq!(got, Ok(SseItem::Error), "{why}"),
            "bad_answer" => assert_eq!(got, Err(NetError::BadAnswer), "{why}"),
            "tools" => {
                let Ok(SseItem::Tools(p)) = got else { panic!("{why}: {got:?}") };
                let wp = want["pieces"].as_array().unwrap();
                assert_eq!(p.len(), wp.len(), "{why}");
                for (a, b) in p.iter().zip(wp) {
                    assert_eq!(a.index as u64, b["index"].as_u64().unwrap(), "{why}");
                    assert_eq!(a.id, s(b, "id"), "{why}");
                    assert_eq!(a.name, s(b, "name"), "{why}");
                    assert_eq!(a.args, s(b, "args"), "{why}");
                }
            }
            k => panic!("unknown kind {k}"),
        }
    }
}

#[test]
fn model_cases() {
    let v = vectors();
    for c in v["models"].as_array().unwrap() {
        let got = parse_models(s(c, "body").as_bytes());
        match &c["want"] {
            Value::String(x) if x == "bad_answer" => assert_eq!(got, Err(NetError::BadAnswer), "{}", s(c, "why")),
            w => {
                let want: Vec<String> = serde_json::from_value(w.clone()).unwrap();
                assert_eq!(got, Ok(want), "{}", s(c, "why"));
            }
        }
    }
    for c in v["model_ids"].as_array().unwrap() {
        assert_eq!(check_model(&s(c, "id")).is_ok(), c["ok"].as_bool().unwrap(), "{}", s(c, "why"));
    }
}

#[test]
fn status_cases() {
    for c in vectors()["status"].as_array().unwrap() {
        let code = c["code"].as_u64().unwrap() as u16;
        let got = match check_status(code) {
            Ok(()) => "ok",
            Err(NetError::Auth) => "auth",
            Err(NetError::NotOffered) => "not_offered",
            Err(NetError::RateLimited) => "rate_limited",
            Err(NetError::Status(_)) => "status",
            Err(e) => panic!("{e:?}"),
        };
        assert_eq!(got, s(c, "want"), "{}", s(c, "why"));
    }
}

#[test]
fn url_name_and_key_cases() {
    let v = vectors();
    for c in v["urls"].as_array().unwrap() {
        assert_eq!(check_url(&s(c, "url")).ok(), c["want"].as_str().map(str::to_string), "{}", s(c, "why"));
    }
    for c in v["names"].as_array().unwrap() {
        assert_eq!(check_name(&s(c, "name")).ok(), c["want"].as_str().map(str::to_string), "{}", s(c, "why"));
    }
    for c in v["keys"].as_array().unwrap() {
        assert_eq!(check_key(&s(c, "key")).ok(), c["want"].as_str().map(str::to_string), "{}", s(c, "why"));
    }
}
