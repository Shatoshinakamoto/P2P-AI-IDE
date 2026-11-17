# `cargo run --example local_containers`: live check of the container runner, and the engine commands it uses

Development test only; not part of the app. Added 2026-11-17 for phase I5d
([features/container-runner.md](../features/container-runner.md)), to prove that real Docker or Podman runs are
confined and that each framework's image can build and test a small project.

## Purpose

`core/src/container.rs` tests the argument list. This example:

1. With `--prepare`, gets every image that is missing (pull by digest, or build the Rust image), as **Get image** does.
2. Checks the confinement with the Python image: the copy is readable and writable in `/work`, `.env` is not copied,
   the image's file system is read-only, there is no network, the real folder is untouched, the user is the
   person's own uid on Linux (not root elsewhere), and the time limit kills the container.
3. Runs one tiny project per framework, each with one passing and one failing test, through `container::run`:

| Framework | Image | Expected |
|---|---|---|
| cargo | `rust` | verdict fail, 1 passed, 1 failed, the failing test named |
| go | `go` | as above |
| npm (`node --test`) | `node` | as above |
| make | `go` | verdict fail |
| unittest | `python` | as above |

## Synopsis

```
cd core && cargo run --example local_containers [-- --prepare]
```

| Option | Type | Default | What it does |
|---|---|---|---|
| `--prepare` | flag | off | downloads the missing pinned images and builds the Rust image before the checks (needs network); without it a missing image makes its checks `SKIP` |

No environment variables read. No root or admin needed: rootless Podman, or a Docker the person can already use
(on Linux, membership of the `docker` group is root-equivalent; prefer rootless Podman).

## Output and exit codes

`INFO` lines (engine, image downloads), one `PASS name`, `FAIL name: reason` or `SKIP name: reason` line per check,
then `{"passed": n, "failed": n, "skipped": n}`. Exit code 0 only when nothing failed; 1 otherwise. If no engine
answers, one `SKIP` and exit 0.

## Engine commands the runner uses

The IDE calls the engine with argument lists (no shell). Official references:
[podman run](https://docs.podman.io/en/latest/markdown/podman-run.1.html),
[docker run](https://docs.docker.com/reference/cli/docker/container/run/),
[podman pull](https://docs.podman.io/en/latest/markdown/podman-pull.1.html),
[podman build](https://docs.podman.io/en/latest/markdown/podman-build.1.html).

| Command | When | Security note |
|---|---|---|
| `<engine> version` | finding the engine (15 s limit) | read only |
| `<engine> image inspect <ref>` | before a run and in the Image list | read only |
| `<engine> pull <ref@sha256:...>` | **Get image** only | pinned digest, so a swapped tag cannot change what runs (CWE-494) |
| `<engine> build --pull=missing -t localhost/p2p-ide-rust:... -f <Containerfile> <empty folder>` | **Get image** for Rust | empty build context: no project file reaches the build |
| `<engine> run ...` (below) | each run | locked-down flags |
| `<engine> kill p2pide-run-<16 hex>` | only at the time limit | kills the container, not just the client |

Flags of `run`, every one fixed by the IDE:

| Flag | Value | Effect and security |
|---|---|---|
| `--rm` | | container removed after the run |
| `--name` | `p2pide-run-<16 hex>` | random name so the time limit can kill exactly this container |
| `--pull` | `never` | no download during a run (SLSA, CWE-494) |
| `--network` | `none` | no network, loopback only inside (NIST SC-7) |
| `--cap-drop` | `ALL` | no Linux capabilities (CIS Docker 5.3) |
| `--security-opt` | `no-new-privileges` | setuid files cannot raise privileges (CIS 5.25) |
| `--read-only` | | image file system read-only (CIS 5.12) |
| `--tmpfs` | `/tmp:rw,nosuid,nodev,size=512m` | small writable `/tmp`; the engine adds `noexec` |
| `--pids-limit` | `1024` | fork bombs stopped (CIS 5.28) |
| `--memory`, `--memory-swap` | `8g`, `8g` | memory cap with no extra swap (CIS 5.10) |
| `--ulimit` | `nofile=4096:4096`, `core=0` | open files capped, no core dumps |
| `--user` | Linux: own `uid:gid`; else `1000:1000` | never root (CWE-250) |
| `--userns` | `keep-id` (Podman on Linux) | the person's uid maps to itself, so files stay theirs |
| `--volume` | job folder `:/work:rw`, cache `:/cache:rw`, `,Z` on SELinux | only IDE-owned folders; `:Z` relabels only them (SELinux stays enforcing) |
| `--workdir` | `/work/<folder in the project>` | |
| `--env` | fixed list (HOME, TMPDIR, GOTMPDIR, CI, caches, offline settings) | none of the person's environment is passed (CWE-526) |

**Never used, and dangerous:** `--privileged`, `--cap-add`, `--network=host`, `--pid=host`, `--ipc=host`,
`--device`, a mount of `/var/run/docker.sock` or of the person's home, `--security-opt seccomp=unconfined` or
`label=disable`. A unit test checks that none appear.

## Examples

```
cd core && cargo run --example local_containers                  # images already present
cd core && cargo run --example local_containers -- --prepare     # first time: get the images
cd core && cargo run --example local_containers 2>/dev/null | tail -1   # summary line only
podman image ls --digests | grep -E 'ubi9|p2p-ide'               # what was downloaded or built
```
