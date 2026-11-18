#!/usr/bin/env python3
"""scripts/test_npm_audit_gate.py -- unit tests for scripts/npm_audit_gate.py (TEST TOOLING).

Covers the gate's decisions without network: shipped advisories always fail, dev advisories fail unless excepted
by exact id and package, expired / over-long / malformed exceptions fail closed, and npm error output fails.
Security: these are the negative tests for the gate's fail-closed rules (NIST SA-11, SSDF PW.8).
Run: python3 -I -m unittest discover -s scripts -p test_npm_audit_gate.py
Dependencies: Python standard library only.
"""
import datetime
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import npm_audit_gate as gate  # noqa: E402

PICO = "GHSA-fc5q-39xc-vjpf"
TODAY = datetime.date(2026, 11, 17)


def report(*advs):
    """Build a minimal npm audit v2 report; each adv is (package, ghsa, severity)."""
    vulns = {}
    for pkg, ghsa, sev in advs:
        vulns[pkg] = {"name": pkg, "severity": sev, "via": [
            {"source": 1, "name": pkg, "url": f"https://github.com/advisories/{ghsa}", "severity": sev}]}
        vulns["vite"] = {"name": "vite", "severity": sev, "via": [pkg]}
    return {"auditReportVersion": 2, "vulnerabilities": vulns}


def exc(**over):
    entry = {"id": PICO, "package": "picomatch", "reason": "build only", "requested_by": "thread",
             "added": "2026-11-17", "expires": "2026-12-31", "review": "check for picomatch > 4.0.7"}
    entry.update(over)
    return {"exceptions": [entry]}


class GateTests(unittest.TestCase):
    def test_clean_passes(self):
        ok, _ = gate.evaluate(report(), report(), {})
        self.assertTrue(ok)

    def test_dev_advisory_without_exception_fails(self):
        ok, _ = gate.evaluate(report(("picomatch", PICO, "high")), report(), {})
        self.assertFalse(ok)

    def test_dev_advisory_with_exception_passes(self):
        ok, lines = gate.evaluate(report(("picomatch", PICO, "high")), report(), gate.load_exceptions(exc(), TODAY))
        self.assertTrue(ok, lines)

    def test_exception_never_applies_to_shipped(self):
        r = report(("picomatch", PICO, "high"))
        ok, _ = gate.evaluate(r, r, gate.load_exceptions(exc(), TODAY))
        self.assertFalse(ok)

    def test_exception_bound_to_package(self):
        ok, _ = gate.evaluate(report(("other", PICO, "high")), report(), gate.load_exceptions(exc(), TODAY))
        self.assertFalse(ok)

    def test_other_advisory_still_fails(self):
        full = report(("picomatch", PICO, "high"), ("minimist", "GHSA-xvch-5gv4-984h", "critical"))
        ok, _ = gate.evaluate(full, report(), gate.load_exceptions(exc(), TODAY))
        self.assertFalse(ok)

    def test_moderate_ignored(self):
        ok, _ = gate.evaluate(report(("x", "GHSA-xvch-5gv4-984h", "moderate")), report(), {})
        self.assertTrue(ok)

    def test_expired_fails(self):
        with self.assertRaises(gate.GateError):
            gate.load_exceptions(exc(), datetime.date(2027, 1, 1))

    def test_too_long_fails(self):
        with self.assertRaises(gate.GateError):
            gate.load_exceptions(exc(expires="2027-06-30"), TODAY)

    def test_wildcard_id_fails(self):
        with self.assertRaises(gate.GateError):
            gate.load_exceptions(exc(id="GHSA-*"), TODAY)

    def test_missing_key_fails(self):
        data = exc()
        del data["exceptions"][0]["review"]
        with self.assertRaises(gate.GateError):
            gate.load_exceptions(data, TODAY)

    def test_npm_error_fails(self):
        with self.assertRaises(gate.GateError):
            gate.evaluate({"error": {"code": "ENOTFOUND"}}, report(), {})

    def test_old_report_format_fails(self):
        with self.assertRaises(gate.GateError):
            gate.evaluate({"advisories": {}}, report(), {})

    def test_unused_exception_is_note_not_failure(self):
        ok, lines = gate.evaluate(report(), report(), gate.load_exceptions(exc(), TODAY))
        self.assertTrue(ok)
        self.assertTrue(any("no longer needed" in line for line in lines))


if __name__ == "__main__":
    unittest.main()
