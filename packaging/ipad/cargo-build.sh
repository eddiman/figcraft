#!/bin/bash
set -euo pipefail

IPAD_DIR="$(cd "$(dirname "$0")" && pwd)"
IPAD_REPO="$(cd "$IPAD_DIR/../.." && pwd)"
source "$IPAD_DIR/prerequisites.sh"
case "${PLATFORM_NAME:-}" in
    iphonesimulator) IPAD_SDK=iphonesimulator; IPAD_TARGET=aarch64-apple-ios-sim ;;
    iphoneos) IPAD_SDK=iphoneos; IPAD_TARGET=aarch64-apple-ios ;;
    *) ipad_fail "unsupported PLATFORM_NAME: ${PLATFORM_NAME:-unset}" ;;
esac
ipad_preflight
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="$IPAD_REPO/target"
IPAD_CLANG="$DEVELOPER_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang"
[[ -x "$IPAD_CLANG" ]] || ipad_fail "Xcode clang is unavailable: $IPAD_CLANG"
case "$IPAD_TARGET" in
    aarch64-apple-ios-sim) export CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER="$IPAD_CLANG" ;;
    aarch64-apple-ios) export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER="$IPAD_CLANG" ;;
esac
cd "$IPAD_REPO"
cargo build --locked -p figcraft-ipad --target "$IPAD_TARGET"
[[ -n "${TARGET_BUILD_DIR:-}" && -n "${EXECUTABLE_PATH:-}" ]] || ipad_fail "Xcode executable destination is missing"
mkdir -p "$(dirname "$TARGET_BUILD_DIR/$EXECUTABLE_PATH")"
install -m 755 "$CARGO_TARGET_DIR/$IPAD_TARGET/debug/figcraft-ipad" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
