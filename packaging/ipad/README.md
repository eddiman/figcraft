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

The default app opens a shared blank editor with the iPad capability provider. Native copy pickers support import and export. Recovery uses completed checkpoints under Library/Application Support/FigCraft/Recovery, adopts previous-run copies at startup, and requests a bounded checkpoint when the application becomes inactive. Native Save and clipboard adapters remain pending. The provider explicitly disables multiple windows, reveal, local hosting and plugin downloads; unsupported requests return Unsupported. Pass `--smoke` for the deterministic T01 small fixture and ordinary egui field; the Xcode launch scheme enables this argument for development.

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

## Native copy services (T07)

The app installs IPadServices from apps/figcraft-ipad/src/services.rs. Open/import actions copy the selected input into a unique persistent Library/Application Support/FigCraft/Imports directory before the engine receives a path. Equal display names do not overwrite an earlier copy. Exporting a batch packages its declared relative paths into one staged folder named Export, then asks the system to copy that folder. Single exports copy their staged files. Native Save/Save As remain unsupported until durable persistence is connected by T10.

The reviewed bridge lives in platform/apple-ipad, an isolated Rust workspace. Its private native.rs contains the D07 UIKit exception; shared/app safety lints remain unchanged. Root host CI does not run its standalone checks. With DEVELOPER_DIR set to the existing Xcode installation, run packaging/ipad/check-bridge.sh simulator and packaging/ipad/check-bridge.sh device in addition to the normal build checks. These commands test callback containment on the host and lint the target-specific bridge. They do not claim physical-device execution.

The production Info.plist does not enable document-browser or in-place editing/file-sharing flags. Files are opened as copies. The isolated simulator probe used separate file-sharing flags only to expose its own synthetic fixtures; those flags are not part of this application.

## Recovery development checks (T11)

The app starts shared recovery with Synced filesystem durability, schedules foreground checkpoints through the UI clock and installs the D11 lifecycle observer. Startup opens completed previous-run copies as unsaved documents before displaying the editor. A checkpoint does not acknowledge collaboration changes or count as a user save. Native termination starts a nonblocking attempt and preserves completed copies.

A UIKit background task allows polling for up to five seconds while the app becomes inactive. Capture retries when the editor is already borrowed. Completion, failure, deadline, expiration, resume and host destruction release the lease. A failed or expired attempt keeps preceding recovery copies and reports a notice on the next available UI frame. Large snapshot capture latency and physical suspension behavior still require measurement.

Run cargo test --locked -p figcraft-ipad for the portable coordinator and file-resource tests. Run check-bridge.sh for both targets because root CI does not cover native bridge code. Simulator notifications and injected expiration can exercise callback ownership, but physical termination/relaunch and ten device suspend/resume cycles remain deferred qualification checks.
