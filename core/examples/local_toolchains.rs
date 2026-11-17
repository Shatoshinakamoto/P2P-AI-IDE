//! Live check of phase I5c: real toolchains inside the local OS sandbox (not a unit test).
//!
//! Purpose: `localbox.rs` tests the confinement with python3. This program makes a tiny project per framework
//! (cargo, go, npm with node:test, make, unittest), each with one passing and one failing test, and runs it with
//! `localbox::run` the way the Run and test view does. A framework whose program is not installed is reported
//! as skipped, not passed. It also checks that a second cargo run reuses the per-folder cache.
//!
//! Usage: `cargo run --example local_toolchains`
//! Outputs: `PASS`/`FAIL`/`SKIP` lines and a last line `{"passed": n, "failed": n, "skipped": n}`; exit 0 only
//! when nothing failed.
//!
//! Security notes: everything runs inside the sandbox under test, on throw-away folders in the temp folder; no
//! network is used (the projects have no dependencies).
//!
//! Dependencies: the core crate and serde_json only.

use p2p_ide_core::localbox;
use p2p_ide_core::runner::{check, RunInput};
use p2p_ide_core::workspace::Workspace;
use std::fs;
use std::path::{Path, PathBuf};

/// A test project: framework, the program it needs, and its files.
type Project = (&'static str, &'static str, Vec<(&'static str, &'static str)>);

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("p2pide-tc-{tag}-{}", p2p_ide_core::store::new_id()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (rel, text) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
}

/// True when `name` is on PATH. On Windows the sandbox starts `.exe` files only, so it looks for `<name>.exe`, for
/// `node.exe` when the name is `npm`, and for `python.exe` when `python3.exe` is missing.
fn on_path(name: &str) -> bool {
    let names: Vec<String> = if cfg!(windows) {
        match name {
            "npm" => vec!["node.exe".into()],
            "python3" => vec!["python3.exe".into(), "python.exe".into()],
            n => vec![format!("{n}.exe")],
        }
    } else {
        vec![name.into()]
    };
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|d| {
            d.is_absolute()
                && !d.to_string_lossy().to_lowercase().contains("windowsapps")
                && names.iter().any(|n| d.join(n).is_file())
        })
    })
}

fn main() {
    let (mut passed, mut failed, mut skipped) = (0, 0, 0);
    if !localbox::available().available {
        println!("SKIP the local sandbox is not available here: {}", localbox::available().reason);
        println!("{}", serde_json::json!({"passed": 0, "failed": 0, "skipped": 1}));
        return;
    }
    let data = tmp("data");
    let projects: Vec<Project> = vec![
        (
            "cargo",
            "cargo",
            vec![
                ("Cargo.toml", "[package]\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
                (
                    "src/lib.rs",
                    "pub fn add(a: i32, b: i32) -> i32 { a - b }\n#[cfg(test)]\nmod t {\n    #[test] fn add() { assert_eq!(super::add(2, 3), 5); }\n    #[test] fn zero() { assert_eq!(super::add(0, 0), 0); }\n}\n",
                ),
            ],
        ),
        (
            "go",
            "go",
            vec![
                ("go.mod", "module example.com/calc\n\ngo 1.21\n"),
                ("calc.go", "package calc\n\nfunc Add(a, b int) int { return a - b }\n"),
                (
                    "calc_test.go",
                    "package calc\n\nimport \"testing\"\n\nfunc TestAdd(t *testing.T) {\n\tif Add(2, 3) != 5 {\n\t\tt.Fatal(\"bad\")\n\t}\n}\n",
                ),
            ],
        ),
        (
            "npm",
            "npm",
            vec![
                ("package.json", "{\"name\":\"calc\",\"version\":\"1.0.0\",\"scripts\":{\"test\":\"node --test\"}}\n"),
                (
                    "test/calc.test.js",
                    "const test = require('node:test');\nconst assert = require('node:assert');\nconst add = (a, b) => a - b;\ntest('adds', () => assert.strictEqual(add(2, 3), 5));\ntest('zero', () => assert.strictEqual(add(0, 0), 0));\n",
                ),
            ],
        ),
        ("make", "make", vec![("Makefile", "test:\n\t@echo make-ran-in-sandbox\n\t@false\n")]),
        (
            "unittest",
            "python3",
            vec![
                ("src/calc.py", "def add(a, b):\n    return a - b\n"),
                ("tests/__init__.py", ""),
                (
                    "tests/test_calc.py",
                    "import sys, unittest\nsys.path.insert(0, 'src')\nfrom calc import add\n\n\nclass T(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n\n    def test_zero(self):\n        self.assertEqual(add(0, 0), 0)\n",
                ),
            ],
        ),
    ];
    for (fw, program, files) in projects {
        if !on_path(program) {
            println!("SKIP {fw}: {program} is not installed");
            skipped += 1;
            continue;
        }
        let root = tmp(fw);
        write(&root, &files);
        let ws = Workspace::open(&root).unwrap();
        let req = check(
            &ws,
            &RunInput { kind: "tests".into(), framework: fw.into(), timeout_s: Some(300), ..Default::default() },
        )
        .unwrap();
        let mut runs = vec![];
        for _ in 0..if fw == "cargo" { 2 } else { 1 } {
            runs.push(localbox::run(&ws, &req, &data, &mut |_| {}));
        }
        let r = runs.pop().unwrap();
        let first_ms = runs.first().and_then(|f| f.as_ref().ok()).map(|f| f.duration_ms);
        let ok = match (&r, fw) {
            (Ok(r), "make") => r.exit_code.is_some_and(|c| c != 0) && r.stdout.contains("make-ran-in-sandbox"),
            (Ok(r), "go") => {
                r.tests.as_ref().is_some_and(|t| t.failed == 1 && t.failures.iter().any(|f| f == "TestAdd"))
            }
            (Ok(r), _) => r.tests.as_ref().is_some_and(|t| t.failed == 1 && t.passed == 1),
            _ => false,
        };
        let detail = match &r {
            Ok(r) => format!(
                "exit={:?} error={:?} tests={:?} out={:?} err={:?} err_start={:?}",
                r.exit_code,
                r.error,
                r.tests,
                r.stdout.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>(),
                r.stderr.chars().rev().take(600).collect::<String>().chars().rev().collect::<String>(),
                r.stderr.chars().take(400).collect::<String>()
            ),
            Err(e) => e.clone(),
        };
        if ok {
            passed += 1;
            println!("PASS {fw} tests run in the local sandbox and the failing test is found");
        } else {
            failed += 1;
            println!("FAIL {fw}: {detail}");
        }
        if fw == "cargo" {
            // Reused when the second run is clearly quicker than the first (or quick outright) and did not compile.
            let fast = r.as_ref().is_ok_and(|r| {
                (r.duration_ms < 15_000 || first_ms.is_some_and(|f| r.duration_ms * 2 < f))
                    && !r.stderr.contains("Compiling calc")
            });
            println!(
                "INFO cargo run times: first {} ms, second {} ms",
                first_ms.unwrap_or(-1),
                r.as_ref().map(|r| r.duration_ms).unwrap_or(-1)
            );
            if fast {
                passed += 1;
                println!("PASS a second cargo run reuses the per-folder build cache");
            } else {
                failed += 1;
                println!("FAIL the second cargo run rebuilt: {detail}");
            }
        }
        let _ = fs::remove_dir_all(&root);
    }
    let _ = fs::remove_dir_all(&data);
    println!("{}", serde_json::json!({"passed": passed, "failed": failed, "skipped": skipped}));
    std::process::exit(if failed == 0 { 0 } else { 1 });
}
