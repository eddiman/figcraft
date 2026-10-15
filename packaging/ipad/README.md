# iPad development target

The app reuses the shared engine, editor and pinned eframe/Wgpu host approved for development in D02 revision 3. Rust main starts the only event loop. The Xcode target invokes Cargo to link the executable and processes the bundle without introducing another UIApplication entry point. Host workspace builds compile an iOS-only notice instead of the native application module; the app's dependencies are conditional on target_os = ios.

## Prerequisites

Install full Xcode, its iOS SDKs and the Rust targets `aarch64-apple-ios` and `aarch64-apple-ios-sim` explicitly. Fetch locked Cargo dependencies before building. Build scripts run Cargo offline and do not install anything. Missing Xcode, SDKs, Rust targets or cached dependencies fail the build.

On the current machine Xcode lives in `/Volumes/Mandalor/XcodeDev.sparsebundle`, mounted read-only at `/Volumes/XcodeDev`. Set this per terminal session without changing the global developer directory:

```sh
export DEVELOPER_DIR=/Volumes/XcodeDev/Xcode-26.3.0.app/Contents/Developer
```

The tested toolchain is Rust 1.98.1, Xcode 26.3 (17C529), iOS SDKs 26.2 and simulator runtime iOS 26.3 (26.3.1, 23D8133). The development deployment floor is iPadOS 17.0. No runtime compatibility with older versions has been qualified. The local development identifier is `org.figcraft.ipad-development`; distribution identity and branding remain T29 work.

## Builds

Run from the repository root:

```sh
packaging/ipad/build.sh simulator
packaging/ipad/build.sh device --unsigned
```

Both commands use the shared `FigcraftIPad` scheme in `FigcraftIPad.xcodeproj`, arm64 and Debug Cargo builds. Artifacts remain separate:

- Simulator: `target/ipad-xcode/simulator/Build/Products/Debug-iphonesimulator/FigcraftIPad.app`.
- Device: `target/ipad-xcode/device/Build/Products/Debug-iphoneos/FigcraftIPad.app`.

The simulator bundle receives a local ad hoc signature. The device command disables Xcode signing and creates no provisioning profile or distribution archive. Its linked output is not installable device-test evidence. No signing team or account is configured.

The build phase selects the corresponding SDKROOT and Xcode clang for each Rust target. It fixes the deployment floor and keeps build debug information small. Xcode's script sandbox is disabled for the trusted Cargo phase because Cargo reads the checkout and its dependency cache. Rust workspace safety lints remain inherited and unchanged. The phase never invokes UIKit directly or exports an FFI entry point.

The Release configuration is not a release-qualification path: the scheme and scripts intentionally use the development Cargo build. Distribution/archive support remains separately gated.

## Simulator smoke

The default app opens a shared blank editor with NoServices. Native file pickers, persistence and clipboard adapters have not been added. Pass `--smoke` for the deterministic T01 small fixture and ordinary egui field; the Xcode launch scheme enables this argument for development.

```sh
xcrun simctl install BCCE5B8D-96D3-45C1-84DD-3E0C84C9AE99 target/ipad-xcode/simulator/Build/Products/Debug-iphonesimulator/FigcraftIPad.app
xcrun simctl launch BCCE5B8D-96D3-45C1-84DD-3E0C84C9AE99 org.figcraft.ipad-development --smoke
```

The recorded destination is the existing iPad Air 11-inch (M3). Boot it first, or select an available iPad UDID from `xcrun simctl list devices available`. A phone destination is not an iPad smoke test.

Verify the visible shared editor, enter text with Simulator keyboard capture enabled, select Fixture image, then use the smoke-only “Set fixture X = 10” button. It dispatches the existing node.set command; verify X=10 in the shared inspector. Automated numeric-field typing was inconsistent and is not counted as passing. Rotate with Device → Rotate Left. Use Home, relaunch the existing app and check that the same process retains field and document state. Disable Simulator keyboard capture afterward.

The host constrains one requested graphics limit to the adapter's supported value because the simulator reports 15 inter-stage shader variables while eframe defaults to 16. Native startup errors can be logged before eframe exits with code zero, so a successful simctl launch is insufficient evidence. Inspect the visible editor/state and startup logs.

The temporary spike sources were removed after T03 simulator parity checks. The reviewed experiment remains recoverable at commit f9cbfb9. Canvas glyph coverage, touch drag, IME and on-screen keyboard behavior remain unqualified. The absent optional craft-fonts checkout is recorded; a readable egui toolbar does not establish canvas font availability.

## Checks

```sh
cargo check --locked -p figcraft-ipad --target aarch64-apple-ios
cargo check --locked -p figcraft-ipad --target aarch64-apple-ios-sim
python3 packaging/ipad/tests/ipad_build_prerequisites.py
cargo xtask ci
```

The prerequisite checks simulate unavailable tools/targets through temporary wrappers rather than uninstalling anything. Physical-device checks are deferred by the user; simulator passes do not certify Pencil, VoiceOver, thermal/memory behavior or ten physical suspend/resume cycles. T03 creates development builds, not a qualified alpha or release candidate.
