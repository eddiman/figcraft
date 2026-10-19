#!/bin/bash
set -euo pipefail

IPAD_DIR="$(cd "$(dirname "$0")" && pwd)"
IPAD_REPO="$(cd "$IPAD_DIR/../.." && pwd)"
source "$IPAD_DIR/prerequisites.sh"
case "${1:-}" in
    simulator)
        [[ "$#" == 1 ]] || ipad_fail "usage: build.sh simulator"
        IPAD_SDK=iphonesimulator; IPAD_TARGET=aarch64-apple-ios-sim
        IPAD_DESTINATION='generic/platform=iOS Simulator'
        ;;
    device)
        [[ "$#" == 2 && "$2" == --unsigned ]] || ipad_fail "usage: build.sh device --unsigned"
        IPAD_SDK=iphoneos; IPAD_TARGET=aarch64-apple-ios
        IPAD_DESTINATION='generic/platform=iOS'
        ;;
    *) ipad_fail "usage: build.sh simulator | device --unsigned" ;;
esac
ipad_preflight
IPAD_DERIVED="$IPAD_REPO/target/ipad-xcode/$1"
# SDKROOT is needed by native Cargo linking, but Xcode selects its SDK from -sdk.
unset SDKROOT
xcodebuild -project "$IPAD_DIR/FigcraftIPad.xcodeproj" -scheme FigcraftIPad \
    -configuration Debug -sdk "$IPAD_SDK" -destination "$IPAD_DESTINATION" \
    -derivedDataPath "$IPAD_DERIVED" ARCHS=arm64 ONLY_ACTIVE_ARCH=YES \
    CODE_SIGNING_ALLOWED=NO build
IPAD_APP="$IPAD_DERIVED/Build/Products/Debug-$IPAD_SDK/FigcraftIPad.app"
[[ -x "$IPAD_APP/FigcraftIPad" ]] || ipad_fail "linked app executable is missing: $IPAD_APP"
if [[ "$1" == simulator ]]; then
    ipad_require codesign
    codesign --force --sign - "$IPAD_APP"
fi
printf 'Built %s\n' "$IPAD_APP"
