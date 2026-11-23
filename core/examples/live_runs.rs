//! Live check of phase I5b against a real P2P-AI gateway, node and sandbox (not a unit test).
//!
//! Purpose: `runner.rs` unit tests cover the request checks and the parsing of answers. This program drives the real
//! Go gateway the way the Run and test view and the chat's approved runs do, with `runner::gateway_run`: send the
//! folder, run unittest (one failing test expected), fix the file locally and run again (the changed file must reach
//! the node copy), run a command, check that the sandbox has no network, that a time limit stops a run, and that the
//! request checks refuse a folder outside the root before anything is sent. Started by
//! `scripts/live_gateway_e2e.py --example live_runs`, which builds and runs the stack on 127.0.0.1.
//!
//! Usage: `cargo run --example live_runs -- <gateway-url> <key-file> <work-folder> <model>`
//! Outputs: `PASS`/`FAIL` lines and a last line `{"passed": n, "failed": n}`; exit 0 only when nothing failed.
//!
//! Security notes:
//!   * SECURITY: the key is read from an owner-only file, never argv, and never printed (CWE-214, CWE-532).
//!   * SECURITY: checks no network inside the sandbox (NIST SC-7), the time limit (CWE-400), and that `.env` is
//!     never sent (the driver searches every stored file for its value, CWE-200).
//!
//! Dependencies: the core crate and serde_json only.

use p2p_ide_core::client::Client;
use p2p_ide_core::gateways::{Gateway, Kind};
use p2p_ide_core::projects::project_view;
use p2p_ide_core::runner::{check, gateway_run, RunInput, RunResult};
use p2p_ide_core::workspace::Workspace;
use serde_json::json;
use std::fs;
use std::path::Path;

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
            // Details are exit codes, counts and short output tails, never keys.
            println!("FAIL {what}: {}", detail.chars().take(600).collect::<String>());
        }
        ok
    }
}

/// The test project: a bug in `add`, a unittest that finds it, and a secret that must stay home.
fn make_folder(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(root.join("src/calc.py"), "def add(a, b):\n    return a - b\n").unwrap();
    fs::write(
        root.join("tests/test_calc.py"),
        "import sys, unittest\nsys.path.insert(0, 'src')\nfrom calc import add\n\n\nclass T(unittest.TestCase):\n    \
         def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n\n    def test_zero(self):\n        \
         self.assertEqual(add(0, 0), 0)\n",
    )
    .unwrap();
    fs::write(root.join("tests/__init__.py"), "# tests package\n").unwrap();
    fs::write(root.join(".env"), "API_KEY=ide-live-do-not-send\n").unwrap();
}

fn short(r: &Result<RunResult, p2p_ide_core::client::NetError>) -> String {
    match r {
        Ok(r) => format!(
            "exit={:?} timed_out={} error={:?} tests={:?} stdout_tail={:?} stderr_tail={:?}",
            r.exit_code,
            r.timed_out,
            r.error,
            r.tests,
            r.stdout.chars().rev().take(200).collect::<String>().chars().rev().collect::<String>(),
            r.stderr.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>()
        ),
        Err(e) => format!("error: {}", e.message()),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 5 {
        eprintln!("usage: live_runs <gateway-url> <key-file> <work-folder> <model>");
        std::process::exit(2);
    }
    let (url, key_file, work) = (&a[1], &a[2], Path::new(&a[3]));
    let key = fs::read_to_string(key_file).expect("key file").trim().to_string();
    let gw = Gateway {
        id: "0123456789abcdef".into(),
        name: "live".into(),
        url: url.clone(),
        kind: Kind::Manual,
        ca_pem: None,
        account: None,
    };
    let c = Client::new(&gw, Some(key)).expect("client");
    let mut k = Checks { passed: 0, failed: 0 };
    let root = work.join("ide-run-project");
    make_folder(&root);
    let ws = Workspace::open(&root).expect("workspace");
    let done = |k: &Checks| {
        println!("{}", json!({"passed": k.passed, "failed": k.failed}));
        std::process::exit(if k.failed == 0 { 0 } else { 1 });
    };
    let p = c.api("POST", "/v1/projects", Some(&json!({"name": "ide-live-runs"}))).ok().and_then(|v| project_view(&v));
    if !k.check(p.is_some(), "create a project on the gateway", "no project") {
        done(&k);
    }
    let pid = p.unwrap().id;
    let input = |kind: &str, fw: &str, argv: &[&str], timeout: Option<u32>| RunInput {
        kind: kind.into(),
        framework: fw.into(),
        argv: argv.iter().map(|s| s.to_string()).collect(),
        cwd: String::new(),
        timeout_s: timeout,
    };
    let mut stages: Vec<&'static str> = vec![];

    // 1. Tests with the bug: one of two tests fails.
    let req = check(&ws, &input("tests", "unittest", &[], Some(120))).unwrap();
    let r = gateway_run(&c, &ws, &pid, &req, &mut |s| stages.push(s));
    k.check(
        stages == ["sending", "copying", "running"],
        "a run reports its stages: sending, copying, running",
        &format!("{stages:?}"),
    );
    let ok = r.as_ref().is_ok_and(|r| {
        r.exit_code.unwrap_or(0) != 0 && r.tests.as_ref().is_some_and(|t| t.failed == 1 && t.passed == 1)
    });
    k.check(ok, "unittest on the gateway sandbox finds the one failing test", &short(&r));
    if let Ok(r) = &r {
        k.check(
            r.tests.as_ref().is_some_and(|t| t.failures.iter().any(|f| f.contains("test_add"))),
            "the failing test is named",
            &format!("{:?}", r.tests),
        );
        k.check(!r.place.is_empty(), "the result says which node ran it", "no place");
    }

    // 2. Fix the file on disk and run again: the changed file must reach the node copy.
    fs::write(root.join("src/calc.py"), "def add(a, b):\n    return a + b\n").unwrap();
    let r = gateway_run(&c, &ws, &pid, &req, &mut |_| {});
    k.check(
        r.as_ref().is_ok_and(|r| r.ok() && r.tests.as_ref().is_some_and(|t| t.failed == 0 && t.passed == 2)),
        "after a local fix the next run sees the change and passes",
        &short(&r),
    );

    // 3. A command, as words (shell signs are plain arguments).
    let req = check(
        &ws,
        &input("command", "", &["python3", "-c", "import sys; print('args', sys.argv[1:])", ";", "$HOME"], None),
    )
    .unwrap();
    let r = gateway_run(&c, &ws, &pid, &req, &mut |_| {});
    k.check(
        r.as_ref().is_ok_and(|r| r.ok() && r.stdout.contains("args [';', '$HOME']")),
        "a command runs as an argument list; ; and $HOME reach the program as text",
        &short(&r),
    );

    // 4. No network in the sandbox.
    let net = "import socket\ntry:\n    socket.create_connection(('1.1.1.1', 53), 3)\n    print('NETWORK-OPEN')\nexcept OSError as e:\n    print('no network', type(e).__name__)\n";
    let req = check(&ws, &input("command", "", &["python3", "-c", net], None)).unwrap();
    let r = gateway_run(&c, &ws, &pid, &req, &mut |_| {});
    k.check(
        r.as_ref().is_ok_and(|r| r.stdout.contains("no network") && !r.stdout.contains("NETWORK-OPEN")),
        "SECURITY: the sandbox has no network",
        &short(&r),
    );

    // 5. The time limit stops a run.
    let req = check(&ws, &input("command", "", &["python3", "-c", "import time; time.sleep(60)"], Some(10))).unwrap();
    let r = gateway_run(&c, &ws, &pid, &req, &mut |_| {});
    k.check(r.as_ref().is_ok_and(|r| r.timed_out && !r.ok()), "a run past its time limit is stopped", &short(&r));

    // 6. Requests are checked before anything is sent.
    let mut bad = input("command", "", &["ls"], None);
    bad.cwd = "../outside".into();
    k.check(check(&ws, &bad).is_err(), "SECURITY: a folder outside the root is refused before sending", "accepted");
    done(&k);
}
