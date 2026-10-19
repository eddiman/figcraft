#!/bin/bash
set -euo pipefail

IPAD_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$IPAD_DIR/prerequisites.sh"
[[ "${1:-}" == simulator ]] || ipad_fail "usage: test.sh simulator [--udid UDID] [--output NEW_DIRECTORY] [--test-failure]"
shift
IPAD_SDK=iphonesimulator
IPAD_TARGET=aarch64-apple-ios-sim
ipad_preflight
ipad_require python3
unset SDKROOT
exec python3 "$IPAD_DIR/tests/simulator.py" "$@"
