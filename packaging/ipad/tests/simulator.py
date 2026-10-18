#!/usr/bin/env python3
"""Run real XCTest UI smoke checks in an isolated simulator app container."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import uuid

REPO = Path(__file__).resolve().parents[3]
TESTS = Path(__file__).resolve().parent


def select_ipad(devices, runtimes, requested=None):
    supported = {
        runtime["identifier"] for runtime in runtimes
        if runtime.get("isAvailable") and int(runtime["version"].split(".")[0]) >= 17
        and runtime["identifier"].startswith("com.apple.CoreSimulator.SimRuntime.iOS-")
    }
    choices = [
        dict(device, runtime=runtime)
        for runtime, entries in devices.items() if runtime in supported
        for device in entries
        if device.get("isAvailable")
        and device.get("deviceTypeIdentifier", "").startswith("com.apple.CoreSimulator.SimDeviceType.iPad-")
        and (requested is None or device["udid"] == requested)
    ]
    if not choices:
        raise RuntimeError("No available iPad simulator with iOS 17+ matches the destination; "
                           "install a runtime/create an iPad in Xcode or supply --udid.")
    return min(choices, key=lambda device: (device["state"] != "Booted", device["name"], device["udid"]))


def run(command, log=None, check=True, timeout=600):
    print("$", " ".join(map(str, command)), flush=True)
    result = subprocess.run(command, cwd=REPO, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=timeout)
    if log is not None:
        log.write_text(result.stdout)
    if result.returncode:
        print(result.stdout[-12000:], file=sys.stderr)
        if check:
            raise subprocess.CalledProcessError(result.returncode, command, result.stdout)
    return result


def simctl(*args, **kwargs):
    return run(["xcrun", "simctl", *args], **kwargs)


def relocate(value, root):
    if isinstance(value, str):
        return value.replace("__TESTROOT__", str(root))
    if isinstance(value, dict):
        return {key: relocate(item, root) for key, item in value.items()}
    if isinstance(value, list):
        return [relocate(item, root) for item in value]
    return value


def check_summary(summary):
    counts = ("passedTests", "failedTests", "skippedTests", "expectedFailures", "totalTestCount")
    if (any(type(summary.get(key)) is not int for key in counts)
            or summary.get("passedTests", 0) < 1 or summary.get("failedTests") != 0
            or summary.get("skippedTests") != 0 or summary.get("expectedFailures") != 0
            or summary.get("totalTestCount") != summary.get("passedTests")
            or summary.get("result") != "Passed"):
        raise RuntimeError("XCTest must report at least one passing test, no failures/skips, and Passed.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--udid", help="Explicit available iPad destination, never a phone.")
    parser.add_argument("--output", type=Path, help="New directory for retained evidence.")
    parser.add_argument("--test-failure", action="store_true", help="Run the deliberate failing assertion (exits nonzero).")
    args = parser.parse_args()
    if os.uname().machine != "arm64":
        raise RuntimeError("The arm64 simulator runner requires an Apple Silicon Mac.")
    root = REPO / "target/ipad-tests"
    root.mkdir(parents=True, exist_ok=True)
    lock = (root / "runner.lock").open("w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as error:
        raise RuntimeError("Another simulator runner is using this checkout.") from error
    output = args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix="run-", dir=root))
    if args.output:
        output.mkdir(parents=True, exist_ok=False)
    print(f"Simulator evidence: {output}", flush=True)
    devices = json.loads(simctl("list", "devices", "available", "-j").stdout)["devices"]
    runtimes = json.loads(simctl("list", "runtimes", "-j").stdout)["runtimes"]
    device = select_ipad(devices, runtimes, args.udid)
    (output / "destination.json").write_text(json.dumps(device, indent=2) + "\n")
    identifier = "org.figcraft.ipad-test-" + uuid.uuid4().hex
    metadata = {
        "qualification": "simulator only", "bundle_identifier": identifier,
        "commit": run(["git", "rev-parse", "HEAD"]).stdout.strip(),
        "worktree": run(["git", "status", "--porcelain"]).stdout,
        "rustc": run(["rustc", "-vV"]).stdout,
        "xcode": run(["xcodebuild", "-version"]).stdout,
        "simulator_sdk": run(["xcrun", "--sdk", "iphonesimulator", "--show-sdk-version"]).stdout.strip(),
        "device_sdk": run(["xcrun", "--sdk", "iphoneos", "--show-sdk-version"]).stdout.strip(),
        "lockfile_sha256": hashlib.sha256((REPO / "Cargo.lock").read_bytes()).hexdigest(),
    }
    (output / "inputs.json").write_text(json.dumps(metadata, indent=2) + "\n")
    installed = False
    did_boot = False
    ran_tests = False
    app = output / "FigcraftIPad.app"
    runner = output / "IPadSmokeTests-Runner.app"
    try:
        run([str(REPO / "packaging/ipad/build.sh"), "simulator"],
            output / "app-build.log", timeout=1800)
        source = REPO / "target/ipad-xcode/simulator/Build/Products/Debug-iphonesimulator/FigcraftIPad.app"
        shutil.copytree(source, app)
        info_path = app / "Info.plist"
        info = plistlib.loads(info_path.read_bytes())
        info["CFBundleIdentifier"] = identifier
        info_path.write_bytes(plistlib.dumps(info))
        run(["codesign", "--force", "--sign", "-", str(app)], output / "app-sign.log")
        metadata["executable_sha256"] = hashlib.sha256((app / info["CFBundleExecutable"]).read_bytes()).hexdigest()
        (output / "inputs.json").write_text(json.dumps(metadata, indent=2) + "\n")
        build = root / "build"
        run([
            "xcodebuild", "-project", str(TESTS / "IPadSmokeTests.xcodeproj"),
            "-scheme", "IPadSmokeTests", "-sdk", "iphonesimulator",
            "-destination", "generic/platform=iOS Simulator", "-derivedDataPath", str(build),
            "ARCHS=arm64", "CODE_SIGNING_ALLOWED=YES", "CODE_SIGN_IDENTITY=-", "build-for-testing",
        ], output / "test-build.log")
        configs = list((build / "Build/Products").glob("*.xctestrun"))
        if len(configs) != 1:
            raise RuntimeError(f"Expected one XCTest configuration, found {len(configs)}; review {build}.")
        config = relocate(plistlib.loads(configs[0].read_bytes()), configs[0].parent)
        target = config["IPadSmokeTests"]
        runner_source = Path(target["TestHostPath"])
        shutil.copytree(runner_source, runner)
        runner_info = runner / "Info.plist"
        runner_plist = plistlib.loads(runner_info.read_bytes())
        runner_plist["CFBundleIdentifier"] = identifier + ".runner"
        runner_info.write_bytes(plistlib.dumps(runner_plist))
        run(["codesign", "--force", "--deep", "--sign", "-", str(runner)], output / "runner-sign.log")
        target["TestHostPath"] = str(runner)
        target["TestHostBundleIdentifier"] = identifier + ".runner"
        target["DependentProductPaths"] = [
            path.replace(str(runner_source), str(runner)) for path in target["DependentProductPaths"]
        ]
        target["EnvironmentVariables"]["FIGCRAFT_TEST_APP_ID"] = identifier
        target["UserAttachmentLifetime"] = "keepAlways"
        target["SystemAttachmentLifetime"] = "keepAlways"
        config_path = output / "smoke.xctestrun"
        config_path.write_bytes(plistlib.dumps(config))
        if device["state"] != "Booted":
            simctl("boot", device["udid"])
            did_boot = True
        simctl("bootstatus", device["udid"], "-b", log=output / "boot.log")
        installed = True
        simctl("install", device["udid"], str(app), log=output / "install.log")
        test = "test_ipad_forced_failure" if args.test_failure else "test_ipad_editor"
        result_bundle = output / "smoke.xcresult"
        ran_tests = True
        result = run([
            "xcodebuild", "test-without-building", "-xctestrun", str(config_path),
            "-destination", f"platform=iOS Simulator,id={device['udid']}",
            "-destination-timeout", "60", "-parallel-testing-enabled", "NO",
            "-resultBundlePath", str(result_bundle),
            "-only-testing:IPadSmokeTests/IPadSmokeTests/" + test,
        ], output / "tests.log", check=False)
        if result_bundle.exists():
            summary = run(["xcrun", "xcresulttool", "get", "test-results", "summary",
                           "--path", str(result_bundle)], output / "summary.json")
            parsed = json.loads(summary.stdout)
            run(["xcrun", "xcresulttool", "export", "attachments", "--path", str(result_bundle),
                 "--output-path", str(output / "attachments")], output / "attachments.log")
        else:
            raise RuntimeError("XCTest did not produce a result bundle.")
        if result.returncode:
            raise subprocess.CalledProcessError(result.returncode, result.args)
        check_summary(parsed)
        if args.test_failure:
            raise RuntimeError("The deliberately failing assertion unexpectedly passed.")
    finally:
        cleanup_failed = False
        for bundle, should_remove in ((identifier, installed), (identifier + ".runner", ran_tests)):
            if should_remove:
                result = simctl("uninstall", device["udid"], bundle,
                                log=output / (bundle + "-uninstall.log"), check=False)
                cleanup_failed |= result.returncode != 0
        if did_boot:
            result = simctl("shutdown", device["udid"], log=output / "shutdown.log", check=False)
            cleanup_failed |= result.returncode != 0
        for bundle in (app, runner):
            if bundle.exists():
                shutil.rmtree(bundle)
        if cleanup_failed:
            raise RuntimeError(f"Simulator cleanup failed; inspect {output}.")
    print(f"PASS: {parsed['passedTests']} simulator UI test(s); evidence: {output}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"iPad simulator test: {error}", file=sys.stderr)
        sys.exit(error.returncode if isinstance(error, subprocess.CalledProcessError) else 1)
