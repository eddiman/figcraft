#!/bin/bash
set -euo pipefail
IPAD_DIR="$(cd "$(dirname "$0")" && pwd)"
IPAD_REPO="$(cd "$IPAD_DIR/../.." && pwd)"
source "$IPAD_DIR/prerequisites.sh"
case "${1:-simulator}" in
    simulator) IPAD_SDK=iphonesimulator; IPAD_TARGET=aarch64-apple-ios-sim ;;
    device) IPAD_SDK=iphoneos; IPAD_TARGET=aarch64-apple-ios ;;
    *) ipad_fail "usage: check-bridge.sh [simulator|device]" ;;
esac
ipad_preflight
IPAD_BRIDGE_SDKROOT="$SDKROOT"
export CARGO_TARGET_DIR="$IPAD_REPO/target"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_NET_OFFLINE=true
unset SDKROOT
python3 "$IPAD_DIR/tests/ipad_host_input.py"
for IPAD_BRIDGE in "$IPAD_REPO/platform/apple-ipad/Cargo.toml" "$IPAD_REPO/platform/apple-ipad-clipboard/Cargo.toml"; do
    unset SDKROOT
    cargo fmt --manifest-path "$IPAD_BRIDGE" -- --check
    cargo test --manifest-path "$IPAD_BRIDGE" --locked --lib
    export SDKROOT="$IPAD_BRIDGE_SDKROOT"
    cargo clippy --manifest-path "$IPAD_BRIDGE" --locked --all-targets --target "$IPAD_TARGET" -- -D warnings
done
