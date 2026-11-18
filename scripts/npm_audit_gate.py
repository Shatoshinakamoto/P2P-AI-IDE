#!/usr/bin/env python3
"""scripts/npm_audit_gate.py -- the npm audit gate for one package folder, with dated, reviewed exceptions.

Purpose: CI and the local CI fail when `npm audit` finds a high or critical advisory. Sometimes an advisory has no
fixed version yet and only touches build tools that never ship (2026-11-17: GHSA-fc5q-39xc-vjpf in picomatch, used
by vite). A plain `npm audit --audit-level=high` then blocks every change until upstream ships a fix. This script
keeps the gate strict and lets one named advisory through only where it is safe, for a limited time:

1. Shipped dependencies: `npm audit --omit=dev --json` must report no high or critical advisory. Exceptions are
   NEVER applied here, so an excepted advisory that reaches the shipped app still fails the gate.
2. All dependencies (dev included): `npm audit --json` must report no high or critical advisory other than those
   listed, unexpired, in the package folder's `npm-audit-exceptions.json`.
3. An expired, malformed or unknown exception file fails the gate (fail closed), so an exception has to be
   reviewed again by its expiry date (docs/files/npm-audit-exceptions.md).

Where it fits: repository tooling for .github/workflows/ci.yml (supply-chain job) and the local CI
(P2P-AI scripts/local-ci.json). Nothing in the app reads it. The same file is kept in P2P-AI-IDE and TandemForge.

Inputs / trust boundary: the package folder named on the command line (developer input); the JSON that npm prints
(from the npm registry's audit service, treated as untrusted data: parsed with json, never executed; unexpected
shapes fail the gate); the exceptions file (repository content, reviewed in pull requests). Output: a short report
on stdout and the exit code.

Security controls:
- No shell: npm is started with an argument list found by shutil.which (CWE-78); the arguments are constants.
- Fail closed: npm errors, unreadable output, bad exception entries or expired exceptions fail the gate
  (NIST SI-2, SA-11; OWASP A06:2021 Vulnerable and Outdated Components).
- Exceptions are exact advisory ids (GHSA-xxxx-xxxx-xxxx) bound to one package name, with an expiry no more than
  120 days after the date added, so an exception cannot silently cover other advisories or live for ever
  (NIST RA-5, CM-3; SSDF RV.2).
- Shipped code is never excepted (step 1).
Dependencies: Python standard library only (3.9+); npm 7 or later (audit report format v2).

Usage:
  python3 -I scripts/npm_audit_gate.py app
Exit codes: 0 gate passed, 1 gate failed (advisory or expired/bad exception), 2 usage error or npm could not run.
"""
import argparse
import datetime
import json
import os
import re
import shutil
import subprocess
import sys

# Severities that fail the gate; the same threshold as `npm audit --audit-level=high`.
BLOCKING = {"high", "critical"}
# SECURITY: exception ids must be exact GitHub advisory ids; no wildcards or package-wide waivers (CWE-20).
GHSA_RE = re.compile(r"^GHSA(-[23456789cfghjmpqrvwx]{4}){3}$")
# SECURITY: npm package names only (scoped or not), checked before they are compared or printed (CWE-20, CWE-117).
PKG_RE = re.compile(r"^(@[a-z0-9][a-z0-9._~-]{0,213}/)?[a-z0-9][a-z0-9._~-]{0,213}$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
MAX_EXCEPTION_DAYS = 120
EXCEPTIONS_NAME = "npm-audit-exceptions.json"
REQUIRED_KEYS = {"id", "package", "reason", "requested_by", "added", "expires", "review"}


class GateError(Exception):
    """A problem that fails the gate: bad npm output or a bad exceptions file."""


def _date(value, where):
    """Parse a YYYY-MM-DD date string.

    Args:
        value: the string from the exceptions file.
        where: text naming the entry, for the error message.
    Returns:
        datetime.date.
    Raises:
        GateError: when the value is not a real date in that format.
    """
    if not isinstance(value, str) or not DATE_RE.match(value):
        raise GateError(f"{where}: dates must be YYYY-MM-DD")
    try:
        return datetime.date.fromisoformat(value)
    except ValueError as exc:
        raise GateError(f"{where}: {exc}") from None


def load_exceptions(data, today):
    """Check the parsed exceptions file and return the exceptions in force today.

    Args:
        data: the parsed JSON: {"exceptions": [ {id, package, reason, requested_by, added, expires, review}, ... ]}.
        today: datetime.date to compare expiry dates with.
    Returns:
        dict mapping advisory id to its package name, for the unexpired entries.
    Raises:
        GateError: on any malformed entry, an expiry more than MAX_EXCEPTION_DAYS after `added`, or an expired entry.
    Security: fail closed; an expired exception fails the gate instead of being skipped, so it must be reviewed.
    """
    if not isinstance(data, dict) or set(data) - {"exceptions", "comment"} or not isinstance(data.get("exceptions"), list):
        raise GateError(f"{EXCEPTIONS_NAME}: expected an object with an 'exceptions' list")
    active = {}
    for i, entry in enumerate(data["exceptions"]):
        where = f"{EXCEPTIONS_NAME} entry {i}"
        if not isinstance(entry, dict) or set(entry) != REQUIRED_KEYS:
            raise GateError(f"{where}: needs exactly the keys {sorted(REQUIRED_KEYS)}")
        if not isinstance(entry["id"], str) or not GHSA_RE.match(entry["id"]):
            raise GateError(f"{where}: id must be one GHSA advisory id")
        if not isinstance(entry["package"], str) or not PKG_RE.match(entry["package"]):
            raise GateError(f"{where}: package must be an npm package name")
        for key in ("reason", "requested_by", "review"):
            if not isinstance(entry[key], str) or not entry[key].strip():
                raise GateError(f"{where}: {key} must be a non-empty string")
        added = _date(entry["added"], where)
        expires = _date(entry["expires"], where)
        # SECURITY: short-lived waivers only (NIST CM-3, RA-5): forces a fresh look at the advisory.
        if not added <= expires <= added + datetime.timedelta(days=MAX_EXCEPTION_DAYS):
            raise GateError(f"{where}: expires must be on or after added and at most {MAX_EXCEPTION_DAYS} days later")
        if today > expires:
            raise GateError(f"{where}: {entry['id']} expired on {entry['expires']}; review it (fixed version?) and "
                            "renew or remove it")
        if entry["id"] in active:
            raise GateError(f"{where}: {entry['id']} listed twice")
        active[entry["id"]] = entry["package"]
    return active


def advisories(report):
    """List the high and critical advisories in an `npm audit --json` report (format v2, npm 7+).

    Args:
        report: the parsed JSON report.
    Returns:
        set of (advisory id, package name) pairs with a blocking severity.
    Raises:
        GateError: when the report is an npm error or not in the expected shape.
    Security: the report comes from the network; every field is type-checked before use and anything unexpected
    fails the gate (fail closed, CWE-20). Advisories with no GHSA id are kept under their URL so they still block.
    """
    if not isinstance(report, dict) or "error" in report:
        raise GateError("npm audit reported an error (no network or registry problem?)")
    vulns = report.get("vulnerabilities")
    if report.get("auditReportVersion") != 2 or not isinstance(vulns, dict):
        raise GateError("npm audit output is not report version 2; npm 7 or later is needed")
    found = set()
    for name, vuln in vulns.items():
        if not isinstance(vuln, dict) or not isinstance(vuln.get("via"), list):
            raise GateError("npm audit output has an unexpected vulnerability entry")
        # Entries in `via` are either names of other vulnerable packages (strings; their own advisories are
        # listed under their own names) or advisory objects. Only the advisory objects are counted.
        for via in vuln["via"]:
            if not isinstance(via, dict):
                continue
            sev = via.get("severity")
            if sev not in BLOCKING:
                continue
            url = via.get("url") if isinstance(via.get("url"), str) else ""
            adv_id = url.rstrip("/").rsplit("/", 1)[-1] if url else ""
            if not GHSA_RE.match(adv_id):
                adv_id = url or f"advisory {via.get('source')!r}"
            pkg = via.get("name") if isinstance(via.get("name"), str) else name
            found.add((adv_id, pkg))
    return found


def evaluate(full_report, prod_report, exceptions):
    """Decide the gate from the two audit reports and the exceptions in force.

    Args:
        full_report: parsed `npm audit --json` (all dependencies).
        prod_report: parsed `npm audit --omit=dev --json` (shipped dependencies only).
        exceptions: dict from load_exceptions (advisory id -> package).
    Returns:
        (ok, lines): ok is True when the gate passes; lines is the report to print.
    Security: exceptions only ever apply to the full report; any blocking advisory in shipped code fails.
    """
    lines = []
    ok = True
    prod = advisories(prod_report)
    for adv_id, pkg in sorted(prod):
        ok = False
        lines.append(f"FAIL shipped dependency: {pkg} {adv_id} (exceptions never apply to shipped code)")
    used = set()
    for adv_id, pkg in sorted(advisories(full_report)):
        if (adv_id, pkg) in prod:
            continue
        if exceptions.get(adv_id) == pkg:
            used.add(adv_id)
            lines.append(f"excepted (dev only): {pkg} {adv_id}, see {EXCEPTIONS_NAME}")
        else:
            ok = False
            lines.append(f"FAIL dev dependency: {pkg} {adv_id}")
    for adv_id in sorted(set(exceptions) - used):
        # Not a failure: the advisory may be fixed or withdrawn. The entry should be removed.
        lines.append(f"note: exception {adv_id} is no longer needed; remove it from {EXCEPTIONS_NAME}")
    lines.append("npm audit gate: " + ("passed" if ok else "FAILED"))
    return ok, lines


def run_npm_audit(npm, folder, omit_dev):
    """Run `npm audit --json` in a folder and parse its output.

    Args:
        npm: absolute path of the npm executable (from shutil.which).
        folder: the package folder (holds package.json and package-lock.json).
        omit_dev: True to add --omit=dev (shipped dependencies only).
    Returns:
        the parsed JSON report.
    Raises:
        GateError: when npm prints nothing parsable.
    Security: argument list, no shell (CWE-78); a timeout bounds a hung registry call (CWE-400). npm exits non-zero
    when it finds advisories, so the exit code is not used; the parsed report decides.
    """
    argv = [npm, "audit", "--json"] + (["--omit=dev"] if omit_dev else [])
    proc = subprocess.run(argv, cwd=folder, capture_output=True, text=True, timeout=300, check=False)
    try:
        return json.loads(proc.stdout)
    except ValueError:
        raise GateError(f"npm audit printed no JSON (exit {proc.returncode})") from None


def main(argv=None):
    """Command-line entry point. See the module docstring for usage and exit codes."""
    parser = argparse.ArgumentParser(description="npm audit gate with dated exceptions for dev-only advisories")
    parser.add_argument("folder", help="package folder with package.json and package-lock.json")
    args = parser.parse_args(argv)
    folder = os.path.abspath(args.folder)
    if not os.path.isfile(os.path.join(folder, "package-lock.json")):
        print(f"no package-lock.json in {args.folder}", file=sys.stderr)
        return 2
    npm = shutil.which("npm")
    if not npm:
        print("npm not found on PATH", file=sys.stderr)
        return 2
    try:
        exc_path = os.path.join(folder, EXCEPTIONS_NAME)
        data = {"exceptions": []}
        if os.path.isfile(exc_path):
            with open(exc_path, encoding="utf-8") as fh:
                try:
                    data = json.load(fh)
                except ValueError as exc:
                    raise GateError(f"{EXCEPTIONS_NAME}: not valid JSON ({exc})") from None
        exceptions = load_exceptions(data, datetime.date.today())
        ok, lines = evaluate(run_npm_audit(npm, folder, False), run_npm_audit(npm, folder, True), exceptions)
    except GateError as exc:
        print(f"npm audit gate: FAILED: {exc}")
        return 1
    except (OSError, subprocess.TimeoutExpired) as exc:
        print(f"npm audit gate: could not run npm: {exc.__class__.__name__}", file=sys.stderr)
        return 2
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
