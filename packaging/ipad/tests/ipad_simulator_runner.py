#!/usr/bin/env python3
"""Host-only regressions for destination selection and XCTest failure propagation."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from simulator import check_summary, relocate, run, select_ipad


def verify_failure(path):
    summary = json.loads(path.read_text())
    if (summary.get("result") != "Failed" or summary.get("failedTests") != 1
            or summary.get("totalTestCount") != 1 or summary.get("passedTests") != 0
            or summary.get("skippedTests") != 0 or summary.get("expectedFailures") != 0
            or not any(failure.get("testIdentifierString") == "IPadSmokeTests/test_ipad_forced_failure()"
                       and "Deliberate T23 assertion failure" in failure.get("failureText", "")
                       for failure in summary.get("testFailures", []))):
        raise RuntimeError("Expected exactly the deliberate XCTest assertion failure, not a setup/build failure.")
    print("ipad_deliberate_assertion_failure: PASS (deliberate XCTest failure confirmed)")


class SimulatorRunnerTests(unittest.TestCase):
    def setUp(self):
        self.runtime = "com.apple.CoreSimulator.SimRuntime.iOS-26-3"
        self.runtimes = [{"identifier": self.runtime, "version": "26.3", "isAvailable": True}]
        self.ipad = {
            "udid": "ipad-1", "name": "Renamed tablet", "state": "Shutdown", "isAvailable": True,
            "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPad-Air-11-inch-M3",
        }
        self.phone = dict(self.ipad, udid="phone", name="iPad impostor",
                          deviceTypeIdentifier="com.apple.CoreSimulator.SimDeviceType.iPhone-17")
        self.devices = {self.runtime: [self.phone, self.ipad]}
        self.summary = {
            "result": "Passed", "passedTests": 1, "failedTests": 0, "skippedTests": 0,
            "expectedFailures": 0, "totalTestCount": 1,
        }

    def test_ipad_uses_device_type_not_display_name(self):
        self.assertEqual(select_ipad(self.devices, self.runtimes)["udid"], "ipad-1")

    def test_ipad_rejects_phone_or_unknown_explicit_destination(self):
        for udid in ("phone", "missing"):
            with self.subTest(udid=udid), self.assertRaisesRegex(RuntimeError, "No available iPad"):
                select_ipad(self.devices, self.runtimes, udid)

    def test_ipad_rejects_unavailable_and_old_runtimes(self):
        for change in ({"isAvailable": False}, {"version": "16.4"}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                select_ipad(self.devices, [dict(self.runtimes[0], **change)])
        with self.assertRaises(RuntimeError):
            select_ipad({self.runtime: [dict(self.ipad, isAvailable=False)]}, self.runtimes)

    def test_ipad_prefers_booted_but_honors_explicit_choice(self):
        devices = copy.deepcopy(self.devices)
        devices[self.runtime].append(dict(self.ipad, udid="ipad-2", state="Booted"))
        self.assertEqual(select_ipad(devices, self.runtimes)["udid"], "ipad-2")
        self.assertEqual(select_ipad(devices, self.runtimes, "ipad-1")["udid"], "ipad-1")

    def test_ipad_rejects_empty_or_incomplete_test_success(self):
        check_summary(self.summary)
        for change in (
            {"passedTests": 0}, {"failedTests": 1}, {"skippedTests": 1},
            {"expectedFailures": 1}, {"result": "Failed"}, {"totalTestCount": 2},
            {"passedTests": True},
        ):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                check_summary(dict(self.summary, **change))
        with self.assertRaises(RuntimeError):
            check_summary({})
        with self.assertRaises(RuntimeError):
            check_summary({"result": "Passed", "passedTests": 1})

    def test_ipad_preserves_command_failure_and_log(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "failure.log"
            with self.assertRaises(subprocess.CalledProcessError) as raised:
                run([sys.executable, "-c", "print('deliberate command failure'); exit(7)"], log)
            self.assertEqual(raised.exception.returncode, 7)
            self.assertIn("deliberate command failure", log.read_text())

    def test_ipad_relocates_generated_test_paths_without_losing_placeholders(self):
        config = {"path": "__TESTROOT__/runner.app", "nested": ["__TESTHOST__/test.xctest", True]}
        self.assertEqual(relocate(config, Path("/tmp/build")), {
            "path": "/tmp/build/runner.app", "nested": ["__TESTHOST__/test.xctest", True],
        })

    def test_ipad_negative_proof_rejects_other_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "summary.json"
            summary = dict(self.summary, result="Failed", passedTests=0, failedTests=1,
                           testFailures=[{
                               "failureText": "Deliberate T23 assertion failure",
                               "testIdentifierString": "IPadSmokeTests/test_ipad_forced_failure()",
                           }])
            path.write_text(json.dumps(summary))
            verify_failure(path)
            summary["testFailures"] = [{"failureText": "Launch failed"}]
            path.write_text(json.dumps(summary))
            with self.assertRaises(RuntimeError):
                verify_failure(path)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--verify-failure":
        verify_failure(Path(sys.argv[2]))
    else:
        unittest.main(verbosity=2)
