//! testparse.rs -- test counts from a framework's output, for the local runners (phases I5c and I5d,
//! docs/features/run-and-test.md).
//!
//! Purpose: the gateway sandbox returns a `tests` object it parsed itself (P2P-AI `test_results.go`). The local
//! runners run the same framework commands on this computer, so this file reads their output the same way and
//! gives the same [`TestSummary`]: pytest, unittest, go test (counts are packages), cargo test, and the jest and
//! node:test styles under npm test. `make` has no stable summary and gives none.
//!
//! Inputs and trust boundary: the output is untrusted text from the program under test (self-reported, OWASP
//! LLM01). It is only counted and short names are copied; nothing is run or followed.
//!
//! Security controls: bounded input (the first and last 128 KiB), at most 50 failure names of at most 200
//! characters (CWE-400, CWE-770); plain line matching, no regular expressions, so no backtracking (CWE-1333);
//! a summary that does not match gives `None`, never guessed numbers.
//!
//! Dependencies: none (standard library).

use crate::runner::TestSummary;

const MAX_PARSE: usize = 256 << 10;
const MAX_FAILURES: usize = 50;
const NAME_CHARS: usize = 200;

/// A framework's program and arguments, and the extra environment it runs with.
pub type FrameworkCommand = (Vec<&'static str>, Vec<(&'static str, &'static str)>);

/// The command line and extra environment for a framework: the same as the gateway's `testFrameworks`, so a run
/// behaves alike on every runner.
pub fn framework_command(framework: &str) -> Option<FrameworkCommand> {
    let py = vec![("PYTHONDONTWRITEBYTECODE", "1")];
    Some(match framework {
        "pytest" => (vec!["python3", "-m", "pytest", "-q", "-p", "no:cacheprovider"], py),
        "unittest" => (vec!["python3", "-m", "unittest", "discover", "-q"], py),
        "go" => {
            (vec!["go", "test", "./..."], vec![("GOPROXY", "off"), ("GOTOOLCHAIN", "local"), ("GOTELEMETRY", "off")])
        }
        "npm" => (
            vec!["npm", "test", "--silent"],
            vec![
                ("NPM_CONFIG_OFFLINE", "true"),
                ("NPM_CONFIG_UPDATE_NOTIFIER", "false"),
                ("NPM_CONFIG_FUND", "false"),
                ("NPM_CONFIG_AUDIT", "false"),
            ],
        ),
        "cargo" => (
            vec!["cargo", "test", "--offline", "--quiet"],
            vec![("CARGO_NET_OFFLINE", "true"), ("CARGO_TERM_COLOR", "never")],
        ),
        "make" => (vec!["make", "test"], vec![]),
        _ => return None,
    })
}

/// The leading number of `s` (after spaces), if any.
fn lead_num(s: &str) -> Option<i64> {
    let d: String = s.trim_start().chars().take_while(char::is_ascii_digit).take(12).collect();
    d.parse().ok()
}

/// Every `<n> <word>` pair in `s`, words cut at the first non-letter (`3 passed, 1 failed in 0.1s`).
fn pairs(s: &str) -> Vec<(i64, String)> {
    let w: Vec<&str> = s.split(|c: char| c.is_whitespace() || c == ',').filter(|x| !x.is_empty()).collect();
    w.windows(2)
        .filter_map(|p| {
            let n = p[0].parse::<i64>().ok()?;
            let word: String = p[1].chars().take_while(char::is_ascii_alphabetic).collect();
            Some((n, word))
        })
        .collect()
}

/// A short failure name (cleaned of control characters and capped).
fn name(s: &str) -> String {
    s.trim().chars().filter(|c| !c.is_control()).take(NAME_CHARS).collect()
}

/// Reads the counts from `stdout` and `stderr` of a `framework` run, or `None` when its summary is not found.
pub fn parse(framework: &str, stdout: &str, stderr: &str) -> Option<TestSummary> {
    let mut text = format!("{stdout}\n{stderr}");
    if text.len() > MAX_PARSE {
        let mut a = MAX_PARSE / 2;
        while !text.is_char_boundary(a) {
            a -= 1;
        }
        let mut b = text.len() - MAX_PARSE / 2;
        while !text.is_char_boundary(b) {
            b += 1;
        }
        text = format!("{}\n{}", &text[..a], &text[b..]);
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut t = TestSummary::default();
    let fail = |t: &mut TestSummary, n: &str| {
        if t.failures.len() < MAX_FAILURES && !n.trim().is_empty() {
            t.failures.push(name(n));
        }
    };
    let parsed = match framework {
        "pytest" => {
            // The last summary line: `==== 3 passed, 1 failed in 0.12s ====` or `3 passed in 0.1s`.
            let words = ["passed", "failed", "skipped", "error", "errors", "xfailed", "xpassed", "deselected"];
            let sum = lines.iter().rev().find(|l| {
                let p = pairs(l.trim_matches(|c: char| c == '=' || c.is_whitespace()));
                !p.is_empty() && p.iter().all(|(_, w)| words.contains(&w.as_str()) || w == "warning" || w == "warnings")
            });
            if let Some(s) = sum {
                for (n, w) in pairs(s.trim_matches(|c: char| c == '=' || c.is_whitespace())) {
                    match w.as_str() {
                        "passed" | "xpassed" => t.passed += n,
                        "failed" | "error" | "errors" => t.failed += n,
                        "skipped" | "xfailed" | "deselected" => t.skipped += n,
                        _ => {}
                    }
                }
            }
            for l in &lines {
                if let Some(rest) = l.strip_prefix("FAILED ").or_else(|| l.strip_prefix("ERROR ")) {
                    fail(&mut t, rest.split(" - ").next().unwrap_or(""));
                }
            }
            t.total = t.passed + t.failed + t.skipped;
            sum.is_some()
        }
        "unittest" => {
            let ran =
                lines.iter().find_map(|l| l.strip_prefix("Ran ").filter(|r| r.contains(" in ")).and_then(lead_num));
            if let Some(total) = ran {
                t.total = total;
                for l in &lines {
                    let inner =
                        l.strip_prefix("FAILED (").or_else(|| l.strip_prefix("OK (")).and_then(|r| r.strip_suffix(')'));
                    if let Some(inner) = inner {
                        for part in inner.split(", ") {
                            let (k, v) = part.split_once('=').unwrap_or(("", ""));
                            let v = lead_num(v).unwrap_or(0);
                            match k {
                                "failures" | "errors" => t.failed += v,
                                "skipped" => t.skipped += v,
                                _ => {}
                            }
                        }
                    }
                }
                t.passed = (t.total - t.failed - t.skipped).max(0);
            }
            for l in &lines {
                if let Some(rest) = l.strip_prefix("FAIL: ").or_else(|| l.strip_prefix("ERROR: ")) {
                    fail(&mut t, rest);
                }
            }
            ran.is_some()
        }
        "go" => {
            // Without -v, go test prints one line per package: counts are packages.
            for l in &lines {
                if l.starts_with("ok ") || l.starts_with("ok\t") {
                    t.passed += 1;
                } else if l.starts_with("FAIL ") || l.starts_with("FAIL\t") {
                    t.failed += 1;
                } else if let Some(rest) = l.trim_start().strip_prefix("--- FAIL: ") {
                    fail(&mut t, rest.split_whitespace().next().unwrap_or(""));
                }
            }
            t.total = t.passed + t.failed;
            t.total > 0
        }
        "cargo" => {
            let mut any = false;
            for l in &lines {
                if let Some(rest) = l.strip_prefix("test result: ") {
                    let p = pairs(rest.split_once(". ").map(|x| x.1).unwrap_or(""));
                    for (n, w) in p {
                        match w.as_str() {
                            "passed" => t.passed += n,
                            "failed" => t.failed += n,
                            "ignored" => t.skipped += n,
                            _ => {}
                        }
                    }
                    any = true;
                } else if let Some(rest) = l.strip_prefix("test ").and_then(|r| r.strip_suffix(" ... FAILED")) {
                    fail(&mut t, rest);
                }
            }
            t.total = t.passed + t.failed + t.skipped;
            any
        }
        "npm" => {
            if let Some(rest) = lines.iter().find_map(|l| l.strip_prefix("Tests:")) {
                // jest: `Tests:       1 failed, 3 passed, 4 total`
                for (n, w) in pairs(rest) {
                    match w.as_str() {
                        "passed" => t.passed = n,
                        "failed" => t.failed = n,
                        "skipped" => t.skipped = n,
                        "total" => t.total = n,
                        _ => {}
                    }
                }
                true
            } else if let Some(p) = lines.iter().find_map(|l| l.strip_prefix("# pass ").and_then(lead_num)) {
                // node:test (TAP): `# pass 3`, `# fail 1`, `not ok 2 - name`.
                t.passed = p;
                t.failed = lines.iter().find_map(|l| l.strip_prefix("# fail ").and_then(lead_num)).unwrap_or(0);
                t.skipped = lines.iter().find_map(|l| l.strip_prefix("# skipped ").and_then(lead_num)).unwrap_or(0);
                t.total = t.passed + t.failed + t.skipped;
                for l in &lines {
                    if let Some(rest) = l.trim_start().strip_prefix("not ok ") {
                        fail(&mut t, rest.split_once(" - ").map(|x| x.1).unwrap_or(rest));
                    }
                }
                true
            } else {
                false
            }
        }
        _ => false,
    };
    parsed.then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frameworks_match_the_gateway() {
        assert_eq!(framework_command("pytest").unwrap().0[..3], ["python3", "-m", "pytest"]);
        assert_eq!(framework_command("cargo").unwrap().0, ["cargo", "test", "--offline", "--quiet"]);
        assert!(framework_command("bash").is_none());
        for f in crate::projects::FRAMEWORKS {
            assert!(framework_command(f).is_some(), "{f}");
        }
    }

    #[test]
    fn summaries_are_read() {
        let p = parse(
            "pytest",
            "..F\nFAILED tests/test_a.py::test_x - assert 1 == 2\n==== 2 passed, 1 failed in 0.12s ====\n",
            "",
        )
        .unwrap();
        assert_eq!((p.passed, p.failed, p.total), (2, 1, 3));
        assert_eq!(p.failures, vec!["tests/test_a.py::test_x"]);
        let u = parse("unittest", "", "F.\n======\nFAIL: test_add (tests.test_calc.T.test_add)\n---\nRan 2 tests in 0.001s\n\nFAILED (failures=1)\n")
            .unwrap();
        assert_eq!((u.passed, u.failed, u.total), (1, 1, 2));
        assert_eq!(u.failures, vec!["test_add (tests.test_calc.T.test_add)"]);
        let ok = parse("unittest", "", "..\nRan 2 tests in 0.001s\n\nOK (skipped=1)\n").unwrap();
        assert_eq!((ok.passed, ok.skipped), (1, 1));
        let c = parse(
            "cargo",
            "test a::b ... FAILED\ntest result: FAILED. 3 passed; 1 failed; 2 ignored; 0 measured\n",
            "",
        )
        .unwrap();
        assert_eq!((c.passed, c.failed, c.skipped, c.failures[0].as_str()), (3, 1, 2, "a::b"));
        let g = parse("go", "--- FAIL: TestAdd (0.00s)\nFAIL\tex.com/a\t0.1s\nok  \tex.com/b\t0.1s\n", "").unwrap();
        assert_eq!((g.passed, g.failed, g.failures[0].as_str()), (1, 1, "TestAdd"));
        let j = parse("npm", "Tests:       1 failed, 3 passed, 4 total\n", "").unwrap();
        assert_eq!((j.passed, j.failed, j.total), (3, 1, 4));
        let n = parse("npm", "not ok 2 - adds\n# pass 3\n# fail 1\n", "").unwrap();
        assert_eq!((n.passed, n.failed, n.failures[0].as_str()), (3, 1, "adds"));
        assert!(parse("make", "all good", "").is_none());
        assert!(parse("pytest", "no summary here", "").is_none());
    }

    #[test]
    fn hostile_output_is_bounded() {
        // SECURITY: huge output and many failures stay bounded (CWE-400).
        let many: String = (0..500).map(|i| format!("test t{i} ... FAILED\n")).collect();
        let big = format!("{many}{}\ntest result: FAILED. 0 passed; 500 failed; 0 ignored\n", "x".repeat(1 << 20));
        let c = parse("cargo", &big, "").unwrap();
        assert_eq!(c.failures.len(), MAX_FAILURES);
        assert_eq!(c.failed, 500);
        let long = format!("FAILED {}\u{1b}[2J - m\n1 failed in 1s\n", "n".repeat(5000));
        let p = parse("pytest", &long, "").unwrap();
        assert!(p.failures[0].chars().count() <= NAME_CHARS && !p.failures[0].contains('\u{1b}'));
    }
}
