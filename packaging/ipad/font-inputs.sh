#!/bin/bash
# Sourced by cargo-build.sh after IPAD_REPO is established.
# The host validator must not inherit the native SDK or required-font build flag.
ipad_font_task() (
    unset SDKROOT CRAFT_FONTS_REQUIRED FIGCRAFT_EMBED_CJK CARGO_BUILD_TARGET
    cd "$IPAD_REPO" || ipad_fail "cannot enter repository directory: $IPAD_REPO"
    cargo xtask ipad-fonts "$@"
)

ipad_prepare_fonts() {
    IPAD_HAS_FONT_PROFILE=false
    # Match .cargo/config.toml rather than allowing Cargo to discover an
    # unchecked optional checkout that this packaging layer did not see.
    export CRAFT_FONTS_DIR="${CRAFT_FONTS_DIR-$IPAD_REPO/../craft-fonts}"
    if [[ -n "${CRAFT_FONTS_DIR:-}" && -f "$CRAFT_FONTS_DIR/fonts/manifest.txt" ]]; then
        CRAFT_FONTS_DIR="$(cd "$CRAFT_FONTS_DIR" && pwd -P)" || ipad_fail "cannot resolve craft-fonts directory"
        ipad_font_task check "$CRAFT_FONTS_DIR" || ipad_fail "font profile validation failed"
        export CRAFT_FONTS_REQUIRED=1
        export FIGCRAFT_EMBED_CJK=1
        IPAD_HAS_FONT_PROFILE=true
    elif [[ "${CONFIGURATION:-Debug}" == Release || "${FIGCRAFT_IPAD_REQUIRE_FONTS:-0}" == 1 || -n "${CRAFT_FONTS_REQUIRED+x}" ]]; then
        ipad_fail "required font inputs missing; set CRAFT_FONTS_DIR to the pinned checkout in maintenance/ipad-fonts.json"
    fi
}

ipad_bundle_fonts() {
    local bundle="$1"
    # Remove only artifacts owned by this build recipe, including a prior profile.
    rm -rf "$bundle/FontLicenses"
    rm -f "$bundle/ipad-fonts.json"
    if "$IPAD_HAS_FONT_PROFILE"; then
        ipad_font_task bundle "$CRAFT_FONTS_DIR" "$bundle" || ipad_fail "cannot bundle verified font licenses"
    fi
}
