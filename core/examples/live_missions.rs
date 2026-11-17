//! Live check of phase I4 against a real P2P-AI gateway, node and sandbox (not a unit test).
//!
//! Purpose: `core/tests/projects.rs` covers push and pull against a small fake. This program drives the real Go
//! gateway the way the IDE's Missions view does, with the same core functions and the same request shapes as
//! `app/src-tauri/src/missions.rs`: create a project, send the folder, switch Autopilot on, start an Autopilot
//! mission with a unittest check, decide its approvals (a stale digest first, which must be refused), collect the
//! node copy's changes, pull them into the pending review and accept one. It is started by
//! `scripts/live_gateway_e2e.py`, which builds and runs the stack on 127.0.0.1.
//!
//! Usage: `cargo run --example live_missions -- <gateway-url> <key-file> <work-folder> <model>`
//! Inputs: the gateway URL (loopback), a file holding a throw-away API key, an empty work folder, a model id.
//! Outputs: `PASS`/`FAIL` lines and a last line `{"passed": n, "failed": n}`; exit 0 only when nothing failed.
//!
//! Security notes:
//!   * SECURITY: the key is read from an owner-only file, never argv, and never printed (CWE-214, CWE-532).
//!   * SECURITY: checks that `.env` is never sent and that nothing is written locally before Accept (CWE-200,
//!     OWASP LLM06), and that a decision with a digest other than the one shown is refused (CWE-345).
//!
//! Dependencies: the core crate and serde_json only.

use p2p_ide_core::agent::Pending;
use p2p_ide_core::client::Client;
use p2p_ide_core::gateways::{Gateway, Kind};
use p2p_ide_core::projects::{
    acceptance, approval_view, mission_view, project_view, pull, push, switches, sync_view, CheckInput,
};
use p2p_ide_core::workspace::Workspace;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Counts and prints check results.
struct Checks {
    passed: u32,
    failed: u32,
}

impl Checks {
    fn check(&mut self, ok: bool, what: &str, detail: &str) -> bool {
        if ok {
            self.passed += 1;
            println!("PASS {what}");
        } else {
            self.failed += 1;
            // Details are gateway states and counts, never keys or file contents.
            println!("FAIL {what}: {detail}");
        }
        ok
    }
}

/// The test project: a bug in `add`, a unittest that finds it, a README and a secret.
fn make_folder(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(root.join("src/calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
    fs::write(
        root.join("tests/test_calc.py"),
        "import sys, unittest\nsys.path.insert(0, 'src')\nfrom calc import add\n\n\nclass T(unittest.TestCase):\n    \
         def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n\n\nif __name__ == '__main__':\n    unittest.main()\n",
    )
    .unwrap();
    // unittest discovery only enters a folder that is a package.
    fs::write(root.join("tests/__init__.py"), "# tests package\n").unwrap();
    fs::write(root.join("README.md"), "# live check\n").unwrap();
    // The marker value is checked by the driver against every file the gateway stored.
    fs::write(root.join(".env"), "API_KEY=ide-live-do-not-send\n").unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 5 {
        eprintln!("usage: live_missions <gateway-url> <key-file> <work-folder> <model>");
        std::process::exit(2);
    }
    let (url, key_file, work, model) = (&a[1], &a[2], Path::new(&a[3]), a[4].clone());
    let key = fs::read_to_string(key_file).expect("key file").trim().to_string();
    let gw = Gateway {
        id: "0123456789abcdef".into(),
        name: "live".into(),
        url: url.clone(),
        kind: Kind::Manual,
        ca_pem: None,
    };
    let c = Client::new(&gw, Some(key)).expect("client");
    let mut k = Checks { passed: 0, failed: 0 };
    let root = work.join("ide-project");
    make_folder(&root);
    let ws = Workspace::open(&root).expect("workspace");
    let done = |k: &Checks| {
        println!("{}", json!({"passed": k.passed, "failed": k.failed}));
        std::process::exit(if k.failed == 0 { 0 } else { 1 });
    };

    // 1. Create a project (project_create).
    let p = c.api("POST", "/v1/projects", Some(&json!({"name": "ide-live"}))).ok().and_then(|v| project_view(&v));
    if !k.check(p.is_some(), "create a project on the gateway", "no project") {
        done(&k);
    }
    let pid = p.unwrap().id;

    // 2. Send the folder (project_push).
    let r = push(&c, &ws, &pid);
    let ok = r.as_ref().is_ok_and(|r| r.uploaded == 4 && r.skipped.iter().any(|s| s.rel == ".env"));
    k.check(ok, "send the folder: 4 files, .env kept on this computer", &format!("{r:?}"));
    let again = push(&c, &ws, &pid);
    k.check(
        again.as_ref().is_ok_and(|r| r.uploaded == 0 && r.unchanged == 4),
        "a second send changes nothing",
        &format!("{again:?}"),
    );

    // 3. Wait for the node copy.
    let t = Instant::now();
    let mut sync = String::new();
    while t.elapsed() < Duration::from_secs(60) {
        sync = c
            .api("GET", &format!("/v1/projects/{pid}/workspace/sync"), None)
            .map(|v| sync_view(&v).state)
            .unwrap_or_default();
        if sync == "ready" {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    k.check(sync == "ready", "a node holds a copy of the project", &sync);

    // 4. Switch Autopilot on (switches_set).
    let f = c.api(
        "PUT",
        &format!("/v1/projects/{pid}/experimental"),
        Some(&json!({"autopilot": true, "auto_improve": false})),
    );
    let e = c.api("GET", "/v1/experimental", None);
    let sw = match (&f, &e) {
        (Ok(f), Ok(e)) => Some(switches(e, f)),
        _ => None,
    };
    k.check(
        sw.as_ref().is_some_and(|s| s.autopilot && s.autopilot_available),
        "Autopilot switched on and available",
        &format!("{sw:?} {:?}", f.as_ref().err().map(|e| e.message())),
    );

    // 5. Start an Autopilot mission with a unittest check (mission_start).
    let checks = acceptance(&[CheckInput {
        id: "unit".into(),
        kind: "tests".into(),
        framework: "unittest".into(),
        argv: vec![],
    }]);
    let body = json!({"project_id": pid, "kind": "autopilot", "goal": "make the tests in tests/ pass", "model": model,
                      "acceptance": checks.unwrap()});
    let m = c.api("POST", "/v1/missions", Some(&body));
    let mv = m.as_ref().ok().and_then(mission_view);
    if !k.check(mv.is_some(), "start an Autopilot mission", &format!("{:?}", m.as_ref().err().map(|e| e.message()))) {
        done(&k);
    }
    let mid = mv.unwrap().id;
    // The key's request limiter may answer first after the polling above; wait it out as the UI would.
    let mut dup = c.api("POST", "/v1/missions", Some(&body));
    for _ in 0..10 {
        if dup.as_ref().err().map(|e| e.message()).as_deref()
            != Some("the gateway is limiting requests; try again shortly")
        {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
        dup = c.api("POST", "/v1/missions", Some(&body));
    }
    k.check(
        dup.as_ref().err().map(|e| e.message()).as_deref()
            == Some("this project already has a mission of this kind; stop it first"),
        "a second mission of the same kind gets the fixed message",
        &format!("{:?}", dup.as_ref().err().map(|e| e.message())),
    );

    // 6. Follow the mission and decide its approvals (mission_get, approvals_list, approval_decide).
    let t = Instant::now();
    let (mut approved, mut stale_refused, mut state) = (0, false, String::new());
    let mut tools = vec![];
    while t.elapsed() < Duration::from_secs(600) {
        let Some(m) = c.api("GET", &format!("/v1/missions/{mid}"), None).ok().and_then(|v| mission_view(&v)) else {
            break;
        };
        state = m.state.clone();
        if matches!(state.as_str(), "done" | "completed" | "failed" | "stopped" | "stalled") {
            break;
        }
        if !m.run_id.is_empty() {
            let list = c.api("GET", &format!("/v1/agents/runs/{}/approvals", m.run_id), None).ok();
            let views: Vec<_> = list
                .as_ref()
                .and_then(|v| v.get("data").and_then(Value::as_array).cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(approval_view)
                .collect();
            for ap in views {
                if !stale_refused {
                    // SECURITY: a decision bound to other arguments than the ones shown must be refused.
                    let bad = "0".repeat(64);
                    let r = c.api(
                        "POST",
                        &format!("/v1/agents/runs/{}/approvals", m.run_id),
                        Some(&json!({"approval_id": ap.id, "decision": "approve", "arguments_sha256": bad})),
                    );
                    stale_refused =
                        r.as_ref().err().map(|e| e.message()).as_deref() == Some("the approval changed; open it again");
                    k.check(
                        stale_refused,
                        "approving with another digest is refused",
                        &format!("{:?}", r.err().map(|e| e.message())),
                    );
                }
                let r = c.api(
                    "POST",
                    &format!("/v1/agents/runs/{}/approvals", m.run_id),
                    Some(
                        &json!({"approval_id": ap.id, "decision": "approve", "arguments_sha256": ap.arguments_sha256}),
                    ),
                );
                if r.is_ok() {
                    approved += 1;
                    tools.push(ap.tool.clone());
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    k.check(
        approved > 0,
        "the mission's write calls were approved with the shown digest",
        &format!("{approved} {tools:?}"),
    );
    let final_m = c.api("GET", &format!("/v1/missions/{mid}"), None).ok().and_then(|v| mission_view(&v));
    let passed = final_m.as_ref().is_some_and(|m| m.checks_total == 1 && m.checks_passed == 1);
    k.check(
        matches!(state.as_str(), "done" | "completed") && passed,
        "the mission finished with its check passing in the sandbox",
        &format!("state {state}, {:?}", final_m.map(|m| (m.checks_passed, m.checks_total, m.reason))),
    );

    // 7. Collect the node copy's changes (project_collect), then wait for the collection.
    let r = c.api("POST", &format!("/v1/projects/{pid}/workspace/sync/delta"), Some(&json!({})));
    k.check(r.is_ok(), "ask the gateway to collect the node's changes", &format!("{:?}", r.err().map(|e| e.message())));
    let t = Instant::now();
    let mut sv = sync_view(&Value::Null);
    while t.elapsed() < Duration::from_secs(60) {
        sv = c
            .api("GET", &format!("/v1/projects/{pid}/workspace/sync"), None)
            .map(|v| sync_view(&v))
            .unwrap_or_default();
        if matches!(sv.delta_state.as_str(), "committed" | "unchanged" | "conflict" | "failed") {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    k.check(
        sv.delta_state == "committed" && sv.delta_modified >= 1,
        "the fix was collected into a new project version",
        &format!("{sv:?}"),
    );

    // 8. Pull into the pending review (project_pull); nothing is written before Accept.
    let mut pending = Pending::default();
    let r = pull(&c, &ws, &mut pending, &pid);
    k.check(
        r.as_ref().is_ok_and(|r| r.proposed >= 1) && pending.files.contains_key("src/calc.py"),
        "the fixed file waits in Changes to review",
        &format!("{r:?}"),
    );
    let local = fs::read_to_string(root.join("src/calc.py")).unwrap_or_default();
    k.check(local.contains("a - b"), "nothing was written locally before Accept", "the file changed");
    k.check(
        fs::read_to_string(root.join(".env")).unwrap_or_default() == "API_KEY=ide-live-do-not-send\n",
        ".env on this computer is untouched",
        "changed",
    );
    let acc = pending.accept(&ws, "src/calc.py");
    let after = fs::read_to_string(root.join("src/calc.py")).unwrap_or_default();
    k.check(acc.is_ok() && after.contains("a + b"), "Accept writes the mission's fix", &after);
    done(&k);
}
