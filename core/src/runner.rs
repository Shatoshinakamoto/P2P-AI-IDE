//! runner.rs -- running the project's tests and commands in a sandbox (phase I5b, docs/features/run-and-test.md).
//!
//! Purpose: the person (Run and test view) or, after the person approves each call, the chat assistant runs a test
//! framework or one program on the open folder. Phase I5b runs it on the gateway: the folder is sent to the linked
//! project (phase I4 push), the node's copy is brought up to date, and the gateway's sandbox tool runs the step on a
//! snapshot of that copy (no network, no shell, changes not kept). Later runners (a local OS sandbox, a local
//! container) take the same [`RunRequest`] and give the same [`RunResult`].
//!
//! Inputs and trust boundaries:
//!   * the request comes from the webview or from model output (untrusted): checked by [`check`];
//!   * the gateway's answer (exit code, output, test summary) is untrusted data, cleaned and capped by
//!     [`parse_tool_result`] and shown as text.
//!
//! Security controls:
//!   * No shell anywhere: a command is an argument list; a test run is a framework name from a fixed list; both run
//!     in the gateway node's sandbox runner (no network, read-only, non-root) on a snapshot (CWE-78, NIST SC-39).
//!   * Strict request shape: 1-64 arguments of at most 4096 bytes without control characters, a folder that passes
//!     the workspace allow-list, 10-300 s (CWE-20, CWE-22, CWE-400).
//!   * What leaves the computer is the push of phase I4 (deny list for secrets, digests); see projects.rs.
//!   * Output is capped (64 KiB per stream kept, 16 KiB given to the model) and control characters other than
//!     newline and tab are removed (CWE-150, CWE-400).
//!   * Model-started runs need the person's approval per call; that gate lives in the shell (agent.rs), which is
//!     the only caller that passes a runner to the agent loop.
//!
//! Dependencies: serde, serde_json, the core's client, gwapi and projects modules.

use crate::client::{Client, NetError};
use crate::projects::{self, sync_view, FRAMEWORKS};
use crate::workspace::Workspace;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Most arguments of a command, and the longest argument (the gateway's own limits).
pub const MAX_ARGS: usize = 64;
pub const MAX_ARG_BYTES: usize = 4096;
/// Output kept per stream for the person.
pub const MAX_STREAM: usize = 64 << 10;
/// Output given to the model per run.
pub const MAX_MODEL_RESULT: usize = 16 << 10;
/// Default and allowed run time, in seconds.
pub const DEFAULT_TIMEOUT_S: u32 = 120;
pub const MIN_TIMEOUT_S: u32 = 10;
pub const MAX_TIMEOUT_S: u32 = 300;
/// How long to wait for the node's copy to be up to date.
const COPY_WAIT: Duration = Duration::from_secs(120);

/// What to run, as the webview or the model sends it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct RunInput {
    /// `tests` or `command`.
    pub kind: String,
    #[serde(default)]
    pub framework: String,
    #[serde(default)]
    pub argv: Vec<String>,
    /// Folder to run in, relative to the root (`""` = the root).
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub timeout_s: Option<u32>,
}

/// A checked request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunRequest {
    /// `run_tests` or `run_command` (the gateway tool name).
    pub tool: &'static str,
    pub framework: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub timeout_s: u32,
}

impl RunRequest {
    /// One line naming what runs, for the chat and the approval card (`run_tests pytest`, `run_command python3 -V`).
    pub fn summary(&self) -> String {
        let what = if self.tool == "run_tests" { self.framework.clone() } else { self.argv.join(" ") };
        let at = if self.cwd.is_empty() { String::new() } else { format!(" (in {})", self.cwd) };
        format!("{} {what}{at}", self.tool).chars().take(400).collect()
    }
}

/// A character refused in an argument: control characters other than newline and tab (a multi-line `python3 -c`
/// script is fine without a shell; NUL, escape and the like are not, CWE-150, CWE-158).
fn bad_char(c: char) -> bool {
    c.is_control() && c != '\n' && c != '\t'
}

/// Checks a request. Security: strict allow-list of shapes and sizes (CWE-20, CWE-22, CWE-88).
pub fn check(ws: &Workspace, input: &RunInput) -> Result<RunRequest, &'static str> {
    let cwd = input.cwd.trim_matches('/').to_string();
    if !cwd.is_empty() && (ws.check_rel(&cwd).is_err() || crate::agent::denied(&cwd)) {
        return Err("the folder to run in is not allowed");
    }
    let timeout_s = input.timeout_s.unwrap_or(DEFAULT_TIMEOUT_S);
    if !(MIN_TIMEOUT_S..=MAX_TIMEOUT_S).contains(&timeout_s) {
        return Err("the time limit is 10 to 300 seconds");
    }
    match input.kind.as_str() {
        "tests" => {
            if !FRAMEWORKS.contains(&input.framework.as_str()) || !input.argv.is_empty() {
                return Err("pick a test framework: cargo, go, make, npm, pytest or unittest");
            }
            Ok(RunRequest { tool: "run_tests", framework: input.framework.clone(), argv: vec![], cwd, timeout_s })
        }
        "command" => {
            let argv = &input.argv;
            if argv.is_empty()
                || argv.len() > MAX_ARGS
                || !input.framework.is_empty()
                || argv.iter().any(|a| a.is_empty() || a.len() > MAX_ARG_BYTES || a.chars().any(bad_char))
            {
                return Err("a command is a program and its arguments: 1 to 64 words, no control characters");
            }
            Ok(RunRequest { tool: "run_command", framework: String::new(), argv: argv.clone(), cwd, timeout_s })
        }
        _ => Err("run tests or a command"),
    }
}

/// Splits a typed command line into words on spaces; `"..."` keeps spaces inside one word. No other shell rules:
/// no variables, globbing, pipes or redirection (the words go to the program as they are).
pub fn split_words(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let (mut quoted, mut any) = (false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

/// Numbers read from a test framework's output.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct TestSummary {
    pub passed: i64,
    pub failed: i64,
    pub skipped: i64,
    pub total: i64,
    pub failures: Vec<String>,
}

/// What a run gave back. All text is untrusted output, cleaned.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct RunResult {
    /// `gateway` (later also `local-os`, `container`).
    pub runner: String,
    /// Where it ran (the node's name), for the person.
    pub place: String,
    pub summary: String,
    pub exit_code: Option<i64>,
    pub timed_out: bool,
    pub duration_ms: i64,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub tests: Option<TestSummary>,
    /// Set when the step could not start (fixed words from the runner, cleaned).
    pub error: String,
}

impl RunResult {
    /// True when the step ran and ended with exit code 0.
    pub fn ok(&self) -> bool {
        self.error.is_empty() && !self.timed_out && self.exit_code == Some(0)
    }

    /// The short text the model gets as the tool result (capped).
    pub fn for_model(&self) -> String {
        let mut s = format!(
            "exit_code: {}\ntimed_out: {}\n",
            self.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "none".into()),
            self.timed_out
        );
        if !self.error.is_empty() {
            s.push_str(&format!("error: {}\n", self.error));
        }
        if let Some(t) = &self.tests {
            s.push_str(&format!("tests: {} passed, {} failed, {} skipped\n", t.passed, t.failed, t.skipped));
            for f in t.failures.iter().take(20) {
                s.push_str(&format!("failed: {f}\n"));
            }
        }
        let half = MAX_MODEL_RESULT / 2;
        s.push_str("stdout:\n");
        s.push_str(&tail(&self.stdout, half));
        s.push_str("\nstderr:\n");
        s.push_str(&tail(&self.stderr, half));
        s
    }
}

/// The last `max` bytes of `s` (on a character boundary), where test summaries usually are.
fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    format!("[...]{}", &s[start..])
}

/// Removes control characters except newline and tab, and caps the length (CWE-150, CWE-400).
fn clean_out(v: &Value, max: usize) -> (String, bool) {
    let s = v.as_str().unwrap_or("");
    let cut = s.len() > max;
    let mut out: String = s.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect();
    if out.len() > max {
        let mut end = max;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    (out, cut)
}

/// Reads the gateway's `workspace.tool_result` for a sandbox step.
pub fn parse_tool_result(v: &Value, req: &RunRequest) -> Option<RunResult> {
    if v.get("object").and_then(Value::as_str) != Some("workspace.tool_result") {
        return None;
    }
    let s = v.get("structured")?;
    let (stdout, c1) = clean_out(s.get("stdout").unwrap_or(&Value::Null), MAX_STREAM);
    let (stderr, c2) = clean_out(s.get("stderr").unwrap_or(&Value::Null), MAX_STREAM);
    let word = |k: &str| projects::clean(s, k, 120);
    let tests = s.get("tests").filter(|t| t.is_object()).map(|t| TestSummary {
        passed: t.get("passed").and_then(Value::as_i64).unwrap_or(0),
        failed: t.get("failed").and_then(Value::as_i64).unwrap_or(0),
        skipped: t.get("skipped").and_then(Value::as_i64).unwrap_or(0),
        total: t.get("total").and_then(Value::as_i64).unwrap_or(0),
        failures: t
            .get("failures")
            .and_then(Value::as_array)
            .map(|a| a.iter().take(50).map(|f| projects::clean(f, "name", 300)).filter(|n| !n.is_empty()).collect())
            .unwrap_or_default(),
    });
    let start_error = word("start_error");
    let status = word("status");
    Some(RunResult {
        runner: "gateway".into(),
        place: projects::clean(v, "peer", 120),
        summary: req.summary(),
        exit_code: s.get("exit_code").and_then(Value::as_i64),
        timed_out: s.get("timed_out").and_then(Value::as_bool).unwrap_or(false),
        duration_ms: s.get("duration_ms").and_then(Value::as_i64).unwrap_or(0),
        stdout,
        stderr,
        truncated: c1 || c2 || s.get("truncated").and_then(Value::as_bool).unwrap_or(false),
        tests,
        error: if !start_error.is_empty() {
            start_error
        } else if status != "succeeded" && status != "failed" && !status.is_empty() {
            status
        } else {
            String::new()
        },
    })
}

/// Runs a request on the gateway: sends the folder (only changed files), waits for the node's copy, runs the step.
///
/// * `pid` -- the linked project; `stage` -- called with a short word as each stage starts (`sending`, `copying`,
///   `running`) so the UI can say what is happening.
pub fn gateway_run(
    client: &Client,
    ws: &Workspace,
    pid: &str,
    req: &RunRequest,
    stage: &mut dyn FnMut(&'static str),
) -> Result<RunResult, NetError> {
    if !projects::id_ok(pid) {
        return Err(NetError::BadRequest("link this folder to a gateway project first (Missions view)"));
    }
    stage("sending");
    projects::push(client, ws, pid)?;
    stage("copying");
    let base = format!("/v1/projects/{pid}/workspace/sync");
    let t = Instant::now();
    loop {
        let st = sync_view(&client.api("GET", &base, None)?).state;
        match st.as_str() {
            "ready" => break,
            "failed" | "stale" | "none" => {
                client.api("POST", &base, Some(&json!({})))?;
            }
            _ => {}
        }
        if t.elapsed() > COPY_WAIT {
            return Err(NetError::BadRequest("the node's copy of the project is not ready yet; try again shortly"));
        }
        std::thread::sleep(Duration::from_millis(800));
    }
    stage("running");
    let mut args = json!({"cwd": req.cwd});
    if req.tool == "run_tests" {
        args["framework"] = json!(req.framework);
    } else {
        args["argv"] = json!(req.argv);
    }
    let body = json!({"arguments": args, "timeout_ms": req.timeout_s * 1000});
    let v = client.api_slow("POST", &format!("/v1/projects/{pid}/workspace/tools/{}", req.tool), Some(&body))?;
    parse_tool_result(&v, req).ok_or(NetError::BadAnswer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> (Workspace, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("p2pide-run-{}", crate::store::new_id()));
        std::fs::create_dir_all(d.join("src")).unwrap();
        (Workspace::open(&d).unwrap(), d)
    }

    #[test]
    fn requests_are_checked() {
        let (w, d) = ws();
        let t = |kind: &str, fw: &str, argv: &[&str], cwd: &str| RunInput {
            kind: kind.into(),
            framework: fw.into(),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.into(),
            timeout_s: None,
        };
        let r = check(&w, &t("tests", "pytest", &[], "")).unwrap();
        assert_eq!((r.tool, r.timeout_s), ("run_tests", DEFAULT_TIMEOUT_S));
        assert!(check(&w, &t("command", "", &["python3", "-c", "import os\n\tprint(1)"], "")).is_ok());
        let r = check(&w, &t("command", "", &["python3", "-c", "print(1)"], "src")).unwrap();
        assert_eq!(r.summary(), "run_command python3 -c print(1) (in src)");
        for bad in [
            t("tests", "bash", &[], ""),
            t("tests", "pytest", &["x"], ""),
            t("command", "", &[], ""),
            t("command", "", &["ls", ""], ""),
            t("command", "", &["echo", "a\u{0}b"], ""),
            t("command", "", &["echo", "a\u{1b}[2Jb"], ""),
            t("command", "pytest", &["ls"], ""),
            t("command", "", &["ls"], "../up"),
            t("command", "", &["ls"], ".git"),
            t("command", "", &["ls"], ".ssh/x"),
            t("shell", "", &["ls"], ""),
        ] {
            assert!(check(&w, &bad).is_err(), "{bad:?}");
        }
        let many: Vec<&str> = vec!["a"; 65];
        assert!(check(&w, &t("command", "", &many, "")).is_err());
        let mut long = t("tests", "go", &[], "");
        long.timeout_s = Some(301);
        assert!(check(&w, &long).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn words_split_like_the_missions_view() {
        assert_eq!(split_words("python3 -m  pytest -q"), vec!["python3", "-m", "pytest", "-q"]);
        assert_eq!(split_words(r#"echo "a b" c"#), vec!["echo", "a b", "c"]);
        assert_eq!(split_words(r#"echo "" x"#), vec!["echo", "", "x"]);
        // SECURITY: shell syntax is just text for the program.
        assert_eq!(split_words("ls; rm -rf /"), vec!["ls;", "rm", "-rf", "/"]);
    }

    #[test]
    fn gateway_answers_are_cleaned_and_capped() {
        let req = RunRequest {
            tool: "run_tests",
            framework: "pytest".into(),
            argv: vec![],
            cwd: String::new(),
            timeout_s: 60,
        };
        let big = "x".repeat(MAX_STREAM + 10);
        let v = json!({"object": "workspace.tool_result", "peer": "node\u{1b}[2J-1", "structured": {
            "status": "failed", "exit_code": 1, "timed_out": false, "duration_ms": 42,
            "stdout": format!("ok\u{7}\n{big}"), "stderr": "E\tbad\n",
            "tests": {"passed": 3, "failed": 1, "skipped": 0, "total": 4, "failures": [{"name": "test_add"}]}}});
        let r = parse_tool_result(&v, &req).unwrap();
        assert_eq!(r.place, "node[2J-1");
        assert_eq!(r.exit_code, Some(1));
        assert!(r.truncated && r.stdout.len() <= MAX_STREAM && !r.stdout.contains('\u{7}'));
        assert_eq!(r.stderr, "E\tbad\n");
        assert_eq!(r.tests.as_ref().unwrap().failures, vec!["test_add"]);
        assert!(r.error.is_empty() && !r.ok());
        let m = r.for_model();
        assert!(
            m.len() <= MAX_MODEL_RESULT + 400
                && m.contains("tests: 3 passed, 1 failed")
                && m.contains("failed: test_add")
        );
        assert!(parse_tool_result(&json!({"object": "x"}), &req).is_none());
        let started = json!({"object": "workspace.tool_result", "structured": {"status": "error", "start_error": "program not found"}});
        assert_eq!(parse_tool_result(&started, &req).unwrap().error, "program not found");
    }
}
