//! projects -- the open folder as a gateway project, for missions (phase I4, docs/features/missions.md).
//!
//! Purpose: Autopilot and Auto Improve run on a Go gateway, on a *project* whose workspace holds the code. This
//! module decides what to send (the push plan), what came back (the pull plan), remembers which project belongs to
//! which folder (links.json), and turns gateway answers into small fixed view shapes for the webview.
//!
//! Inputs and trust boundaries:
//!   * the open folder (the person's files) -- read through [`Workspace`], never followed through symlinks;
//!   * gateway answers (untrusted): every field is picked by name, typed, length-capped and stripped of control
//!     characters before it reaches the webview, which shows it as text only (CWE-20, CWE-79, CWE-150);
//!   * ids from the webview (project, mission, proposal, run, approval): checked against fixed patterns before they
//!     go into a URL path (CWE-74, CWE-918).
//!
//! Security controls:
//!   * Secrets stay home: the same deny list as the chat tools (`.env`, keys, `.git/`, `.ssh/` ...) applies to the
//!     push, and those names are also never pulled back (CWE-200, CWE-538).
//!   * Only names the gateway's path grammar accepts are sent (ASCII subset, no Windows device names, no case
//!     collisions), so one odd file cannot fail the whole commit; others are listed as skipped (CWE-22, CWE-178).
//!   * Caps: 2000 files, 5 MiB per file, 100 MiB per push; 50 files and 5 MiB each per pull (CWE-400, CWE-770).
//!   * Nothing from the gateway is written directly: pulled files become pending changes that the person accepts
//!     per file in the diff review, with the same version check as the editor (OWASP LLM06, CWE-367).
//!
//! Dependencies: serde, serde_json, sha2 (already in the crate).

use crate::agent::{denied, propose_external, Pending};
use crate::client::{Client, NetError};
use crate::gwapi::encode_path;
use crate::store::{self, StoreError};
use crate::workspace::{version_of, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

/// Most files sent in one push.
pub const MAX_PUSH_FILES: usize = 2000;
/// Largest file sent.
pub const MAX_PUSH_FILE: u64 = 5 << 20;
/// Most bytes sent in one push.
pub const MAX_PUSH_TOTAL: u64 = 100 << 20;
/// Most files brought back in one pull.
pub const MAX_PULL_FILES: usize = 50;
/// Most changes per gateway commit (the gateway's own limit).
pub const MAX_COMMIT_CHANGES: usize = 1000;
/// Most saved folder links.
const MAX_LINKS: usize = 200;

/// Windows device names, refused by the gateway's path grammar in any segment.
const DEVICE_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2",
    "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Whether a relative path is safe to send: a subset of the gateway's workspace path grammar (P2P-AI
/// `projects-and-workspaces.md`, "Path rules"), kept to ASCII so no Unicode spelling question can arise.
pub fn gateway_path_ok(rel: &str) -> bool {
    if rel.is_empty() || rel.len() > 512 {
        return false;
    }
    let segs: Vec<&str> = rel.split('/').collect();
    if segs.len() > 32 {
        return false;
    }
    segs.iter().all(|s| {
        let base = s.split('.').next().unwrap_or("").trim_end().to_ascii_lowercase();
        !s.is_empty()
            && s.len() <= 255
            && *s != "."
            && *s != ".."
            && !s.starts_with(' ')
            && !s.ends_with(' ')
            && !s.ends_with('.')
            && s.chars().all(|c| c.is_ascii_alphanumeric() || " -_.()+@,=~".contains(c))
            && !DEVICE_NAMES.contains(&base.as_str())
    })
}

/// An id from the webview or a gateway answer that goes into a URL path: 1-64 of `[A-Za-z0-9_-]`.
pub fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Text from a gateway answer, made safe to show: control characters removed (CWE-150), cut to `max` characters.
pub fn clean(v: &Value, key: &str, max: usize) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(max)
        .collect()
}

fn num(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// One file in the gateway project's workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Parses `GET /v1/projects/{id}/workspace/files` (`{data: [{path, bytes, sha256}]}`), keeping only well-formed
/// entries (path passes [`gateway_path_ok`], sha256 is 64 hex).
pub fn parse_remote_files(v: &Value) -> Vec<RemoteFile> {
    v.get("data")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .take(10_000)
                .filter_map(|f| {
                    let path = f.get("path")?.as_str()?.to_string();
                    let sha = f.get("sha256")?.as_str()?.to_ascii_lowercase();
                    let ok = gateway_path_ok(&path) && sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit());
                    ok.then(|| RemoteFile {
                        path,
                        bytes: f.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                        sha256: sha,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A file left out, with a fixed reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub rel: String,
    pub reason: &'static str,
}

/// What a push will do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushPlan {
    /// Files to upload (new or changed), with their local SHA-256.
    pub upload: Vec<(String, String)>,
    /// Paths on the gateway that no longer exist locally.
    pub delete: Vec<String>,
    pub unchanged: usize,
    pub skipped: Vec<Skipped>,
}

/// Picks which local files may be sent, before any is read: deny list, path grammar, size, case collisions, caps.
///
/// * `files` -- `(rel, size)` from [`crate::Workspace::all_files`].
///
/// Returns the files to hash and the ones skipped.
pub fn eligible(files: &[(String, u64)]) -> (Vec<String>, Vec<Skipped>) {
    let mut ok = Vec::new();
    let mut skipped = Vec::new();
    let mut folded: HashSet<String> = HashSet::new();
    let mut total = 0u64;
    for (rel, size) in files {
        let reason = if denied(rel) {
            // SECURITY: likely secrets are never sent (CWE-200).
            Some("may hold secrets, kept on this computer")
        } else if !gateway_path_ok(rel) {
            Some("name the gateway does not accept")
        } else if *size > MAX_PUSH_FILE {
            Some("larger than 5 MiB")
        } else if !folded.insert(rel.to_ascii_lowercase()) {
            Some("differs from another name only by case")
        } else if ok.len() >= MAX_PUSH_FILES || total + size > MAX_PUSH_TOTAL {
            Some("over the push limit (2000 files, 100 MiB)")
        } else {
            None
        };
        match reason {
            Some(r) => skipped.push(Skipped { rel: rel.clone(), reason: r }),
            None => {
                total += size;
                ok.push(rel.clone());
            }
        }
    }
    (ok, skipped)
}

/// Compares hashed local files with the gateway's list.
///
/// * `local` -- `(rel, sha256)` of every eligible local file.
/// * `skipped_local` -- local paths that exist but were skipped; their gateway copies are kept, not deleted.
pub fn plan_push(local: &[(String, String)], remote: &[RemoteFile], skipped_local: &[Skipped]) -> PushPlan {
    let remote_map: BTreeMap<&str, &str> = remote.iter().map(|r| (r.path.as_str(), r.sha256.as_str())).collect();
    let mut plan = PushPlan::default();
    let mut local_set: HashSet<&str> = HashSet::new();
    for (rel, sha) in local {
        local_set.insert(rel);
        match remote_map.get(rel.as_str()) {
            Some(r) if *r == sha => plan.unchanged += 1,
            _ => plan.upload.push((rel.clone(), sha.clone())),
        }
    }
    let kept: HashSet<&str> = skipped_local.iter().map(|s| s.rel.as_str()).collect();
    for r in remote {
        if !local_set.contains(r.path.as_str()) && !kept.contains(r.path.as_str()) {
            plan.delete.push(r.path.clone());
        }
    }
    plan
}

/// One file to bring back, and whether it exists locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullItem {
    pub path: String,
    pub sha256: String,
    /// The local version (SHA-256) it would replace, or `None` for a new file.
    pub local_version: Option<String>,
}

/// What a pull will do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullPlan {
    pub fetch: Vec<PullItem>,
    pub same: usize,
    /// Local files the gateway project does not have (left alone; deletes are not brought back).
    pub only_local: usize,
    pub skipped: Vec<Skipped>,
}

/// Compares the gateway's files with local versions and lists what differs.
///
/// * `local` -- `(rel, sha256)` of local files (any size the caller could hash).
pub fn plan_pull(local: &[(String, String)], remote: &[RemoteFile]) -> PullPlan {
    let local_map: BTreeMap<&str, &str> = local.iter().map(|(r, v)| (r.as_str(), v.as_str())).collect();
    let mut plan = PullPlan::default();
    let remote_set: HashSet<&str> = remote.iter().map(|r| r.path.as_str()).collect();
    for r in remote {
        let lv = local_map.get(r.path.as_str()).map(|s| s.to_string());
        if lv.as_deref() == Some(r.sha256.as_str()) {
            plan.same += 1;
            continue;
        }
        let reason = if denied(&r.path) {
            // SECURITY: a gateway cannot plant a file where the person keeps secrets (.env, .git/, keys).
            Some("a name kept private; not brought back")
        } else if r.bytes > MAX_PUSH_FILE {
            Some("larger than 5 MiB")
        } else if plan.fetch.len() >= MAX_PULL_FILES {
            Some("over the limit of 50 files per pull; pull again after reviewing")
        } else {
            None
        };
        match reason {
            Some(why) => plan.skipped.push(Skipped { rel: r.path.clone(), reason: why }),
            None => plan.fetch.push(PullItem { path: r.path.clone(), sha256: r.sha256.clone(), local_version: lv }),
        }
    }
    plan.only_local = local.iter().filter(|(r, _)| !remote_set.contains(r.as_str())).count();
    plan
}

/// Checks downloaded bytes against the listed digest and makes them text.
///
/// Security: a mismatch means the file changed or was tampered with between list and download, so it is refused
/// (NIST SI-7, CWE-345); only UTF-8 text goes into the diff review.
pub fn check_download(item: &PullItem, bytes: Vec<u8>) -> Result<String, &'static str> {
    if version_of(&bytes) != item.sha256 {
        return Err("changed on the gateway while downloading; pull again");
    }
    String::from_utf8(bytes).map_err(|_| "not a text file; get it from the gateway's web app")
}

// ---- push and pull against a live gateway

/// What a push did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PushReport {
    pub uploaded: usize,
    pub deleted: usize,
    pub unchanged: usize,
    pub skipped: Vec<Skipped>,
    /// The project's workspace version after the push.
    pub version: i64,
    /// State of the copy to a node after asking for one (`pending`, `ready`, ...), or `off` when the gateway does
    /// not copy projects to nodes (then Autopilot cannot run there).
    pub copy: String,
}

/// What a pull did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PullReport {
    /// Files now waiting in "Changes to review".
    pub proposed: usize,
    pub same: usize,
    pub only_local: usize,
    pub skipped: Vec<Skipped>,
}

fn files_path(pid: &str) -> String {
    format!("/v1/projects/{pid}/workspace/files")
}

/// Errors that stop a whole push or pull (the gateway is unreachable, the key is refused, ...), as opposed to one
/// file being refused.
fn fatal(e: &NetError) -> bool {
    !matches!(e, NetError::Gateway(_) | NetError::BadRequest(_) | NetError::Status(400 | 413 | 415))
}

fn file_reason(e: &NetError) -> &'static str {
    match e {
        NetError::Gateway(m) | NetError::BadRequest(m) => m,
        _ => "the gateway refused this file",
    }
}

/// Waits (up to 60 s) until uploaded files leave quarantine, when the gateway scans uploads.
fn wait_scanned(client: &Client, ids: &[String]) -> Result<(), NetError> {
    let t0 = Instant::now();
    for id in ids {
        loop {
            let f = client.api("GET", &format!("/v1/files/{id}"), None)?;
            match f.get("state").and_then(Value::as_str) {
                Some("quarantined") if t0.elapsed() < Duration::from_secs(60) => {
                    std::thread::sleep(Duration::from_secs(2))
                }
                Some("quarantined") => {
                    return Err(NetError::Gateway("a file is still being scanned on the gateway; try again shortly"))
                }
                Some("rejected") => return Err(NetError::Gateway("the gateway's malware scan rejected a file")),
                _ => break,
            }
        }
    }
    Ok(())
}

/// Sends the open folder to the gateway project: uploads new and changed files, deletes files that are gone,
/// commits them as the next version, then asks the gateway to copy the project to a node (for Autopilot).
///
/// Security: [`eligible`] keeps secrets and odd names home; each file is re-read and re-hashed right before it is
/// sent; the commit carries `base_version`, so a concurrent change on the gateway is never overwritten (CWE-362).
pub fn push(client: &Client, ws: &Workspace, pid: &str) -> Result<PushReport, NetError> {
    if !id_ok(pid) {
        return Err(NetError::BadRequest("pick a gateway project first"));
    }
    let info = client.api("GET", &format!("/v1/projects/{pid}/workspace"), None)?;
    let mut version = num(&info, "version");
    let remote = parse_remote_files(&client.api("GET", &files_path(pid), None)?);
    let (all, truncated) = ws.all_files(MAX_PUSH_FILES * 4);
    let (ok, mut skipped) = eligible(&all);
    if truncated {
        skipped.push(Skipped {
            rel: "(rest of the folder)".into(),
            reason: "the folder has too many files; only the first are sent",
        });
    }
    let mut local = Vec::new();
    for rel in ok {
        match ws.read_bytes(&rel, MAX_PUSH_FILE) {
            Ok((_, v)) => local.push((rel, v)),
            Err(_) => skipped.push(Skipped { rel, reason: "could not be read" }),
        }
    }
    let plan = plan_push(&local, &remote, &skipped);
    let mut changes: Vec<Value> = Vec::new();
    let mut ids = Vec::new();
    let mut report = PushReport { unchanged: plan.unchanged, ..Default::default() };
    for (rel, sha) in &plan.upload {
        let Ok((bytes, v)) = ws.read_bytes(rel, MAX_PUSH_FILE) else {
            skipped.push(Skipped { rel: rel.clone(), reason: "could not be read" });
            continue;
        };
        if &v != sha {
            skipped.push(Skipped { rel: rel.clone(), reason: "changed while sending; push again" });
            continue;
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        match client.upload_file(name, &bytes) {
            Ok(f) => {
                let id = clean(&f, "id", 64);
                if !id_ok(&id) {
                    return Err(NetError::BadAnswer);
                }
                if f.get("state").and_then(Value::as_str) == Some("quarantined") {
                    ids.push(id.clone());
                }
                changes.push(serde_json::json!({"path": rel, "file_id": id}));
            }
            Err(e) if !fatal(&e) => skipped.push(Skipped { rel: rel.clone(), reason: file_reason(&e) }),
            Err(e) => return Err(e),
        }
    }
    report.uploaded = changes.len();
    for d in &plan.delete {
        changes.push(serde_json::json!({"path": d, "delete": true}));
    }
    report.deleted = plan.delete.len();
    wait_scanned(client, &ids)?;
    for chunk in changes.chunks(MAX_COMMIT_CHANGES) {
        let body = serde_json::json!({"base_version": version, "changes": chunk});
        let r = client.api("POST", &format!("/v1/projects/{pid}/workspace/commit"), Some(&body))?;
        version = num(&r, "version");
    }
    report.version = version;
    report.skipped = skipped;
    report.copy = match client.api("POST", &format!("/v1/projects/{pid}/workspace/sync"), Some(&serde_json::json!({})))
    {
        Ok(v) => clean(&v, "state", 16),
        Err(
            NetError::NotOffered
            | NetError::Status(503)
            | NetError::Gateway("this gateway has that feature switched off"),
        ) => "off".into(),
        Err(NetError::Gateway(m)) => m.into(),
        Err(e) => return Err(e),
    };
    Ok(report)
}

/// Brings the gateway project's files back as pending changes for review (after "Collect changes" moved the
/// node copy's work into the project).
///
/// Security: names kept private are never brought back; each download is checked against the listed SHA-256 and
/// must be text; every file waits for the person's Accept (OWASP LLM06, NIST SI-7).
pub fn pull(client: &Client, ws: &Workspace, pending: &mut Pending, pid: &str) -> Result<PullReport, NetError> {
    if !id_ok(pid) {
        return Err(NetError::BadRequest("pick a gateway project first"));
    }
    let remote = parse_remote_files(&client.api("GET", &files_path(pid), None)?);
    let mut local = Vec::new();
    for r in &remote {
        if let Ok((_, v)) = ws.read_bytes(&r.path, MAX_PUSH_FILE) {
            local.push((r.path.clone(), v));
        }
    }
    let mut plan = plan_pull(&local, &remote);
    let (all, _) = ws.all_files(MAX_PUSH_FILES * 4);
    let (ok, _) = eligible(&all);
    let remote_set: HashSet<&str> = remote.iter().map(|r| r.path.as_str()).collect();
    plan.only_local = ok.iter().filter(|r| !remote_set.contains(r.as_str())).count();
    let mut report =
        PullReport { same: plan.same, only_local: plan.only_local, skipped: plan.skipped, ..Default::default() };
    for item in plan.fetch {
        let bytes = match client.download(&format!("{}/{}?download=1", files_path(pid), encode_path(&item.path))) {
            Ok(b) => b,
            Err(e) if !fatal(&e) => {
                report.skipped.push(Skipped { rel: item.path, reason: file_reason(&e) });
                continue;
            }
            Err(e) => return Err(e),
        };
        let text = match check_download(&item, bytes) {
            Ok(t) => t,
            Err(why) => {
                report.skipped.push(Skipped { rel: item.path, reason: why });
                continue;
            }
        };
        match propose_external(ws, pending, &item.path, item.local_version.clone(), text) {
            Ok(()) => report.proposed += 1,
            Err(_) => report.skipped.push(Skipped {
                rel: item.path,
                reason: "could not be added for review (changed locally, or too many waiting)",
            }),
        }
    }
    Ok(report)
}

// ---- folder links (links.json in the app data folder)

/// Which gateway project belongs to an open folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// First 32 hex of SHA-256 of the folder's canonical path (the path itself is not stored).
    pub folder: String,
    pub gateway_id: String,
    pub project_id: String,
    pub project_name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkFile {
    links: Vec<Link>,
}

/// The key for a folder in links.json.
pub fn folder_key(root: &Path) -> String {
    version_of(root.to_string_lossy().as_bytes())[..32].to_string()
}

/// Reads links.json (missing = none).
pub fn links_load(dir: &Path) -> Result<Vec<Link>, StoreError> {
    match store::read_capped(&dir.join("links.json"), 256 << 10) {
        Ok(b) => serde_json::from_slice::<LinkFile>(&b).map(|f| f.links).map_err(|_| StoreError::Bad),
        Err(StoreError::Missing) => Ok(vec![]),
        Err(e) => Err(e),
    }
}

/// The link for `folder` on `gateway_id`, if any.
pub fn link_for(dir: &Path, folder: &str, gateway_id: &str) -> Option<Link> {
    links_load(dir).ok()?.into_iter().find(|l| l.folder == folder && l.gateway_id == gateway_id)
}

/// Saves (or with `project_id = None` removes) the link for a folder on one gateway.
pub fn link_set(dir: &Path, folder: &str, gateway_id: &str, project: Option<(&str, &str)>) -> Result<(), StoreError> {
    if !id_ok(gateway_id) || folder.len() != 32 || project.is_some_and(|(id, _)| !id_ok(id)) {
        return Err(StoreError::Bad);
    }
    let mut links = links_load(dir)?;
    links.retain(|l| !(l.folder == folder && l.gateway_id == gateway_id));
    if let Some((id, name)) = project {
        links.push(Link {
            folder: folder.into(),
            gateway_id: gateway_id.into(),
            project_id: id.into(),
            project_name: name.chars().filter(|c| !c.is_control()).take(128).collect(),
        });
    }
    if links.len() > MAX_LINKS {
        links.drain(..links.len() - MAX_LINKS);
    }
    store::ensure_dir(dir)?;
    let bytes = serde_json::to_vec_pretty(&LinkFile { links }).map_err(|_| StoreError::Bad)?;
    store::write_atomic(&dir.join("links.json"), &bytes)
}

// ---- view shapes for the webview (all from untrusted gateway JSON)

/// A gateway project in the picker.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub role: String,
    pub version: i64,
    pub file_count: i64,
}

pub fn project_view(v: &Value) -> Option<ProjectView> {
    let id = clean(v, "id", 64);
    id_ok(&id).then(|| ProjectView {
        id,
        name: clean(v, "name", 128),
        role: clean(v, "role", 16),
        version: num(v, "version"),
        file_count: num(v, "file_count"),
    })
}

/// The project's mission switches and what the gateway offers.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Switches {
    /// The gateway has the feature at all (`GET /v1/experimental`).
    pub autopilot_available: bool,
    pub improve_available: bool,
    /// The administrator's kill switch is on.
    pub disabled: bool,
    /// The project's own switches (`GET /v1/projects/{id}/experimental`).
    pub autopilot: bool,
    pub auto_improve: bool,
    pub resource_scope: String,
}

pub fn switches(exp: &Value, flags: &Value) -> Switches {
    Switches {
        autopilot_available: exp.get("autopilot").map(|a| flag(a, "available")).unwrap_or(false),
        improve_available: exp.get("auto_improve").map(|a| flag(a, "available")).unwrap_or(false),
        disabled: flag(exp, "disabled"),
        autopilot: flag(flags, "autopilot"),
        auto_improve: flag(flags, "auto_improve"),
        resource_scope: clean(flags, "resource_scope", 16),
    }
}

/// The copy on a node and the last collection of its changes.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncView {
    /// `none`, `pending`, `ready`, `failed`, `stale` (or `off` when the gateway has sync switched off).
    pub state: String,
    pub version: i64,
    /// `pending`, `committed`, `unchanged`, `conflict`, `failed`, or empty.
    pub delta_state: String,
    pub delta_version: i64,
    pub delta_added: i64,
    pub delta_modified: i64,
    pub delta_deleted: i64,
    pub delta_conflicts: Vec<String>,
}

pub fn sync_view(v: &Value) -> SyncView {
    let d = v.get("delta").cloned().unwrap_or(Value::Null);
    SyncView {
        state: clean(v, "state", 16),
        version: num(v, "version"),
        delta_state: clean(&d, "state", 16),
        delta_version: num(&d, "version"),
        delta_added: num(&d, "added"),
        delta_modified: num(&d, "modified"),
        delta_deleted: num(&d, "deleted"),
        delta_conflicts: d
            .get("conflicts")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter().take(20).filter_map(Value::as_str).filter(|p| gateway_path_ok(p)).map(String::from).collect()
            })
            .unwrap_or_default(),
    }
}

/// One mission row (and, for Autopilot, its tasks and checks).
#[derive(Debug, Clone, Serialize)]
pub struct MissionView {
    pub id: String,
    pub kind: String,
    pub project_id: String,
    pub model: String,
    pub goal: String,
    pub state: String,
    pub reason: String,
    pub cycle: i64,
    pub max_cycles: i64,
    pub tokens: i64,
    pub run_id: String,
    pub waiting_approval: bool,
    pub checks_passed: i64,
    pub checks_total: i64,
    pub tasks_done: i64,
    pub tasks_total: i64,
    pub proposals_created: i64,
    pub updated_at: i64,
    pub tasks: Vec<TaskView>,
    pub checks: Vec<CheckView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub title: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckView {
    pub id: String,
    pub passed: bool,
    pub exit_code: i64,
    pub error: String,
}

pub fn mission_view(v: &Value) -> Option<MissionView> {
    let id = clean(v, "id", 64);
    if !id_ok(&id) {
        return None;
    }
    let run_id = clean(v, "run_id", 64);
    let list = |k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    Some(MissionView {
        id,
        kind: clean(v, "kind", 16),
        project_id: clean(v, "project_id", 64),
        model: clean(v, "model", 200),
        goal: clean(v, "goal", 1000),
        state: clean(v, "state", 24),
        reason: clean(v, "reason", 300),
        cycle: num(v, "cycle"),
        max_cycles: num(v, "max_cycles"),
        tokens: num(v, "tokens"),
        run_id: if id_ok(&run_id) { run_id } else { String::new() },
        waiting_approval: flag(v, "waiting_approval"),
        checks_passed: num(v, "checks_passed"),
        checks_total: num(v, "checks_total"),
        tasks_done: num(v, "tasks_done"),
        tasks_total: num(v, "tasks_total"),
        proposals_created: num(v, "proposals_created"),
        updated_at: num(v, "updated_at"),
        tasks: list("tasks")
            .iter()
            .take(64)
            .map(|t| TaskView { title: clean(t, "title", 300), state: clean(t, "state", 16) })
            .collect(),
        checks: list("checks")
            .iter()
            .take(8)
            .map(|c| CheckView {
                id: clean(c, "id", 32),
                passed: flag(c, "passed"),
                exit_code: num(c, "exit_code"),
                error: clean(c, "error", 64),
            })
            .collect(),
    })
}

/// A write call waiting for the person's decision.
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalView {
    pub id: String,
    pub tool: String,
    pub access: String,
    /// The call's arguments as pretty JSON text (shown verbatim, never run), at most 64 KiB.
    pub arguments: String,
    pub arguments_sha256: String,
    pub expires_at: i64,
}

pub fn approval_view(v: &Value) -> Option<ApprovalView> {
    let id = clean(v, "id", 64);
    let sha = clean(v, "arguments_sha256", 64);
    if !id_ok(&id) || sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let args = v.get("arguments").map(|a| serde_json::to_string_pretty(a).unwrap_or_default()).unwrap_or_default();
    Some(ApprovalView {
        id,
        tool: clean(v, "tool", 64),
        access: clean(v, "access", 16),
        arguments: args.chars().take(64 << 10).collect(),
        arguments_sha256: sha,
        expires_at: num(v, "expires_at"),
    })
}

/// An agent's checkpoint waiting for the person's commit (git_commit mode "local", docs/features/agent-commits.md).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CheckpointView {
    pub id: String,
    pub version: i64,
    /// The message's first line.
    pub subject: String,
    /// The whole commit message (at most 4,000 bytes at the gateway; cut to 5,000 characters here).
    pub message: String,
    pub created_at: i64,
    /// pending, committed, rejected or failed.
    pub local: String,
}

/// Reads one entry of `GET /v1/projects/{id}/checkpoints`; `None` for a malformed one.
///
/// Security: the id goes into URL paths later, so it must pass `id_ok` (CWE-74); texts are cleaned (CWE-150).
pub fn checkpoint_view(v: &Value) -> Option<CheckpointView> {
    let id = clean(v, "id", 64);
    if !id_ok(&id) || v.get("kind").and_then(Value::as_str) != Some("checkpoint") {
        return None;
    }
    let local = v.get("local").map(|l| clean(l, "state", 16)).unwrap_or_default();
    Some(CheckpointView {
        id,
        version: num(v, "version"),
        subject: clean(v, "label", 200).replace('\n', " "),
        message: clean(v, "message", 5000),
        created_at: num(v, "created_at"),
        local,
    })
}

/// An Auto Improve proposal.
#[derive(Debug, Clone, Serialize)]
pub struct ProposalView {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub rationale: String,
    pub effort: String,
    pub steps: Vec<String>,
    pub sources: Vec<String>,
    pub state: String,
}

pub fn proposal_view(v: &Value) -> Option<ProposalView> {
    let id = clean(v, "id", 64);
    let strings = |k: &str, n: usize, max: usize| -> Vec<String> {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .take(n)
                    .filter_map(Value::as_str)
                    .map(|s| s.chars().filter(|c| !c.is_control()).take(max).collect())
                    .collect()
            })
            .unwrap_or_default()
    };
    id_ok(&id).then(|| ProposalView {
        id,
        title: clean(v, "title", 200),
        kind: clean(v, "kind", 16),
        rationale: clean(v, "rationale", 2000),
        effort: clean(v, "effort", 32),
        steps: strings("steps", 12, 300),
        // Sources are shown as plain text, never as links (the webview opens no URLs from gateway text).
        sources: strings("sources", 8, 300),
        state: clean(v, "state", 16),
    })
}

/// One acceptance check from the start form.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckInput {
    pub id: String,
    /// `tests` (with `framework`) or `command` (with `argv`).
    pub kind: String,
    #[serde(default)]
    pub framework: String,
    #[serde(default)]
    pub argv: Vec<String>,
}

/// Test frameworks the gateway knows (P2P-AI autopilot doc, section on acceptance).
pub const FRAMEWORKS: [&str; 6] = ["cargo", "go", "make", "npm", "pytest", "unittest"];

/// Turns the start form's checks into the gateway's `acceptance` list.
///
/// Security: argv is an array (no shell, CWE-78); the gateway checks programs against its own allow-list too.
pub fn acceptance(checks: &[CheckInput]) -> Result<Value, &'static str> {
    if checks.is_empty() || checks.len() > 8 {
        return Err("Autopilot needs 1 to 8 checks that say when the work is done");
    }
    let mut out = Vec::new();
    let mut ids = HashSet::new();
    for c in checks {
        let id_good = !c.id.is_empty()
            && c.id.len() <= 32
            && c.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if !id_good || !ids.insert(c.id.clone()) {
            return Err("check names must be unique: lower-case letters, digits, - and _");
        }
        match c.kind.as_str() {
            "tests" if FRAMEWORKS.contains(&c.framework.as_str()) => {
                out.push(serde_json::json!({"id": c.id, "kind": "tests", "framework": c.framework}))
            }
            "command"
                if !c.argv.is_empty()
                    && c.argv.len() <= 32
                    && c.argv.iter().all(|a| !a.is_empty() && a.len() <= 512 && !a.chars().any(char::is_control)) =>
            {
                out.push(serde_json::json!({"id": c.id, "kind": "command", "argv": c.argv}))
            }
            _ => return Err("each check is a test framework from the list, or a command split into words"),
        }
    }
    Ok(Value::Array(out))
}

/// The shared mission cases (core/tests/vectors/missions.json), also run by the VS Code extension (side track X3).
#[cfg(test)]
#[path = "projects_vectors.rs"]
mod vectors;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rf(p: &str, sha: &str) -> RemoteFile {
        RemoteFile { path: p.into(), bytes: 3, sha256: sha.into() }
    }

    #[test]
    fn gateway_paths_follow_the_grammar() {
        for p in ["src/main.py", "docs/my notes (1).md", "a/b-c_d.e+f@g,h=i~j.txt", ".github/workflows/ci.yml"] {
            assert!(gateway_path_ok(p), "{p}");
        }
        for p in
            ["", "a//b", "../x", "a/./b", "con.txt", "src/NUL", "lpt1 .log", "a/b.", " a", "a ", "é.md", "a\\b", "x:y"]
        {
            assert!(!gateway_path_ok(p), "{p}");
        }
        assert!(!gateway_path_ok(&"a/".repeat(40)));
    }

    #[test]
    fn push_keeps_secrets_and_odd_names_home() {
        let files = vec![
            ("src/a.py".to_string(), 10),
            (".env".to_string(), 10),
            ("keys/server.pem".to_string(), 10),
            ("src/A.py".to_string(), 10),
            ("big.bin".to_string(), MAX_PUSH_FILE + 1),
            ("aux.c".to_string(), 1),
        ];
        let (ok, skipped) = eligible(&files);
        assert_eq!(ok, vec!["src/a.py"]);
        let reasons: Vec<_> = skipped.iter().map(|s| (s.rel.as_str(), s.reason)).collect();
        assert!(reasons.contains(&(".env", "may hold secrets, kept on this computer")));
        assert!(reasons.contains(&("src/A.py", "differs from another name only by case")));
        assert!(reasons.iter().any(|(r, _)| *r == "big.bin"));
        assert!(reasons.iter().any(|(r, _)| *r == "aux.c"));
    }

    #[test]
    fn push_and_pull_plans_compare_digests() {
        let local = vec![
            ("a".to_string(), "1".repeat(64)),
            ("b".to_string(), "2".repeat(64)),
            ("c".to_string(), "3".repeat(64)),
        ];
        let remote = vec![
            rf("a", &"1".repeat(64)),
            rf("b", &"9".repeat(64)),
            rf("gone", &"4".repeat(64)),
            rf("big.bin", &"5".repeat(64)),
        ];
        let skipped = vec![Skipped { rel: "big.bin".into(), reason: "x" }];
        let p = plan_push(&local, &remote, &skipped);
        assert_eq!(p.unchanged, 1);
        assert_eq!(p.upload.iter().map(|u| u.0.as_str()).collect::<Vec<_>>(), vec!["b", "c"]);
        // A file that exists locally but was skipped is not deleted on the gateway.
        assert_eq!(p.delete, vec!["gone"]);

        let remote2 = vec![
            rf("a", &"1".repeat(64)),
            rf("b", &"9".repeat(64)),
            rf("new.py", &"7".repeat(64)),
            rf(".env", &"8".repeat(64)),
        ];
        let q = plan_pull(&local, &remote2);
        assert_eq!(q.same, 1);
        assert_eq!(q.fetch.len(), 2);
        assert_eq!(q.fetch[0].local_version.as_deref(), Some("2".repeat(64).as_str()));
        assert_eq!(q.fetch[1].local_version, None);
        // SECURITY: the gateway cannot plant .env.
        assert_eq!(q.skipped[0].rel, ".env");
        assert_eq!(q.only_local, 1);
    }

    #[test]
    fn downloads_must_match_their_digest_and_be_text() {
        let item = PullItem { path: "a.py".into(), sha256: version_of(b"print(1)\n"), local_version: None };
        assert_eq!(check_download(&item, b"print(1)\n".to_vec()).unwrap(), "print(1)\n");
        assert!(check_download(&item, b"print(2)\n".to_vec()).is_err());
        let bin = PullItem { path: "a.bin".into(), sha256: version_of(&[0xff, 0xfe]), local_version: None };
        assert!(check_download(&bin, vec![0xff, 0xfe]).is_err());
    }

    #[test]
    fn gateway_views_are_cleaned_and_capped() {
        let m = json!({"id": "msn_0123abcd", "kind": "autopilot", "goal": "fix\u{1b}[31m it", "state": "running", "run_id": "../x",
            "tasks": [{"title": "t1", "state": "done"}], "checks": [{"id": "unit", "passed": true, "exit_code": 0}], "extra": "ignored"});
        let v = mission_view(&m).unwrap();
        assert_eq!(v.goal, "fix[31m it");
        assert_eq!(v.run_id, "", "a run id that is not an id is dropped");
        assert_eq!(v.tasks.len(), 1);
        assert!(mission_view(&json!({"id": "a/b"})).is_none());
        let a = approval_view(&json!({"id": "apr_1", "tool": "write_file", "arguments": {"path": "a.py"}, "arguments_sha256": "a".repeat(64)})).unwrap();
        assert!(a.arguments.contains("\"path\": \"a.py\""));
        assert!(approval_view(&json!({"id": "apr_1", "arguments_sha256": "short"})).is_none());
        let files = parse_remote_files(
            &json!({"data": [{"path": "a.py", "bytes": 3, "sha256": "A".repeat(64)}, {"path": "../x", "sha256": "a".repeat(64)}]}),
        );
        assert_eq!(files, vec![RemoteFile { path: "a.py".into(), bytes: 3, sha256: "a".repeat(64) }]);
    }

    #[test]
    fn acceptance_checks_are_strict() {
        let ok = acceptance(&[
            CheckInput { id: "unit".into(), kind: "tests".into(), framework: "pytest".into(), argv: vec![] },
            CheckInput {
                id: "lint".into(),
                kind: "command".into(),
                framework: String::new(),
                argv: vec!["ruff".into(), "check".into()],
            },
        ])
        .unwrap();
        assert_eq!(ok[1]["argv"][0], "ruff");
        assert!(acceptance(&[]).is_err());
        assert!(acceptance(&[CheckInput {
            id: "x".into(),
            kind: "tests".into(),
            framework: "bash".into(),
            argv: vec![]
        }])
        .is_err());
        assert!(acceptance(&[CheckInput {
            id: "X Y".into(),
            kind: "tests".into(),
            framework: "go".into(),
            argv: vec![]
        }])
        .is_err());
    }

    #[test]
    fn links_are_saved_per_folder_and_gateway() {
        let d = std::env::temp_dir().join(format!("p2pide-links-{}", store::new_id()));
        let f = folder_key(Path::new("/home/x/proj"));
        assert_eq!(f.len(), 32);
        link_set(&d, &f, "gw1", Some(("prj_1", "Demo"))).unwrap();
        link_set(&d, &f, "gw2", Some(("prj_2", "Other"))).unwrap();
        assert_eq!(link_for(&d, &f, "gw1").unwrap().project_id, "prj_1");
        link_set(&d, &f, "gw1", None).unwrap();
        assert!(link_for(&d, &f, "gw1").is_none());
        assert!(link_for(&d, &f, "gw2").is_some());
        assert!(link_set(&d, &f, "gw1", Some(("../p", "x"))).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn checkpoints_are_read_and_checked() {
        let v = serde_json::json!({"id": "0123456789abcdef0123456789abcdef", "kind": "checkpoint", "version": 3,
            "label": "Fix\u{1b}[2J it", "message": "Fix it\n\nwhy", "created_at": 5, "local": {"state": "pending"}});
        let c = checkpoint_view(&v).unwrap();
        assert_eq!(
            (c.version, c.subject.as_str(), c.message.as_str(), c.local.as_str()),
            (3, "Fix[2J it", "Fix it\n\nwhy", "pending")
        );
        // SECURITY: an id that could change the URL path, or a plain snapshot, is dropped (CWE-74).
        assert!(checkpoint_view(&serde_json::json!({"id": "../x", "kind": "checkpoint"})).is_none());
        assert!(checkpoint_view(&serde_json::json!({"id": "abc", "kind": "snapshot"})).is_none());
    }
}
