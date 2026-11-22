//! container.rs -- the local container runner: Docker or Podman on this computer (phase I5d,
//! docs/features/container-runner.md).
//!
//! Purpose: run the project's tests or one program in a throw-away container, with the same request and result as
//! the gateway runner ([`crate::runner`]) and the local OS sandbox ([`crate::localbox`]). Requested by Pheonix on
//! 2026-11-17: "include support for local container like docker or podman".
//!
//! How a run works:
//!   1. The engine is found on PATH: **Podman first** (rootless by default), then Docker.
//!   2. A **snapshot** of the folder (the agent deny list applies; dependency folders are copied, not linked) goes
//!      into a fresh job folder; the per-folder build cache is the same as the local sandbox's.
//!   3. `<engine> run` with an argument list (no shell): `--network=none`, `--cap-drop=ALL`,
//!      `--security-opt=no-new-privileges`, `--read-only` root with a small `/tmp` tmpfs, process, memory, file and
//!      core limits, a non-root user (the person's own uid on Linux, `--userns=keep-id` with Podman), `--pull=never`,
//!      the job folder at `/work` and the cache at `/cache` (`:Z` on SELinux hosts, which relabels only these two
//!      IDE-owned folders). The image is one of a fixed list, pinned by digest.
//!   4. At the time limit the container is killed by name; the job folder is removed.
//!
//! Images (Pheonix's 2026-11-17 card, recommended option: Red Hat UBI per framework): UBI 9 Python 3.12, Node.js
//! 22 and Go toolset images pulled by digest; Rust has no UBI image, so it is built here from the pinned UBI 9 base
//! with `rust-toolset` from the UBI repositories. Getting an image (pull or build) is a separate step the person
//! starts; runs never download anything.
//!
//! Inputs and trust boundaries: the request was checked by `runner::check`; the image id comes from a fixed list;
//! the project's code runs only inside the container; its output is untrusted text (cleaned, capped).
//!
//! Security controls: no network (NIST SC-7); no capabilities, no privilege gain, non-root, read-only root file
//! system (CIS Docker 5.3, 5.12, 5.25; NIST AC-6; CWE-250, CWE-269); pids, memory, file and core limits (CIS 5.10,
//! 5.28; CWE-400, CWE-770); images pinned by digest from a fixed list (SLSA, NIST SR-3, CWE-494); argument lists only,
//! no shell (CWE-78, CWE-88); secrets never copied (CWE-200); the engine binary is taken from absolute PATH folders
//! outside the open folder (CWE-426). Fail closed: the runner is offered only when the engine answers.
//!
//! Accepted risks: the container shares the host kernel (a kernel or engine escape is out of scope; rootless Podman
//! limits the damage); with rootful Docker the daemon runs as root, so the IDE never passes privileged flags, host
//! namespaces, devices or any mount other than the two IDE folders; the Rust image is built from UBI packages that
//! are not pinned one by one.
//!
//! Dependencies: the Docker or Podman command line (installed by the person); the core's workspace, runner,
//! testparse and localbox (snapshot, cache, output helpers) modules. No new crates.

use crate::localbox::{cache_dir, clean, read_capped, snapshot, JobDir};
use crate::runner::{RunRequest, RunResult};
use crate::workspace::Workspace;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The runner's id in the view and the shell.
pub const RUNNER: &str = "container";

/// Pinned UBI 9 base for the built Rust image (the Containerfile below must name exactly this; checked by a test).
#[cfg(test)]
const UBI9_BASE: &str =
    "registry.access.redhat.com/ubi9/ubi@sha256:1e9f0ed9fceafb40878bc1705ff124aa150e3fed873f18c42cdc2dded319e0a8";

/// One image the runner can use.
pub struct Image {
    /// Fixed id used by the view and the shell.
    pub id: &'static str,
    /// What the person sees.
    pub label: &'static str,
    /// The reference given to `run` (a digest for pulled images, a local tag for built ones).
    pub reference: &'static str,
    /// For a built image: the Containerfile (from pinned bases only).
    pub build: Option<&'static str>,
}

/// The fixed image list (Red Hat UBI 9, pinned by digest on 2026-11-17).
pub const IMAGES: [Image; 4] = [
    Image {
        id: "python",
        label: "Python 3.12 (UBI 9)",
        reference: "registry.access.redhat.com/ubi9/python-312@sha256:5b184c2acfe9e2eb9df1a243da8e4a82fedd96e5e7f6648ec5512656177fa326",
        build: None,
    },
    Image {
        id: "node",
        label: "Node.js 22 (UBI 9)",
        reference: "registry.access.redhat.com/ubi9/nodejs-22@sha256:623c4bda86a58190bb410ff85b7b3f8eaf16d8ead17d57388b9db1b6672c1742",
        build: None,
    },
    Image {
        id: "go",
        label: "Go toolset, with make and gcc (UBI 9)",
        reference: "registry.access.redhat.com/ubi9/go-toolset@sha256:4053e919374d739ee6b0007e412d89256b0bd065e0bc690c2b3905bbaab6500e",
        build: None,
    },
    Image {
        id: "rust",
        label: "Rust toolset (built here from UBI 9)",
        reference: "localhost/p2p-ide-rust:ubi9-1e9f0ed9",
        build: Some(
            "# Built by the P2P-AI IDE (phase I5d): UBI 9 base pinned by digest, Rust from the UBI repositories.\n\
             FROM registry.access.redhat.com/ubi9/ubi@sha256:1e9f0ed9fceafb40878bc1705ff124aa150e3fed873f18c42cdc2dded319e0a8\n\
             RUN dnf -y --setopt=install_weak_deps=0 install rust-toolset make && dnf clean all\n\
             USER 1001\n",
        ),
    },
];

/// The image for a request: a test framework picks its own; a command uses the one the person chose.
///
/// Errors: an unknown image id (deny by default).
pub fn image_for(req: &RunRequest, chosen: &str) -> Result<&'static Image, String> {
    let id = if req.tool == "run_tests" {
        match req.framework.as_str() {
            "pytest" | "unittest" => "python",
            "npm" => "node",
            "go" | "make" => "go",
            "cargo" => "rust",
            _ => return Err("unknown test framework".into()),
        }
    } else {
        chosen
    };
    IMAGES.iter().find(|i| i.id == id).ok_or_else(|| "pick an image from the list".to_string())
}

/// A container engine on this computer.
#[derive(Debug, Clone)]
pub struct Engine {
    pub program: PathBuf,
    /// `podman` or `docker`.
    pub kind: &'static str,
}

/// Finds Podman, then Docker, in absolute PATH folders.
///
/// Security: never a relative folder, so a program planted in the open folder is not picked up (CWE-426).
pub fn engine() -> Option<Engine> {
    let path = std::env::var_os("PATH")?;
    for kind in ["podman", "docker"] {
        let name = if cfg!(windows) { format!("{kind}.exe") } else { kind.to_string() };
        for d in std::env::split_paths(&path) {
            let p = d.join(&name);
            if d.is_absolute() && p.is_file() {
                return Some(Engine { program: p, kind });
            }
        }
    }
    None
}

/// On Windows, starts the engine's console program without a console window (CREATE_NO_WINDOW), so no window
/// flashes up from the windowed app; elsewhere does nothing.
fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Runs the engine with `args`, no input, output discarded; true on exit code 0 within `limit`.
fn quiet(e: &Engine, args: &[&str], limit: Duration) -> bool {
    let mut cmd = Command::new(&e.program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    no_window(&mut cmd);
    let Ok(mut c) = cmd.spawn() else {
        return false;
    };
    let t = Instant::now();
    loop {
        match c.try_wait() {
            Ok(Some(s)) => return s.success(),
            Ok(None) if t.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = c.kill();
                let _ = c.wait();
                return false;
            }
        }
    }
}

/// Whether the container runner can be used now: an engine is installed and answers.
pub fn available() -> crate::localbox::Availability {
    use crate::localbox::Availability;
    match engine() {
        None => Availability { available: false, reason: "install Podman or Docker to use containers".into() },
        Some(e) if quiet(&e, &["version"], Duration::from_secs(15)) => {
            Availability { available: true, reason: String::new() }
        }
        Some(e) => Availability {
            available: false,
            reason: if e.kind == "docker" {
                "Docker is installed but not running; start Docker Desktop or the Docker service".into()
            } else {
                "Podman is installed but does not answer; on Windows or macOS start the Podman machine".into()
            },
        },
    }
}

/// One image as the view shows it.
#[derive(Debug, Clone, Serialize)]
pub struct ImageView {
    pub id: &'static str,
    pub label: &'static str,
    /// On this computer already.
    pub present: bool,
    /// `pull` or `build`: what getting it does.
    pub get: &'static str,
}

/// The image list with what is already on this computer.
pub fn images() -> Vec<ImageView> {
    let e = engine();
    IMAGES
        .iter()
        .map(|i| ImageView {
            id: i.id,
            label: i.label,
            present: e.as_ref().is_some_and(|e| present(e, i)),
            get: if i.build.is_some() { "build" } else { "pull" },
        })
        .collect()
}

/// True when the image is on this computer.
fn present(e: &Engine, i: &Image) -> bool {
    quiet(e, &["image", "inspect", i.reference], Duration::from_secs(20))
}

/// Gets an image: pulls it by digest, or builds the Rust image from its pinned base. Downloads happen only here,
/// when the person presses **Get image** (never during a run).
///
/// Errors: fixed text; the engine's own output is not shown (it can hold registry or proxy details).
pub fn prepare(id: &str, stage: &mut dyn FnMut(&'static str)) -> Result<(), String> {
    let e = engine().ok_or("install Podman or Docker to use containers")?;
    let i = IMAGES.iter().find(|i| i.id == id).ok_or("pick an image from the list")?;
    let limit = Duration::from_secs(1800);
    match i.build {
        None => {
            stage("pulling");
            if !quiet(&e, &["pull", i.reference], limit) {
                return Err("the image could not be downloaded; check the network and try again".into());
            }
        }
        Some(file) => {
            stage("building");
            // An empty build context: the Containerfile needs no files, so nothing of the person's is sent to the
            // engine (CWE-200).
            let ctx = std::env::temp_dir().join(format!("p2pide-build-{}", crate::store::random_id()));
            std::fs::create_dir(&ctx).map_err(|_| "could not prepare the image build")?;
            let cf = ctx.join("Containerfile");
            std::fs::write(&cf, file).map_err(|_| "could not prepare the image build")?;
            let cfs = cf.to_string_lossy().to_string();
            let cxs = ctx.to_string_lossy().to_string();
            let ok = quiet(&e, &["build", "--pull=missing", "-t", i.reference, "-f", &cfs, &cxs], limit)
                || quiet(&e, &["build", "-t", i.reference, "-f", &cfs, &cxs], limit);
            let _ = std::fs::remove_dir_all(&ctx);
            if !ok {
                return Err("the image could not be built; check the network and try again".into());
            }
        }
    }
    if present(&e, i) {
        Ok(())
    } else {
        Err("the image is still missing after getting it".into())
    }
}

/// The `-v` suffix for this host: `:Z` relabels the folder for the container on SELinux hosts (only the IDE's own
/// job and cache folders are ever mounted, so no file of the person's is relabelled).
fn selinux_suffix() -> &'static str {
    if cfg!(target_os = "linux") && Path::new("/sys/fs/selinux/enforce").exists() {
        ",Z"
    } else {
        ""
    }
}

/// The host side of a `--volume` mount. On Windows the `\\?\` verbatim prefix is removed, since Docker Desktop
/// reads `\\?\D:\...` as a named volume or rejects it; the path itself is the IDE's own data folder.
fn host_path(p: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        crate::localbox::plain(p.to_path_buf())
    }
    #[cfg(not(windows))]
    {
        p.to_path_buf()
    }
}

/// The user the container runs as, and any engine flag it needs.
///
/// Linux: the person's own uid and gid, so files in `/work` belong to them (Podman adds `--userns=keep-id` so the
/// uid maps to itself in rootless mode). Elsewhere (Docker Desktop, Podman machine): uid 1000, never root.
fn user_flags(e: &Engine) -> Vec<String> {
    #[cfg(unix)]
    {
        // SAFETY: getuid and getgid cannot fail.
        let (u, g) = unsafe { (libc::getuid(), libc::getgid()) };
        if cfg!(target_os = "linux") {
            let mut v = vec![format!("--user={u}:{g}")];
            if e.kind == "podman" {
                v.insert(0, "--userns=keep-id".into());
            }
            return v;
        }
    }
    let _ = e;
    vec!["--user=1000:1000".into()]
}

/// The full argument list for `<engine> run` (pure, so it is unit-tested).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_args(
    name: &str,
    user: &[String],
    job: &Path,
    cache: &Path,
    cwd: &str,
    env: &[(String, String)],
    image: &str,
    argv: &[String],
    mount_suffix: &str,
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "run".into(),
        "--rm".into(),
        format!("--name={name}"),
        // SECURITY: never download during a run; images come only from **Get image** (CWE-494).
        "--pull=never".into(),
        // SECURITY: no network at all (NIST SC-7, CIS Docker 5.31 spirit).
        "--network=none".into(),
        // SECURITY: no capabilities and no privilege gain through setuid files (CIS 5.3, 5.25; CWE-250).
        "--cap-drop=ALL".into(),
        "--security-opt=no-new-privileges".into(),
        // SECURITY: the image cannot be changed; writes go to /work, /cache and a small /tmp (CIS 5.12).
        "--read-only".into(),
        "--tmpfs=/tmp:rw,nosuid,nodev,size=512m".into(),
        // SECURITY: resource limits (CIS 5.10, 5.28; CWE-400, CWE-770).
        "--pids-limit=1024".into(),
        "--memory=8g".into(),
        "--memory-swap=8g".into(),
        "--ulimit=nofile=4096:4096".into(),
        "--ulimit=core=0".into(),
    ];
    a.extend(user.iter().cloned());
    // SECURITY: only the IDE's job folder and the per-folder cache are mounted; no host paths, sockets or devices.
    a.push(format!("--volume={}:/work:rw{mount_suffix}", host_path(job).display()));
    a.push(format!("--volume={}:/cache:rw{mount_suffix}", host_path(cache).display()));
    a.push(format!("--workdir=/work{}", if cwd.is_empty() { String::new() } else { format!("/{cwd}") }));
    for (k, v) in env {
        a.push(format!("--env={k}={v}"));
    }
    a.push(image.into());
    a.extend(argv.iter().cloned());
    a
}

/// Runs a checked request in a container.
///
/// * `image_id` -- for commands, the image the person chose (`python`, `node`, `go`, `rust`); tests pick their own.
/// * `data_dir` -- the IDE's data folder (job folders and the per-folder cache).
/// * `stage` -- called with `copying` and `running`.
///
/// Errors are fixed messages. The result's `place` is `a container on this computer`.
pub fn run(
    ws: &Workspace,
    req: &RunRequest,
    image_id: &str,
    data_dir: &Path,
    stage: &mut dyn FnMut(&'static str),
) -> Result<RunResult, String> {
    let e = engine().ok_or("install Podman or Docker to use containers")?;
    let image = image_for(req, image_id)?;
    if !present(&e, image) {
        return Err(format!("the {} image is not on this computer yet; press Get image first", image.label));
    }
    stage("copying");
    let cache = cache_dir(data_dir, ws)?;
    let jobs = data_dir.join("run-jobs");
    std::fs::create_dir_all(&jobs).map_err(|_| "could not prepare the run folder")?;
    let id = crate::store::random_id();
    let job = JobDir { path: jobs.join(&id) };
    // SECURITY: a new folder (create_dir fails if it exists) (CWE-377).
    std::fs::create_dir(&job.path).map_err(|_| "could not prepare the run folder")?;
    snapshot(ws, &job.path, false)?;
    for d in [".p2pide-home", ".p2pide-tmp"] {
        std::fs::create_dir(job.path.join(d)).map_err(|_| "could not prepare the run folder")?;
    }
    if !req.cwd.is_empty() && !job.path.join(&req.cwd).is_dir() {
        return Err("the folder to run in is not in the copy".into());
    }

    let (argv, extra_env): (Vec<String>, Vec<(&str, &str)>) = if req.tool == "run_tests" {
        let (a, e) = crate::testparse::framework_command(&req.framework).ok_or("unknown test framework")?;
        (a.iter().map(|s| s.to_string()).collect(), e)
    } else {
        (req.argv.clone(), vec![])
    };
    // SECURITY: a program name only; the image decides what exists (CWE-426).
    if argv[0].contains(['/', '\\']) {
        return Err("give the program by name (it is looked up on the image's PATH), not by path".into());
    }
    let mut env: Vec<(String, String)> = [
        ("HOME", "/work/.p2pide-home"),
        ("TMPDIR", "/tmp"),
        ("CI", "true"),
        ("CARGO_TARGET_DIR", "/cache/target"),
        ("GOCACHE", "/cache/go-build"),
        // /tmp is mounted without execute; Go builds and runs its test programs in GOTMPDIR, so that is the job
        // folder (already writable and executable for the job).
        ("GOTMPDIR", "/work/.p2pide-tmp"),
        ("GOPATH", "/cache/gopath"),
        ("GOFLAGS", "-mod=mod"),
        ("npm_config_cache", "/cache/npm"),
        ("XDG_CACHE_HOME", "/cache/xdg"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    env.extend(extra_env.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let name = format!("p2pide-run-{}", &id[..16]);
    let args =
        run_args(&name, &user_flags(&e), &job.path, &cache, &req.cwd, &env, image.reference, &argv, selinux_suffix());

    stage("running");
    let t = Instant::now();
    let mut cmd = Command::new(&e.program);
    cmd.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|_| "the container engine could not be started")?;
    let out = child.stdout.take().map(|o| std::thread::spawn(move || read_capped(o)));
    let err = child.stderr.take().map(|o| std::thread::spawn(move || read_capped(o)));
    let limit = Duration::from_secs(u64::from(req.timeout_s));
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if t.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                timed_out = true;
                // SECURITY: the container itself is killed (killing only the client would leave it running).
                quiet(&e, &["kill", &name], Duration::from_secs(30));
                let _ = child.kill();
                break child.wait().ok();
            }
            Err(_) => break None,
        }
    };
    let (so, c1) = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let (se, c2) = err.and_then(|h| h.join().ok()).unwrap_or_default();
    let exit_code = status.and_then(|s| s.code()).map(i64::from);
    let stdout = clean(&so);
    let stderr = clean(&se);
    let tests = if req.tool == "run_tests" { crate::testparse::parse(&req.framework, &stdout, &stderr) } else { None };
    // 125: the engine could not start the container; 126/127: the program cannot run or is not in the image.
    let error = match exit_code {
        Some(125) if !timed_out => "the container could not be started".into(),
        Some(127) if !timed_out => "the program is not in this image".into(),
        _ => String::new(),
    };
    Ok(RunResult {
        runner: RUNNER.into(),
        place: "a container on this computer".into(),
        summary: format!("{} · {}", req.summary(), image.label),
        exit_code,
        timed_out,
        duration_ms: t.elapsed().as_millis() as i64,
        stdout,
        stderr,
        truncated: c1 || c2,
        tests,
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{check, RunInput};

    #[test]
    fn run_arguments_are_locked_down() {
        let a = run_args(
            "p2pide-run-x",
            &["--user=1000:1000".into()],
            Path::new("/data/run-jobs/x"),
            Path::new("/data/run-cache/k"),
            "sub",
            &[("CI".into(), "true".into())],
            IMAGES[0].reference,
            &["python3".into(), "-c".into(), "print(1); $(rm -rf /)".into()],
            ",Z",
        );
        for must in [
            "--network=none",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--read-only",
            "--pull=never",
            "--pids-limit=1024",
            "--user=1000:1000",
            "--volume=/data/run-jobs/x:/work:rw,Z",
            "--volume=/data/run-cache/k:/cache:rw,Z",
            "--workdir=/work/sub",
        ] {
            assert!(a.iter().any(|x| x == must), "{must} missing in {a:?}");
        }
        for never in [
            "--privileged",
            "--cap-add",
            "--network=host",
            "--pid=host",
            "--ipc=host",
            "--device",
            "docker.sock",
            "podman.sock",
            "unconfined",
            "label=disable",
        ] {
            assert!(!a.iter().any(|x| x.contains(never)), "{never} in {a:?}");
        }
        // The program and its arguments come last, as separate words, after the image.
        let img = a.iter().position(|x| x == IMAGES[0].reference).unwrap();
        assert_eq!(&a[img + 1..], ["python3", "-c", "print(1); $(rm -rf /)"]);
        // Every image is pinned: a digest, or a local tag built from a pinned base.
        for i in IMAGES {
            match i.build {
                None => assert!(i.reference.contains("@sha256:") && i.reference.len() > 80, "{}", i.id),
                Some(f) => assert!(i.reference.starts_with("localhost/") && f.contains(UBI9_BASE), "{}", i.id),
            }
        }
    }

    #[test]
    fn images_follow_the_framework_and_unknown_ones_are_refused() {
        let d = std::env::temp_dir().join(format!("p2pide-ct-{}", crate::store::random_id()));
        std::fs::create_dir_all(&d).unwrap();
        let ws = Workspace::open(&d).unwrap();
        let t = |fw: &str| {
            check(&ws, &RunInput { kind: "tests".into(), framework: fw.into(), ..Default::default() }).unwrap()
        };
        assert_eq!(image_for(&t("cargo"), "python").unwrap().id, "rust");
        assert_eq!(image_for(&t("npm"), "python").unwrap().id, "node");
        assert_eq!(image_for(&t("make"), "").unwrap().id, "go");
        assert_eq!(image_for(&t("unittest"), "").unwrap().id, "python");
        let c =
            check(&ws, &RunInput { kind: "command".into(), argv: vec!["ls".into()], ..Default::default() }).unwrap();
        assert_eq!(image_for(&c, "node").unwrap().id, "node");
        assert!(image_for(&c, "ubuntu:latest").is_err());
        assert!(image_for(&c, "").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
