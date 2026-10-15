#!/usr/bin/env python3
"""Exercise build prerequisites without uninstalling any host tools or SDKs."""
import os
from pathlib import Path
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[3]
script = repo / 'packaging/ipad/build.sh'

def ipad_missing_xcode():
    env = os.environ.copy()
    env['DEVELOPER_DIR'] = '/nonexistent/figcraft-ipad-xcode/Contents/Developer'
    result = subprocess.run([str(script), 'simulator'], env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode != 0, result.stdout
    assert 'full Xcode is required' in result.stdout, result.stdout
    print('ipad_missing_xcode: PASS (nonzero exit, actionable error)')

def ipad_missing_target():
    with tempfile.TemporaryDirectory(prefix='figcraft-ipad-prerequisites-') as directory:
        wrapper = Path(directory) / 'rustup'
        wrapper.write_text('#!/bin/bash\nset -euo pipefail\nprintf "aarch64-apple-darwin\\n"\n')
        wrapper.chmod(0o755)
        env = os.environ.copy()
        env['PATH'] = directory + os.pathsep + env['PATH']
        result = subprocess.run([str(script), 'simulator'], env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        assert result.returncode != 0, result.stdout
        assert 'missing Rust target aarch64-apple-ios-sim' in result.stdout, result.stdout
        print('ipad_missing_target: PASS (nonzero exit, actionable error)')

def ipad_missing_sdk():
    with tempfile.TemporaryDirectory(prefix='figcraft-ipad-sdk-') as directory:
        wrapper = Path(directory) / 'xcrun'
        wrapper.write_text('#!/bin/bash\nset -euo pipefail\necho "test: SDK unavailable" >&2\nexit 72\n')
        wrapper.chmod(0o755)
        env = os.environ.copy()
        env['PATH'] = directory + os.pathsep + env['PATH']
        result = subprocess.run([str(script), 'simulator'], env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        assert result.returncode != 0, result.stdout
        assert 'missing SDK: iphonesimulator' in result.stdout, result.stdout
        print('ipad_missing_sdk: PASS (nonzero exit, actionable error)')

ipad_missing_xcode()
ipad_missing_target()
ipad_missing_sdk()
