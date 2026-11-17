//! Public gateway list (phase I2b): the shared P2P-AI protocol vectors, refusals, and a fake directory listing.
//!
//! The vectors file is a copy of P2P-AI `decentralized/protocol/vectors/vectors.json` (provenance and how to refresh
//! it: docs/files/discovery-vectors.md). Passing here means the IDE verifies adverts byte-for-byte like the node.
//! Security cases: tampered fields, wrong id, unknown fields, http/private endpoints, relay-name tricks, every
//! `client_access` refusal from the vectors, forged probe replies, and adverts that are not for users.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signer, SigningKey};
use p2p_ide_core::discovery::*;
use serde_json::{json, Value};
use std::collections::HashMap;

fn vectors() -> Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/discovery-vectors.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn key() -> SigningKey {
    let seed = vectors()["seed_hex"].as_str().unwrap().to_string();
    let b: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&seed[i * 2..i * 2 + 2], 16).unwrap()).collect();
    SigningKey::from_bytes(&b.try_into().unwrap())
}

/// Re-signs an advert value with the vector key (for synthetic test adverts).
fn sign(mut m: Value) -> String {
    m.as_object_mut().unwrap().remove("sig");
    let body = canon_json(&m, 0).unwrap();
    let sig = key().sign(format!("P2PAD-V1\n{body}").as_bytes());
    m["sig"] = json!(B64.encode(sig.to_bytes()));
    canon_json(&m, 0).unwrap()
}

#[test]
fn shared_vectors_verify_like_the_node() {
    let v = vectors();
    let now = v["now"].as_i64().unwrap();
    let text = v["advert"].as_str().unwrap();
    let a = verify_advert(text, now, false).expect("advert vector must verify");
    assert_eq!(a.gateway_id, v["probe"]["gateway_id"].as_str().unwrap());
    assert!(a.client.is_none(), "the plain advert is node-facing only");
    assert_eq!(canon_json(&parse_strict(text).unwrap(), 0).unwrap(), text);
    let p = &v["probe"];
    assert!(verify_probe_reply(&p.to_string(), p["nonce"].as_str().unwrap(), &a, p["ts"].as_i64().unwrap()));
    // A reply for another nonce, or a stale one, never counts.
    assert!(!verify_probe_reply(&p.to_string(), "ff", &a, p["ts"].as_i64().unwrap()));
    assert!(!verify_probe_reply(&p.to_string(), p["nonce"].as_str().unwrap(), &a, p["ts"].as_i64().unwrap() + 1000));

    let c = verify_advert(v["advert_client"].as_str().unwrap(), now, false).expect("client vector must verify");
    let ca = c.client.expect("client_access present");
    assert_eq!(ca.mode, "signup");
    assert_eq!(ca.api_url, "https://gw.example.org/");
    assert_eq!(ca.features, vec!["chat", "openai_api"]);
    assert!(ca.logs_prompts);
    assert_eq!(ca.retention_days, 30);

    verify_advert(v["advert_reach"].as_str().unwrap(), now, false).expect("reach vector must verify");
    for group in ["advert_client_refused", "advert_reach_refused"] {
        for case in v[group].as_array().unwrap() {
            let got = verify_advert(case["text"].as_str().unwrap(), now, false).err();
            assert_eq!(got, Some(case["error"].as_str().unwrap()), "{group}/{}", case["name"]);
        }
    }
}

#[test]
fn tampering_and_bad_fields_are_refused() {
    let v = vectors();
    let now = v["now"].as_i64().unwrap();
    let base: Value = serde_json::from_str(v["advert"].as_str().unwrap()).unwrap();
    let with = |f: &dyn Fn(&mut Value)| {
        let mut m = base.clone();
        f(&mut m);
        m.to_string()
    };
    let cases: Vec<(&str, String, &str)> = vec![
        ("tamper", with(&|m| m["offers_public"] = json!(true)), "bad_signature"),
        ("wrong id", with(&|m| m["gateway_id"] = json!("0".repeat(40))), "bad_id"),
        ("unknown field", with(&|m| m["extra"] = json!(1)), "bad_shape"),
        ("http endpoint", with(&|m| m["endpoints"]["api"] = json!("http://gw.example.org/")), "bad_endpoint"),
        ("private probe", with(&|m| m["endpoints"]["probe"] = json!("https://10.0.0.1/v1/probe")), "bad_endpoint"),
        ("float", with(&|m| m["seq"] = json!(1.5)), "bad_json"),
        ("long life", with(&|m| m["expires_at"] = json!(1800000000 + 86401)), "bad_time"),
    ];
    for (name, t, want) in cases {
        assert_eq!(verify_advert(&t, now, false).err(), Some(want), "{name}");
    }
    // Duplicate keys are refused before parsing (signature confusion).
    let dup = v["advert"].as_str().unwrap().replacen("{\"contact\":\"\"", "{\"contact\":\"\",\"contact\":\"x\"", 1);
    assert_eq!(verify_advert(&dup, now, false).err(), Some("bad_json"));
    // A correctly signed advert is refused when expired or issued in the future.
    assert_eq!(verify_advert(v["advert"].as_str().unwrap(), 1800003600, false).err(), Some("expired"));
    assert_eq!(verify_advert(v["advert"].as_str().unwrap(), 1800000000 - 121, false).err(), Some("bad_time"));
}

fn client_advert(seq: i64, api: &str) -> String {
    let v = vectors();
    let mut m: Value = serde_json::from_str(v["advert_client"].as_str().unwrap()).unwrap();
    m["seq"] = json!(seq);
    m["client_access"]["api_url"] = json!(api);
    sign(m)
}

#[test]
fn listing_verifies_dedupes_probes_and_counts_rejections() {
    let v = vectors();
    let now = v["now"].as_i64().unwrap();
    let gid = v["probe"]["gateway_id"].as_str().unwrap().to_string();
    let node_only = v["advert"].as_str().unwrap().to_string();
    let mut forged: Value = serde_json::from_str(&client_advert(9, "https://evil.example.org/")).unwrap();
    forged["seq"] = json!(10); // changed after signing
    let dir_a =
        json!({"adverts": [client_advert(3, "https://old.example.org/"), node_only, forged.to_string()]}).to_string();
    let dir_b = json!({"adverts": [client_advert(5, "https://gw.example.org/"), 42]}).to_string();
    let sk = key();
    let gid2 = gid.clone();
    let mut pages: HashMap<String, String> = HashMap::new();
    pages.insert("https://a.example.org/v1/adverts".into(), dir_a);
    pages.insert("https://b.example.org/v1/adverts".into(), dir_b);
    let fetch = move |url: &str, _max: u64| -> Result<String, String> {
        if let Some(n) = url.strip_prefix("https://gw.example.org/v1/probe?nonce=") {
            let ts = 1800000100i64;
            let sig = sk.sign(format!("P2PPROBE-V1\n{n}\n{ts}\n{gid2}").as_bytes());
            return Ok(json!({"gateway_id": gid2, "nonce": n, "ts": ts, "sig": B64.encode(sig.to_bytes())}).to_string());
        }
        pages.get(url).cloned().ok_or_else(|| "down".to_string())
    };
    let dirs =
        vec!["https://a.example.org".to_string(), "https://b.example.org/".into(), "https://c.example.org".into()];
    let out = list_public(&dirs, now, &fetch);
    assert_eq!(out.gateways.len(), 1);
    let g = &out.gateways[0];
    assert_eq!(g.gateway_id, gid);
    assert_eq!(g.access.api_url, "https://gw.example.org/", "the newest seq wins");
    assert_eq!(g.directories, 2);
    assert!(g.latency_ms.is_some(), "the signed probe verified");
    assert_eq!(out.rejected.get("not_for_users"), Some(&1));
    assert_eq!(out.rejected.get("bad_signature"), Some(&1));
    assert_eq!(out.rejected.get("bad_shape"), Some(&1));
    assert_eq!(out.rejected.get("directory_unreachable"), Some(&1));
}

#[test]
fn forged_probe_gives_no_latency() {
    let v = vectors();
    let now = v["now"].as_i64().unwrap();
    let list = json!({"adverts": [client_advert(3, "https://gw.example.org/")]}).to_string();
    let other = SigningKey::from_bytes(&[9u8; 32]);
    let gid = v["probe"]["gateway_id"].as_str().unwrap().to_string();
    let fetch = move |url: &str, _max: u64| -> Result<String, String> {
        if let Some(n) = url.strip_prefix("https://gw.example.org/v1/probe?nonce=") {
            let sig = other.sign(format!("P2PPROBE-V1\n{n}\n1800000100\n{gid}").as_bytes());
            return Ok(
                json!({"gateway_id": gid, "nonce": n, "ts": 1800000100, "sig": B64.encode(sig.to_bytes())}).to_string()
            );
        }
        Ok(list.clone())
    };
    let out = list_public(&["https://a.example.org".to_string()], now, &fetch);
    assert_eq!(out.gateways.len(), 1);
    assert_eq!(out.gateways[0].latency_ms, None);
}

#[test]
fn bad_directories_are_skipped_without_fetching() {
    let fetch = |_u: &str, _m: u64| -> Result<String, String> { panic!("must not fetch") };
    let out = list_public(&["https://127.0.0.1".to_string(), "http://dir.example.org".into()], 0, &fetch);
    assert!(out.gateways.is_empty());
    assert_eq!(out.rejected.get("bad_directory"), Some(&2));
}
