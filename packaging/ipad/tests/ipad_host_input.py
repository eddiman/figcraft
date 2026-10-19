#!/usr/bin/env python3
"""Test the original host loop and clipboard producer without changing their lockfiles."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def run_tests(repo, package, features, minimum):
    vendor = repo / "maintenance/ipad-deps"
    source = vendor / package
    formatting = (source / "src/native/epi_integration.rs" if package == "eframe"
                  else source / "src/ipad_shortcut_tests.rs")
    subprocess.run([
        "rustfmt", "--check", "--edition", "2024",
        "--config", "max_width=100,use_small_heuristics=Default",
        str(formatting),
    ], cwd=repo, check=True)
    manifest = (source / "Cargo.toml").read_text()
    library_path = 'path = "src/lib.rs"'
    if manifest.count(library_path) != 1:
        raise RuntimeError(f"{package}'s library manifest changed; review the host test runner")
    manifest = manifest.replace(
        library_path, "path = " + json.dumps(str(source / "src/lib.rs"))
    )
    if package == "eframe":
        # Inspection is not enabled by the native host or present in its root lockfile.
        manifest, removed = re.subn(
            r"(?ms)^\[dependencies\.egui_inspection\]\n.*?(?=^\[)",
            "",
            manifest,
        )
        if removed != 1:
            raise RuntimeError("eframe's inspection dependency changed; review the host test runner")
        manifest = manifest.replace('    "dep:egui_inspection",\n', "")
    manifest += "\n[workspace]\n\n[patch.crates-io]\n"
    for name in ("egui", "egui-winit", "winit"):
        manifest += f"{name} = {{ path = {json.dumps(str(vendor / name))} }}\n"
    manifest += "\n" + "\n".join(re.findall(
        r"(?ms)^\[profile\.dev(?:\.[^\]]+)?\]\n.*?(?=^\[|\Z)",
        (repo / "Cargo.toml").read_text(),
    ))

    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(repo / "target"))
    env.setdefault("CARGO_INCREMENTAL", "0")
    env.setdefault("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
    with tempfile.TemporaryDirectory(prefix="figcraft-ipad-host-input-") as directory:
        root = Path(directory)
        (root / "Cargo.toml").write_text(manifest)
        shutil.copyfile(repo / "Cargo.lock", root / "Cargo.lock")
        common = [
            "--offline",
            "--manifest-path", str(root / "Cargo.toml"),
            "--no-default-features",
            "--features", features,
        ]
        subprocess.run(["cargo", "check", "--quiet", *common, "--lib"], cwd=repo, env=env, check=True)
        original = set(re.findall(
            r'(?m)^name = "([^"]+)"\nversion = "([^"]+)"',
            (repo / "Cargo.lock").read_text(),
        ))
        compiler = subprocess.check_output(["rustc", "-vV"], text=True)
        host = next(line[len("host: "):] for line in compiler.splitlines() if line.startswith("host: "))
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", *common, "--format-version", "1", "--filter-platform", host],
            cwd=repo, env=env, text=True,
        ))
        active = {node["id"] for node in metadata["resolve"]["nodes"]}
        resolved = {
            (package["name"], package["version"])
            for package in metadata["packages"]
            if package["id"] in active
        }
        declared_tests = set(re.findall(
            r'(?m)^name = "([^"]+)"\nversion = "([^"]+)"',
            (vendor / "eframe/Cargo.lock").read_text(),
        ))
        if not resolved or resolved - original - declared_tests:
            raise RuntimeError("host test dependencies differ from the recorded lockfiles")
        nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
        runtime = set()
        remaining = [metadata["resolve"]["root"]]
        while remaining:
            package = remaining.pop()
            if package in runtime:
                continue
            runtime.add(package)
            remaining.extend(
                dependency["pkg"]
                for dependency in nodes[package]["deps"]
                if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"])
            )
        changed = {
            (package["name"], package["version"])
            for package in metadata["packages"]
            if package["id"] in runtime
        } - original
        if changed:
            raise RuntimeError(f"host runtime dependencies differ from production: {changed}")
        command = [
            "cargo", "test", "--locked", *common,
            "--lib", "ipad_host_", "--", "--nocapture",
        ]
        result = subprocess.run(
            command, cwd=repo, env=env,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        print(result.stdout, end="", flush=True)
        if result.returncode:
            raise SystemExit(result.returncode)
        matched = re.search(r"test result: ok\. (\d+) passed;", result.stdout)
        if not matched or int(matched.group(1)) < minimum:
            raise RuntimeError(f"{package} input verification must run at least {minimum} passing tests")
        lint = subprocess.run(
            ["cargo", "clippy", "--locked", *common, "--lib", "--tests", "--message-format=json"],
            cwd=repo, env=env, stdout=subprocess.PIPE, text=True,
        )
        changed_files = {
            str(source / "src/native/epi_integration.rs"),
            str(source / "src/native/epi_integration_tests.rs"),
        } if package == "eframe" else {
            str(source / "src/ipad_shortcut_tests.rs"),
        }
        producer_range = None
        if package == "egui-winit":
            text = (source / "src/lib.rs").read_text()
            first = text.index('#[cfg(all(test, feature = "figcraft-ipad-clipboard"))]')
            last = text.index("\nfn translate_mouse_button", first)
            producer_range = (text[:first].count("\n") + 1, text[:last].count("\n") + 1)
        new_warnings = []
        for line in lint.stdout.splitlines():
            message = json.loads(line)
            if message["reason"] != "compiler-message":
                continue
            diagnostic = message["message"]
            print(diagnostic["rendered"], end="", flush=True)
            if diagnostic["level"] in ("warning", "error") and any(
                span["file_name"] in changed_files or (
                    producer_range is not None
                    and span["file_name"] == str(source / "src/lib.rs")
                    and producer_range[0] <= span["line_start"] < producer_range[1]
                ) for span in diagnostic["spans"]
            ):
                new_warnings.append(diagnostic["message"])
        if lint.returncode:
            raise SystemExit(lint.returncode)
        if new_warnings:
            raise RuntimeError(f"host input code has lint diagnostics: {new_warnings}")


def main():
    repo = Path(__file__).resolve().parents[3]
    run_tests(repo, "eframe", "default_fonts,wgpu,figcraft-ipad-focus,figcraft-ipad-clipboard", 6)
    run_tests(repo, "egui-winit", "figcraft-ipad-clipboard", 4)
    run_tests(repo, "egui-winit", "", 1)


if __name__ == "__main__":
    main()
