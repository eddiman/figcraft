# iPad development target

The app reuses the shared engine, editor and pinned eframe/Wgpu host approved for development in D02 revision 3. Rust main starts the only event loop. The Xcode target invokes Cargo to link the executable and processes the bundle without introducing another UIApplication entry point. Host workspace builds compile an iOS-only notice instead of the native application module; the app's dependencies are conditional on target_os = ios.

The app enables the pinned host's `figcraft-ipad-focus` feature. Native redraws
consume up to eight ordered input batches before rendering their combined
output; the existing pending buffer retains any suffix for the next redraw.
This preserves field ownership before egui aggregates clicks and keeps the
existing Native coordinator as the only input owner. Hidden logic-only ticks
do not run the mutating native input hook. Desktop/web behavior is unchanged.
Host regressions and startup rendering pass; interactive input backlogs and
hidden/resumed canvas behavior still need qualification.

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

The default app opens a shared blank editor with the iPad capability provider. Native copy pickers support import and export. Recovery uses completed checkpoints under Library/Application Support/FigCraft/Recovery, adopts previous-run copies at startup, and requests a bounded checkpoint when the application becomes inactive. Owned local Save/Save As is implemented as described below. The development clipboard adapter handles owned text and images, with interactive and physical qualification still open. The provider explicitly disables multiple windows, reveal, local hosting and plugin downloads; unsupported requests return Unsupported. Pass `--smoke` for the deterministic T01 small fixture and ordinary egui field; the Xcode launch scheme enables this argument for development.

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
python3 packaging/ipad/tests/ipad_host_input.py
cargo xtask ci
```

The host-input runner exercises the actual vendored eframe update loop with a
headless Frame, including the redraw budget, suffix ordering, hidden/resumed
normalization, combined outputs and close cancellation. Both check-bridge.sh
targets invoke it. It requires six passing tests, checks formatting, and runs
Clippy with diagnostics in the changed host/test files treated as failures.
Existing vendored Clippy warnings elsewhere are printed, not suppressed; strict
standalone eframe Clippy still has those baseline failures.

Because eframe is excluded from root workspace tests, the runner creates a
temporary manifest pointing directly at its original library source. It omits
the unused inspection dependency, copies the root lockfile and development
profiles, and uses the production local dependency patches. Active runtime
versions must match the root lockfile; dev-only versions must appear in either
the root or vendored eframe lockfile. Checks run offline and tests use the
resolved temporary lockfile with --locked. No production manifest/lockfile is
rewritten, and the temporary directory is removed on exit. These are headless
host-loop tests, not simulator interaction or physical lifecycle qualification.

The prerequisite checks simulate unavailable tools/targets through temporary wrappers rather than uninstalling anything. Physical-device checks are deferred by the user; simulator passes do not certify Pencil, thermal/memory behavior or ten physical suspend/resume cycles. VoiceOver, contrast and reduced-motion requirements are excluded from the iPad port scope and acceptance gates. Existing shared code, labels, keyboard navigation and UI sizing requirements remain. T03 creates development builds, not a qualified alpha or release candidate.

## Automated simulator tests and CI (T23)

```sh
python3 packaging/ipad/tests/ipad_simulator_runner.py
packaging/ipad/test.sh simulator
# Optional explicit destination and a NEW evidence directory:
packaging/ipad/test.sh simulator --udid IPAD_UDID --output target/ipad-smoke
```

The runner requires Apple Silicon, full Xcode with XCTest/Vision and an available
iPad simulator running iOS 17 or newer. It uses the simulator device type, not
the editable display name, to reject phones. It prefers an already booted iPad,
records its UDID/runtime and shuts down only a destination it booted itself.
It fails if no supported iPad exists; it never installs a runtime automatically.

Each run builds the current Rust app and a separate Swift XCTest UI-test bundle.
The Swift code is test infrastructure only, using public XCTest input and Vision
recognition of the actual rendered egui screen. There is no production test
command channel or new native/accessibility bridge. The app and runner receive
unique bundle identifiers, use fresh containers and are uninstalled on exit.
The development app's container and clipboard are not used. Do not run parallel
simulator checks against the same checkout.

The smoke workflow covers startup, the generated small fixture, field typing,
the existing smoke control's `node.set` edit and cold restart. This is a bounded
simulator workflow, not simultaneous-touch, Pencil, software-keyboard/IME,
physical lifecycle or general font qualification.

Evidence is retained in a unique `target/ipad-tests/run-*` directory, or the
new directory supplied with `--output`: source/toolchain/SDK inputs, build and
test logs, destination metadata, `.xcresult`, JSON test summary and exported
screenshots/attachments. Copied app bundles are cleaned up. A passing process
exit alone is insufficient: a nonzero passing test count and no failed, skipped
or expected-failure tests are required.

To prove failure propagation, run `test.sh simulator --test-failure --output
target/ipad-negative`. It must fail. Then run
`python3 packaging/ipad/tests/ipad_simulator_runner.py --verify-failure
target/ipad-negative/summary.json` to distinguish the deliberate assertion from
an unrelated build/startup failure.

`.github/workflows/ipad-ci.yml` adds separate unsigned device/simulator jobs on
the standard Apple Silicon `macos-15` runner, with Rust 1.98.1 and Xcode 26.3.
Caches include compiler/SDK/target/lockfile inputs; PRs only restore caches.
The jobs fetch locked dependencies before the existing offline build scripts,
check both iOS targets, link both app variants and run the positive/negative
simulator checks. They require no signing credentials or release environment.
Existing upstream Linux/wasm jobs are unchanged. The workflow is restricted to
downstream repositories to preserve the original project's macOS runner policy.
The authorized fork is `eddiman/figcraft` (`origin`); the original repository is
preserved as `upstream`. Remote CI remains configured, not verified, until the
workflow executes in the fork; local evidence does not certify hosted-runner
execution.

## Native copy services (T07)

The app installs IPadServices from apps/figcraft-ipad/src/services.rs. Open/import actions copy the selected input into a unique persistent Library/Application Support/FigCraft/Imports directory before the engine receives a path. Equal display names do not overwrite an earlier copy. Exporting a batch packages its declared relative paths into one staged folder named Export, then asks the system to copy that folder. Single exports copy their staged files.

### Local Save and Save As (T10)

File > Save writes an existing owned .figcraft document. An untitled document or
an imported non-.figcraft file opens the local-copy naming dialog instead.
Save As always creates a new owned copy; it does not select a Files destination
or overwrite the imported original. Use Export Copy for an external copy.

The editor captures the document when Save/Save As is activated, before naming.
A bounded worker encodes and synchronizes the archive off the UI thread.
Only successful acknowledgement clears the matching saved baseline; edits made
after capture remain dirty. Accepted saves survive tab closure. Relative recents
resolve against the current app container at launch.

Simulator Save As, edited Save and recents reopening after relaunch are verified.
Physical-device cancellation, interrupted writes and real disk/sync failures
remain in review. Host destruction before UI completion does not guarantee a
recents entry for a drained write; recovery remains a separate mechanism.

The reviewed bridge lives in platform/apple-ipad, an isolated Rust workspace. Its private native.rs contains the D07 UIKit exception; shared/app safety lints remain unchanged. Root host CI does not run its standalone checks. With DEVELOPER_DIR set to the existing Xcode installation, run packaging/ipad/check-bridge.sh simulator and packaging/ipad/check-bridge.sh device in addition to the normal build checks. These commands test callback containment on the host and lint the target-specific bridge. They do not claim physical-device execution.

The production Info.plist does not enable document-browser or in-place editing/file-sharing flags. Files are opened as copies. The isolated simulator probe used separate file-sharing flags only to expose its own synthetic fixtures; those flags are not part of this application.

## Recovery development checks (T11)

The app starts shared recovery with Synced filesystem durability, schedules foreground checkpoints through the UI clock and installs the D11 lifecycle observer. Startup opens completed previous-run copies as unsaved documents before displaying the editor. A checkpoint does not acknowledge collaboration changes or count as a user save. Native termination starts a nonblocking attempt and preserves completed copies.

A UIKit background task allows polling for up to five seconds while the app becomes inactive. Capture retries when the editor is already borrowed. Completion, failure, deadline, expiration, resume and host destruction release the lease. A failed or expired attempt keeps preceding recovery copies and reports a notice on the next available UI frame. Large snapshot capture latency and physical suspension behavior still require measurement.

Run cargo test --locked -p figcraft-ipad for the portable coordinator and file-resource tests. Run check-bridge.sh for both targets because root CI does not cover native bridge code. Simulator notifications and injected expiration can exercise callback ownership, but physical termination/relaunch and ten device suspend/resume cycles remain deferred qualification checks.


## Embedded font profile (T12)

Development builds may omit craft-fonts; the app then reports that document fonts
are unavailable. iOS does not search desktop Unix font directories. Latin UI text
continues to use egui's bundled fonts.

For document text and script fallbacks, use the exact revision and seven files
in `maintenance/ipad-fonts.json`, from the external craft-fonts repository. Set
`CRAFT_FONTS_DIR` to that checkout's absolute path before building. The native
recipe validates the commit, primary manifest, font hashes and license hashes,
embeds CJK as well as the other declared fonts, and includes OFL texts and the
profile in the app bundle. It never creates a missing Latin manifest. Run
`cargo xtask ipad-fonts check /absolute/path/to/craft-fonts` to check inputs alone.

Set `FIGCRAFT_IPAD_REQUIRE_FONTS=1` to reject absent inputs in a development build;
Xcode Release configurations require them automatically. Current build.sh targets
still produce development Debug bundles. An existing but mismatched input set
fails validation rather than being accepted as the pinned profile. Other hosts
retain their existing optional two-manifest build recipe.

The pinned revision has no Inter files. Requested document font names are retained;
iOS prefers BIZ UDPGothic Regular when a requested primary font is unavailable.
Its metrics can change line wrapping and text bounds. Compare desktop/native text
using the same explicit BIZ UDPGothic font before assessing rendering parity.
UI language availability for CJK requires the actual embedded sample glyphs.
Device text/render qualification remains deferred.

## Clipboard development checks (T13)

The shell installs one revocable general-pasteboard owner without reading or
seeding the clipboard at startup. Explicit Paste returns one owned outcome.
Native Copy as PNG reports success only after its writer acknowledges the image;
denial or unavailability is shown instead of a success message. Desktop/browser
adapters retain their existing output-based behavior.

The excluded `platform/apple-ipad-clipboard` crate requires its own checks:

```sh
cargo test --locked --manifest-path platform/apple-ipad-clipboard/Cargo.toml
cargo clippy --locked --manifest-path platform/apple-ipad-clipboard/Cargo.toml \
    --all-targets --target aarch64-apple-ios-sim -- -D warnings
cargo clippy --locked --manifest-path platform/apple-ipad-clipboard/Cargo.toml \
    --all-targets --target aarch64-apple-ios -- -D warnings
```

The clipboard decoder checks encoded size and expanded RGBA dimensions before
pixel decoding, then applies decoder allocation limits. Simulator startup and
host regressions do not qualify tap-to-focus, hardware shortcuts, paste permission
prompts, Universal Clipboard, or IME.

Supported PNG/JPEG RGB and grayscale ICC profiles are converted to straight sRGB
RGBA. PNG gamma/primaries and SDR sRGB/Display P3 cICP metadata are supported.
Malformed/discarded profiles and unsupported HDR fail instead of silently changing
colors. ICC metadata has a separate 4 MiB budget. Numerical conversion, alpha and
malformed-input checks use generated synthetic images; real-device interoperability
still needs qualification. moxcms and png retain their existing pinned versions.

`packaging/ipad/check-bridge.sh simulator` and `device` now run the standalone
clipboard checks as well as the document/lifecycle bridge checks.
`cargo test --locked -p figcraft-ipad` also runs ten production-feature callback
and field-focus regressions. These host tests include invalid-receipt Cut
preservation, cross-thread reentry, two paste outcomes and Escape after a tap.
