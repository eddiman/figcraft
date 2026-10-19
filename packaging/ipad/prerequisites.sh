#!/bin/bash
set -euo pipefail

ipad_fail() { echo "iPad build: $*" >&2; exit 1; }
ipad_require() { command -v "$1" >/dev/null 2>&1 || ipad_fail "missing tool: $1"; }
ipad_preflight() {
    for tool in xcodebuild xcrun rustup cargo; do ipad_require "$tool"; done
    if [[ -z "${DEVELOPER_DIR:-}" ]]; then
        DEVELOPER_DIR="$(xcode-select -p)" || ipad_fail "select full Xcode or set DEVELOPER_DIR"
        export DEVELOPER_DIR
    fi
    [[ -d "$DEVELOPER_DIR/Platforms/iPhoneOS.platform" ]] || ipad_fail "full Xcode is required; set DEVELOPER_DIR to Xcode.app/Contents/Developer"
    xcodebuild -version >/dev/null || ipad_fail "Xcode is unavailable at $DEVELOPER_DIR"
    SDKROOT="$(xcrun --sdk "$IPAD_SDK" --show-sdk-path)" || ipad_fail "missing SDK: $IPAD_SDK"
    [[ -d "$SDKROOT" ]] || ipad_fail "SDK directory is unavailable: $SDKROOT"
    export SDKROOT
    local installed targets found=false
    targets="$(rustup target list --installed)" || ipad_fail "cannot list installed Rust targets"
    while IFS= read -r installed; do
        if [[ "$installed" == "$IPAD_TARGET" ]]; then found=true; fi
    done <<< "$targets"
    "$found" || ipad_fail "missing Rust target $IPAD_TARGET; install it explicitly with rustup target add $IPAD_TARGET"
}
