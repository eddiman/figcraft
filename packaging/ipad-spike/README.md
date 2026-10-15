# T02 simulator host experiment

This directory supports an unapproved native-host experiment. It is separate from the T03 production target. The Rust binary reuses `FigApp`, the existing engine and the pinned eframe/Wgpu renderer. The spike adds a normal egui field above the editor and uses the T01 small synthetic recipe. It has no native bridge or new unsafe code. `NoServices` provides no platform persistence, clipboard or picker implementation.

The current machine has Xcode inside `/Volumes/Mandalor/XcodeDev.sparsebundle`, mounted read-only at `/Volumes/XcodeDev`. Xcode is `/Volumes/XcodeDev/Xcode-26.3.0.app`. Set the developer directory for each native build without changing `xcode-select`. The SDKs report 26.2; the installed simulator runtime reports iOS 26.3. The experimental minimum deployment version is 17.0 and has not been qualified on older systems.

Run from the repository root after the image is mounted and the Rust simulator target is installed:

```sh
export DEVELOPER_DIR=/Volumes/XcodeDev/Xcode-26.3.0.app/Contents/Developer
export SDKROOT="$(xcrun --sdk iphonesimulator --show-sdk-path)"
export CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER="$DEVELOPER_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang"
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
cargo build --locked -p figcraft-ipad-spike --target aarch64-apple-ios-sim
```

Both SDKROOT and the Xcode linker are explicit: the first link attempt with the default `cc` used a macOS sysroot and could not locate UIKit. Do not set the simulator SDKROOT for host-only CI commands.

The experiment packages the linked Rust executable directly in a simulator app bundle. There is no exported C entry point and no call to UIApplicationMain before winit. No Xcode project or native-language glue is introduced.

```sh
mkdir -p target/ipad-spike/FigCraftSpike.app
install -m 755 target/aarch64-apple-ios-sim/debug/figcraft-ipad-spike target/ipad-spike/FigCraftSpike.app/figcraft-ipad-spike
cp packaging/ipad-spike/Info.plist target/ipad-spike/FigCraftSpike.app/Info.plist
codesign --force --sign - target/ipad-spike/FigCraftSpike.app
xcrun simctl install BCCE5B8D-96D3-45C1-84DD-3E0C84C9AE99 target/ipad-spike/FigCraftSpike.app
xcrun simctl launch BCCE5B8D-96D3-45C1-84DD-3E0C84C9AE99 org.figcraft.ipad-spike
```

The recorded destination is the existing iPad Air 11-inch (M3) simulator, UDID `BCCE5B8D-96D3-45C1-84DD-3E0C84C9AE99`. It must be booted before launch. The identifier `org.figcraft.ipad-spike` is a local experimental identifier. Ad hoc simulator signing uses no account, provisioning profile or distribution identity.

Simulator observations and failures are recorded in the workspace's `ipad-port-plan/execution/results/T02.md` and `ipad-port-evidence/T02/`. A successful simulator run does not qualify physical input, memory, thermal or lifecycle behavior. T02 remains unpassed until its device evidence and architecture review are complete. No T03 production implementation is released by this experiment.

The simulator adapter reports 15 inter-stage shader variables; eframe initially requested 16 and exited. The spike constrains that one requested limit to the adapter capability. Startup logs retain this failure and the successful retry. The ordinary field accepted `iPad test` with Simulator keyboard capture enabled. Disable capture after testing.
