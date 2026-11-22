//! projects_vectors -- runs the shared mission test cases against the IDE (tests only).
//!
//! Purpose: the VS Code / VSCodium extension (side track X3, ADR-029) has a TypeScript port of the mission logic
//! (`vscode-extension/src/missions/projects.ts`). Both read `core/tests/vectors/missions.json` -- gateway path
//! grammar, ids, URL encoding, upload names, error-code messages, push and pull plans, view shapes and acceptance
//! checks -- so a change to one that the other doesn't match fails a test.
//! Inputs: that file only. No network, no disk.
//! Security: the cases cover secrets kept home (CWE-200), path and id grammar (CWE-22, CWE-74), header-safe upload
//! names (CWE-93), fixed error text (CWE-209), sanitised gateway text (CWE-150) and argv-only checks (CWE-78).

use super::*;
use crate::gwapi::{answer_error, encode_path, upload_name};
use serde_json::json;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../tests/vectors/missions.json")).expect("vectors are JSON")
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_else(|| panic!("{k} in {v}")).to_string()
}

fn pairs(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string()))
        .collect()
}

fn remote(v: &Value) -> Vec<RemoteFile> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| RemoteFile { path: s(r, "path"), bytes: r["bytes"].as_u64().unwrap(), sha256: s(r, "sha256") })
        .collect()
}

fn skipped(v: &[Skipped]) -> Value {
    json!(v.iter().map(|x| json!({"rel": x.rel, "reason": x.reason})).collect::<Vec<_>>())
}

fn kind(e: &NetError) -> &'static str {
    match e {
        NetError::Gateway(_) => "gateway",
        NetError::Auth => "auth",
        NetError::Status(_) => "status",
        NetError::NotOffered => "not_offered",
        NetError::RateLimited => "rate_limited",
        _ => "other",
    }
}

#[test]
fn grammar_ids_encoding_and_names() {
    let v = vectors();
    for c in v["gateway_paths"].as_array().unwrap() {
        assert_eq!(gateway_path_ok(&s(c, "path")), c["ok"].as_bool().unwrap(), "{}", s(c, "why"));
    }
    for c in v["ids"].as_array().unwrap() {
        assert_eq!(id_ok(&s(c, "id")), c["ok"].as_bool().unwrap(), "{}", s(c, "why"));
    }
    for c in v["encode_path"].as_array().unwrap() {
        assert_eq!(encode_path(&s(c, "rel")), s(c, "want"), "{}", s(c, "why"));
    }
    for c in v["upload_names"].as_array().unwrap() {
        assert_eq!(upload_name(&s(c, "rel")), c["want"].as_str(), "{}", s(c, "why"));
    }
}

#[test]
fn error_codes() {
    for c in vectors()["errors"].as_array().unwrap() {
        let e = answer_error(c["status"].as_u64().unwrap() as u16, s(c, "body").as_bytes());
        assert_eq!((kind(&e), e.message()), (c["kind"].as_str().unwrap(), s(c, "message")), "{}", s(c, "why"));
    }
}

#[test]
fn push_and_pull_plans() {
    let v = vectors();
    for c in v["eligible"].as_array().unwrap() {
        let files: Vec<(String, u64)> = c["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f[0].as_str().unwrap().to_string(), f[1].as_u64().unwrap()))
            .collect();
        let (ok, sk) = eligible(&files);
        assert_eq!(json!(ok), c["ok"], "{}", s(c, "why"));
        assert_eq!(skipped(&sk), c["skipped"], "{}", s(c, "why"));
    }
    for c in v["plan_push"].as_array().unwrap() {
        let sl: Vec<Skipped> = c["skipped_local"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| Skipped { rel: s(x, "rel"), reason: "kept" })
            .collect();
        let p = plan_push(&pairs(&c["local"]), &remote(&c["remote"]), &sl);
        assert_eq!(json!(p.upload), c["upload"], "{}", s(c, "why"));
        assert_eq!(json!(p.delete), c["delete"], "{}", s(c, "why"));
        assert_eq!(p.unchanged as u64, c["unchanged"].as_u64().unwrap(), "{}", s(c, "why"));
    }
    for c in v["plan_pull"].as_array().unwrap() {
        let p = plan_pull(&pairs(&c["local"]), &remote(&c["remote"]));
        let fetch: Vec<Value> = p
            .fetch
            .iter()
            .map(|f| json!({"path": f.path, "sha256": f.sha256, "local_version": f.local_version}))
            .collect();
        assert_eq!(json!(fetch), c["fetch"], "{}", s(c, "why"));
        assert_eq!(
            (p.same as u64, p.only_local as u64),
            (c["same"].as_u64().unwrap(), c["only_local"].as_u64().unwrap())
        );
        assert_eq!(skipped(&p.skipped), c["skipped"], "{}", s(c, "why"));
    }
    for c in v["remote_files"].as_array().unwrap() {
        let got: Vec<Value> = parse_remote_files(&c["answer"])
            .iter()
            .map(|r| json!({"path": r.path, "bytes": r.bytes, "sha256": r.sha256}))
            .collect();
        assert_eq!(json!(got), c["want"], "{}", s(c, "why"));
    }
}

#[test]
fn view_shapes() {
    let v = &vectors()["views"];
    let check = |name: &str, f: &dyn Fn(&Value) -> Value| {
        for c in v[name].as_array().unwrap() {
            assert_eq!(f(&c["in"]), c["want"], "{name}: {}", s(c, "why"));
        }
    };
    check("mission", &|i| json!(mission_view(i)));
    check("approval", &|i| json!(approval_view(i)));
    check("proposal", &|i| json!(proposal_view(i)));
    check("sync", &|i| json!(sync_view(i)));
    check("project", &|i| json!(project_view(i)));
    for c in v["switches"].as_array().unwrap() {
        assert_eq!(json!(switches(&c["exp"], &c["flags"])), c["want"], "{}", s(c, "why"));
    }
}

#[test]
fn acceptance_checks() {
    for c in vectors()["acceptance"].as_array().unwrap() {
        let checks: Vec<CheckInput> = serde_json::from_value(c["checks"].clone()).unwrap();
        match (acceptance(&checks), c.get("want")) {
            (Ok(got), Some(w)) => assert_eq!(&got, w, "{}", s(c, "why")),
            (Err(e), None) => assert_eq!(e, s(c, "error"), "{}", s(c, "why")),
            (got, _) => panic!("{}: {got:?}", s(c, "why")),
        }
    }
}
