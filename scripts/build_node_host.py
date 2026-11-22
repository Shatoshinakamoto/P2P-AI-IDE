#!/usr/bin/env python3
"""scripts/build_node_host.py -- builds the Node extension host server from VS Code's MIT source (BUILD TOOLING).

Purpose: the IDE's editor library (@codingame/monaco-vscode-api) is built on one exact VS Code version, and Node
extensions only work with a VS Code server ("REH") of the same minor version (docs/features/node-extensions-design.md
section 7). Published VSCodium servers rarely line up with the editor library's version, so Pheonix chose
(2026-11-22) to build our own server from the same VS Code source tag. This script does that build on the machine it
runs on (Linux, macOS or Windows; the server contains native modules, so each OS builds its own) and writes:

  <out>/p2pide-node-host-<platform>-<arch>-<tag>.tar.gz      the server, one top folder with node + out/server-main.js
  <out>/p2pide-node-host-<platform>-<arch>-<tag>.tar.gz.sha256
  <out>/p2pide-node-host-<platform>-<arch>-<tag>.json         build record (tag, commit, Node, OS, time, SHA-256)

The archive's layout is the one core/src/node_host.rs (`server_root`, `Launcher::for_root`) expects.

Inputs / trust boundary: the VS Code tag and its expected commit (from the command line, normally the values in
`core/src/node_host.rs`), the network (GitHub for the source, npm for dependencies, nodejs.org for the server's
Node), and the build machine. Output: the files above. Nothing here is part of the shipped app.

Security controls:
- Source pinned: the clone must be at exactly the expected 40-hex commit or the build stops (SSDF PS.2, CWE-494).
- Toolchain pinned: Node must be the version in the tag's .nvmrc (VS Code's own rule), so the build is repeatable.
- No shell: every command is an argument list (CWE-78); tag, platform and arch are allow-listed (CWE-20).
- Dependencies come from the tag's package-lock.json through `npm ci` (lockfile integrity hashes, SLSA build L1).
- Reproducible-ish archive: sorted entries, fixed owner and mtime, no links followed outside the tree (CWE-59).
- The SHA-256 written here is what gets pinned in the app; the app refuses any other bytes (NIST SI-7).
Dependencies: Python 3.10+ standard library, git, Node/npm of the tag's version, and the native build tools VS Code
needs (Linux: gcc, make, python3, libkrb5-dev, libx11-dev, libxkbfile-dev, libsecret-1-dev; Windows: Visual Studio
Build Tools with the C++ workload, its Spectre-mitigated libraries and the Windows SDK (signtool.exe, found
automatically); macOS: Xcode command line tools).

Usage: python3 scripts/build_node_host.py --tag 1.145.2 --commit c4453f8ea430795f941c1b2ff6c9321135a74a02 --out dist-node-host
See docs/commands/build-node-host.md.
"""
from __future__ import annotations

import argparse
import datetime as dt
import gzip
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

REPO = "https://github.com/microsoft/vscode.git"
# The platform and arch names VS Code's gulp tasks use (`vscode-reh-<platform>-<arch>-min`).
PLATFORMS = {"linux": "linux", "darwin": "darwin", "win32": "win32"}
ARCHES = {"x86_64": "x64", "amd64": "x64", "x64": "x64", "aarch64": "arm64", "arm64": "arm64"}


def host_platform() -> str:
    """VS Code's name for this OS: linux, darwin or win32 (SECURITY: allow-list, unknown OS stops the build)."""
    key = "win32" if sys.platform.startswith("win") else sys.platform
    if key not in PLATFORMS:
        raise SystemExit(f"unsupported OS: {sys.platform}")
    return PLATFORMS[key]


def host_arch() -> str:
    """VS Code's name for this CPU: x64 or arm64."""
    m = platform.machine().lower()
    if m not in ARCHES:
        raise SystemExit(f"unsupported CPU: {m}")
    return ARCHES[m]


def run(argv: list[str], cwd: Path, env: dict[str, str] | None = None) -> str:
    """Runs one command (no shell, CWE-78), echoes it, fails the build on a non-zero exit. Returns stdout."""
    print("+", " ".join(argv), flush=True)
    # On Windows npm is npm.cmd; shutil.which finds it without a shell.
    exe = shutil.which(argv[0]) or argv[0]
    r = subprocess.run([exe, *argv[1:]], cwd=cwd, env=env, check=False, stdout=subprocess.PIPE, text=True)
    sys.stdout.write(r.stdout)
    if r.returncode != 0:
        raise SystemExit(f"command failed ({r.returncode}): {' '.join(argv)}")
    return r.stdout


def check_node(src: Path) -> str:
    """The running Node must have the .nvmrc version's major and be at least that version (VS Code's own rule)."""
    want = (src / ".nvmrc").read_text(encoding="utf-8").strip()
    have = run(["node", "--version"], src).strip().lstrip("v")
    w = [int(x) for x in want.split(".")]
    h = [int(x) for x in have.split(".")]
    if h[0] != w[0] or h < w:
        raise SystemExit(f"Node {want} (same major, not older) is needed for this VS Code tag; this is {have}")
    return have


def archive(folder: Path, dest: Path, epoch: int) -> None:
    """Writes `folder` as dest (.tar.gz): sorted entries, owner 0, fixed mtime, gzip mtime 0 (repeatable bytes)."""
    def norm(ti: tarfile.TarInfo) -> tarfile.TarInfo:
        ti.uid = ti.gid = 0
        ti.uname = ti.gname = ""
        ti.mtime = epoch
        return ti

    with open(dest, "wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, compresslevel=9) as gz:
        with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as tar:
            paths = sorted(folder.rglob("*"), key=lambda p: p.relative_to(folder).as_posix())
            tar.add(folder, arcname=folder.name, recursive=False, filter=norm)
            for p in paths:
                # SECURITY: links are stored as links, never followed (CWE-59); the app's unpacker refuses them anyway.
                tar.add(p, arcname=f"{folder.name}/{p.relative_to(folder).as_posix()}", recursive=False, filter=norm)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def windows_sdk_tools() -> Path | None:
    """Find the newest Windows SDK x64 tools folder (it holds `signtool.exe`), or None.

    VS Code's Windows server build calls `signtool.exe verify` and `signtool remove /s` by bare name to strip
    invalidated signatures before it stamps version info on native modules. The SDK is installed with Visual Studio's
    C++ workload but isn't on PATH outside a developer prompt.

    Security: only the fixed SDK location under Program Files (x86) is searched, which only an admin can write
    (CWE-427, uncontrolled search path); signtool is used only to check and strip signatures, never to sign.
    """
    if os.name != "nt":
        return None
    # SECURITY: a fixed path, not read from the environment (CWE-427).
    base = Path(r"C:\Program Files (x86)\Windows Kits\10\bin")

    def ver(p: Path) -> tuple[int, ...]:
        try:
            return tuple(int(x) for x in p.parent.parent.name.split("."))
        except ValueError:
            return ()

    found = sorted(base.glob("10.*/x64/signtool.exe"), key=ver)
    return found[-1].parent if found else None


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--tag", required=True, help="VS Code release tag, e.g. 1.145.2")
    ap.add_argument("--commit", required=True, help="the tag's expected 40-hex commit (the build stops on any other)")
    ap.add_argument("--out", required=True, type=Path, help="folder for the archive, its .sha256 and the build record")
    ap.add_argument("--work", type=Path, default=None, help="build folder (default: <out>/work); reused if present")
    ap.add_argument("--arch", choices=["x64", "arm64"], default=None, help="target CPU (default: this machine's)")
    a = ap.parse_args()

    # SECURITY: allow-listed formats only (CWE-20); these values end up in paths and git arguments.
    if not re.fullmatch(r"\d{1,3}\.\d{1,4}\.\d{1,4}", a.tag):
        raise SystemExit("--tag must look like 1.145.2")
    if not re.fullmatch(r"[0-9a-f]{40}", a.commit):
        raise SystemExit("--commit must be 40 lowercase hex characters")
    plat, arch = host_platform(), a.arch or host_arch()
    out = a.out.resolve()
    work = (a.work or out / "work").resolve()
    out.mkdir(parents=True, exist_ok=True)
    work.mkdir(parents=True, exist_ok=True)
    src = work / "vscode"

    if not src.exists():
        run(["git", "clone", "--depth", "1", "--branch", a.tag, REPO, str(src)], work)
    head = run(["git", "rev-parse", "HEAD"], src).strip()
    # SECURITY: the source must be exactly the pinned commit (SSDF PS.2); a moved tag or a wrong clone stops here.
    if head != a.commit:
        raise SystemExit(f"tag {a.tag} is at {head}, expected {a.commit}")
    # SECURITY: product.json and the server gulp file are the only files this script changes (see below); any other
    # change means an untrusted tree.
    changed = [line[3:] for line in run(["git", "status", "--porcelain", "--untracked-files=no"], src).splitlines()]
    if [p for p in changed if p not in ("product.json", "build/gulpfile.reh.ts")]:
        raise SystemExit("the VS Code checkout has local changes; delete the work folder and run again")
    node = check_node(src)

    # The editor announces quality "stable"; the server gets the same so its routes and data folders line up.
    pj = src / "product.json"
    product = json.loads(pj.read_text(encoding="utf-8"))
    product["quality"] = "stable"
    # Marketplace-built extensions (js-debug, js-debug-companion, js-profile-table) are left out: the IDE has no
    # debugger UI for them, and downloading them needs GitHub's API. Least functionality (NIST CM-7), smaller server.
    product["builtInExtensions"] = []
    pj.write_text(json.dumps(product, indent="\t") + "\n", encoding="utf-8")

    # The built-in GitHub Copilot extension is left out of the server (least functionality, NIST CM-7: the IDE has
    # its own agent and must not ship a client for a third-party AI service). VS Code's server build step that
    # prepares Copilot's ripgrep shim needs that extension's SDK, so that one step is removed; the build stops if the
    # line isn't found exactly once (an upstream change must be looked at, not silently skipped).
    reh = src / "build" / "gulpfile.reh.ts"
    gulp = reh.read_text(encoding="utf-8")
    shim = "\t\t\t\tprepareCopilotRipgrepShimTaskREH(platform, arch, destinationFolderName)\n"
    if gulp.count(shim) == 1:
        reh.write_text(gulp.replace(shim, ""), encoding="utf-8")
    elif "prepareCopilotRipgrepShimTaskREH(platform, arch, destinationFolderName)" in gulp:
        raise SystemExit("unexpected layout of build/gulpfile.reh.ts; update this script")

    env = dict(os.environ)
    env.update({
        "NODE_OPTIONS": "--max-old-space-size=8192",
        # The desktop app (Electron) and test browsers are not needed for the server.
        "ELECTRON_SKIP_BINARY_DOWNLOAD": "1",
        "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD": "1",
        "VSCODE_ARCH": arch,
        "npm_config_arch": arch,
    })
    if plat == "win32" and not shutil.which("signtool.exe", path=env.get("PATH")):
        sdk = windows_sdk_tools()
        if sdk is None:
            raise SystemExit("signtool.exe not found: install the Windows SDK (Visual Studio C++ workload)")
        # Appended, not prepended, so nothing already on PATH is shadowed.
        env["PATH"] = env.get("PATH", "") + os.pathsep + str(sdk)
    if not (src / "node_modules").exists():
        # SECURITY: npm ci installs exactly the lockfile, with its integrity hashes (SLSA, NIST SA-12).
        run(["npm", "ci"], src, env)
    run(["npm", "run", "gulp", f"vscode-reh-{plat}-{arch}-min"], src, env)

    built = work / f"vscode-reh-{plat}-{arch}"
    if not (built / "out" / "server-main.js").is_file():
        raise SystemExit(f"the build did not produce {built}/out/server-main.js")
    # Copilot is left out (see above): its extension and the server's Copilot runtime packages (about 170 MB, loaded
    # only when the Copilot agent host is asked for) are removed from the output.
    shutil.rmtree(built / "extensions" / "copilot", ignore_errors=True)
    for p in sorted((built / "node_modules" / "@github").glob("copilot*")):
        shutil.rmtree(p)
    server_product = json.loads((built / "product.json").read_text(encoding="utf-8"))
    if server_product.get("commit") != a.commit:
        raise SystemExit(f"the server's product.json says commit {server_product.get('commit')}, expected {a.commit}")

    name = f"p2pide-node-host-{plat}-{arch}-{a.tag}"
    tgz = out / f"{name}.tar.gz"
    # Fixed mtime: the commit's own date, so two builds of the same commit can produce the same bytes.
    epoch = int(run(["git", "log", "-1", "--format=%ct"], src).strip())
    archive(built, tgz, epoch)
    digest = sha256(tgz)
    (out / f"{name}.tar.gz.sha256").write_text(f"{digest}  {tgz.name}\n", encoding="utf-8")
    record = {
        "file": tgz.name,
        "sha256": digest,
        "bytes": tgz.stat().st_size,
        "vscode_tag": a.tag,
        "vscode_commit": a.commit,
        "platform": plat,
        "arch": arch,
        "node": node,
        "built_on": f"{platform.system()} {platform.release()} {platform.machine()}",
        "built_at": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "product_quality": "stable",
        "builtin_marketplace_extensions": "none (left out)",
        "copilot_extension": "left out",
    }
    (out / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
