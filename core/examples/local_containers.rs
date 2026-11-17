//! Live check of phase I5d: the container runner with a real Docker or Podman (not a unit test).
//!
//! Purpose: `container.rs` tests the argument list. This program gets each image if needed (`--prepare`), then
//! checks the confinement with the Python image (no network, read-only image, not root on Linux, `.env` not
//! copied, the real folder untouched, the time limit) and runs one tiny project per framework (cargo, go, npm with
//! node:test, make, unittest), each with one passing and one failing test, through `container::run` as the Run and
//! test view does. An image that is missing (and not prepared) is reported as skipped, not passed.
//!
//! Usage: `cargo run --example local_containers [-- --prepare]`
//! Outputs: `PASS`/`FAIL`/`SKIP` lines and a last line `{"passed": n, "failed": n, "skipped": n}`; exit 0 only
//! when nothing failed.
//!
//! Security notes: everything runs in containers with no network, on throw-away folders in the temp folder. With
//! `--prepare` the images are downloaded (pull by digest) or built (Rust, from the pinned UBI base).
//!
//! Dependencies: the core crate and serde_json only.

use p2p_ide_core::container;
use p2p_ide_core::runner::{check, RunInput};
use p2p_ide_core::workspace::Workspace;
use std::fs;
use std::path::{Path, PathBuf};

/// A test project: framework, the image it needs, and its files.
type Project = (&'static str, &'static str, Vec<(&'static str, &'static str)>);

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("p2pide-ct-{tag}-{}", p2p_ide_core::store::new_id()));
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

fn main() {
    let (mut passed, mut failed, mut skipped) = (0, 0, 0);
    let a = container::available();
    if !a.available {
        println!("SKIP the container runner is not available here: {}", a.reason);
        println!("{}", serde_json::json!({"passed": 0, "failed": 0, "skipped": 1}));
        return;
    }
    println!("INFO engine: {}", container::engine().map(|e| e.kind).unwrap_or("none"));
    if std::env::args().any(|x| x == "--prepare") {
        for i in container::images() {
            if !i.present {
                let r = container::prepare(i.id, &mut |_| {});
                println!("INFO get image {}: {}", i.id, r.err().unwrap_or_else(|| "ok".into()));
            }
        }
    }
    let present: Vec<&str> = container::images().into_iter().filter(|i| i.present).map(|i| i.id).collect();
    let data = tmp("data");
    let mut check_ok = |name: &str, ok: bool, detail: String| {
        if ok {
            passed += 1;
            println!("PASS {name}");
        } else {
            failed += 1;
            println!("FAIL {name}: {detail}");
        }
    };

    // Confinement, with the Python image.
    if present.contains(&"python") {
        let root = tmp("conf");
        write(&root, &[("hello.txt", "hello from the copy\n"), (".env", "API_KEY=local-do-not-copy\n")]);
        let ws = Workspace::open(&root).unwrap();
        let py = |code: &str, t: Option<u32>| {
            let req = check(
                &ws,
                &RunInput {
                    kind: "command".into(),
                    argv: vec!["python3".into(), "-c".into(), code.into()],
                    timeout_s: t,
                    ..Default::default()
                },
            )
            .unwrap();
            container::run(&ws, &req, "python", &data, &mut |_| {})
        };
        let code = "import os,socket\n\
print('read', open('hello.txt').read().strip())\n\
open('made.txt','w').write('x')\n\
print('env', os.path.exists('.env'))\n\
print('uid', os.getuid())\n\
try:\n    open('/usr/planted','w'); print('rootfs writable')\nexcept OSError: print('rootfs read-only')\n\
try:\n    socket.create_connection(('1.1.1.1', 80), timeout=5); print('net open')\nexcept OSError: print('net closed')\n";
        let r = py(code, None);
        let d = format!("{r:?}");
        let out = r.as_ref().map(|r| r.stdout.clone()).unwrap_or_default();
        check_ok("the copy is readable and writable in /work", out.contains("read hello from the copy"), d.clone());
        check_ok(".env was not copied into the container", out.contains("env False"), d.clone());
        check_ok("the image's file system is read-only", out.contains("rootfs read-only"), d.clone());
        check_ok("the container has no network", out.contains("net closed"), d.clone());
        check_ok("the real folder is untouched", !root.join("made.txt").exists(), d.clone());
        if cfg!(target_os = "linux") {
            // SAFETY: getuid cannot fail.
            let me = unsafe { libc_getuid() };
            check_ok("the container runs as the person's own uid", out.contains(&format!("uid {me}")), d.clone());
        } else {
            check_ok("the container does not run as root", !out.contains("uid 0"), d.clone());
        }
        let r = py("import time\ntime.sleep(120)", Some(10));
        check_ok("the time limit ends the container", r.as_ref().is_ok_and(|r| r.timed_out), format!("{r:?}"));
        let _ = fs::remove_dir_all(&root);
    } else {
        skipped += 1;
        println!("SKIP confinement: the python image is not on this computer");
    }

    let projects: Vec<Project> = vec![
        (
            "cargo",
            "rust",
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
            "node",
            vec![
                ("package.json", "{\"name\":\"calc\",\"version\":\"1.0.0\",\"scripts\":{\"test\":\"node --test\"}}\n"),
                (
                    "test/calc.test.js",
                    "const test = require('node:test');\nconst assert = require('node:assert');\nconst add = (a, b) => a - b;\ntest('adds', () => assert.strictEqual(add(2, 3), 5));\ntest('zero', () => assert.strictEqual(add(0, 0), 0));\n",
                ),
            ],
        ),
        ("make", "go", vec![("Makefile", "test:\n\t@echo make-ran-in-sandbox\n\t@false\n")]),
        (
            "unittest",
            "python",
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
    for (fw, image, files) in projects {
        if !present.contains(&image) {
            println!("SKIP {fw}: the {image} image is not on this computer");
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
        let r = container::run(&ws, &req, "", &data, &mut |_| {});
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
                "exit={:?} error={:?} tests={:?} out={:?} err={:?}",
                r.exit_code,
                r.error,
                r.tests,
                r.stdout.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>(),
                r.stderr.chars().take(600).collect::<String>()
            ),
            Err(e) => e.clone(),
        };
        check_ok(&format!("{fw} tests run in a container and the failing test is found"), ok, detail);
        let _ = fs::remove_dir_all(&root);
    }
    let _ = fs::remove_dir_all(&data);
    println!("{}", serde_json::json!({"passed": passed, "failed": failed, "skipped": skipped}));
    std::process::exit(if failed == 0 { 0 } else { 1 });
}

#[cfg(unix)]
/// The person's uid (Linux check above).
unsafe fn libc_getuid() -> u32 {
    extern "C" {
        fn getuid() -> u32;
    }
    getuid()
}

#[cfg(not(unix))]
/// Not used off Unix.
unsafe fn libc_getuid() -> u32 {
    0
}
