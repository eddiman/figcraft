# T13 development checkpoint

These local copies pin egui, egui-winit and eframe 0.36.2 and winit 0.30.13.
Their upstream licenses remain alongside their sources. The patches add owned
clipboard outcomes, confirmed writes and native keyboard routing for iOS.
The app uses one revocable clipboard owner and the existing Native coordinator.

The native `eframe/figcraft-ipad-focus` opt-in now consumes ordered raw-input
batches before normalization and egui aggregation. It limits work to eight
batches per redraw, retains unconsumed input in the existing pending buffer,
and does not normalize hidden-window input twice. Ordinary desktop/web paths
remain single-update. The helper also compiles without the clipboard feature.
Seven new app-host regressions reproduce and repair multi-click field ownership,
cross-field writer ordering, Escape and suffix/metadata preservation; one runs
through the actual Native panel normalizer. Full CI passes 2,545 tests with nine
ignored, including 37 app-host tests. The latest linked simulator app renders.
Interactive batching, lifecycle and hardware qualification remain open.

Six new headless tests now call the actual eframe update and logic-only methods.
They verify redraw-budget/suffix handling, hidden/resumed normalization, retained
suffixes across hidden ticks, ordered output merging and accepted/cancelled close
behavior. Test builds select the iOS focus branch without changing desktop
production behavior. `packaging/ipad/tests/ipad_host_input.py` runs these tests
from the original source through an isolated manifest and checks dependency
provenance. Both native bridge check targets include it. Formatting and the
changed host/test code's lint checks pass; upstream vendored Clippy warnings
outside those files remain visible. Physical and interactive qualification is
still required.

Current-source revalidation also passes an eight-case eframe compile matrix:
neither opt-in, focus only, clipboard only and both on the simulator and device
targets. An isolated harness uses these production path patches and a lockfile
seeded from the root lockfile. This closes the earlier feature-off compile gap;
it does not qualify interactive input or hidden/resumed behavior. The external
T13 result records the harness and resume-eframe-root-lock-matrix.log evidence.

The latest follow-up validates receipt identities before accepting Copy/Cut and
records image-writer errors without losing denial causes to reentry. Owned image
validation borrows pixels rather than cloning them. Ten app-host regressions in
`apps/figcraft-ipad/src/clipboard_tests.rs` exercise the production opt-in features;
they do not substitute for real hardware input, IME or multi-click qualification.
The excluded clipboard crate now normalizes supported ICC/PNG metadata to sRGB
and has eleven tests. The existing bridge check script includes it.

This checkpoint is incomplete. Production simulator rendering and host/native
checks pass, but interactive clipboard and hardware qualification remain open.
Do not treat the presence of these sources as T13 acceptance.

On 2026-10-18, the production follow-up passed `cargo xtask ci` (2,528 tests,
nine ignored), including workspace Clippy, layers, assets and WASM/plugins.
The shared UI passed 581 tests with one ignored; the standalone clipboard crate
passed four tests and Clippy on both iOS targets. Both app target checks and the
simulator linked build passed. An isolated installation of the production
executable displayed the shared synthetic editor. This is startup/rendering
evidence, not interactive Copy/Paste or physical-device evidence.

The follow-up requires acknowledged native Copy as PNG before displaying success
and checks RGBA expansion bounds before decoding pixels. Regression tests reproduce
both failures. The standalone clipboard lockfile now uses its actual package name;
no dependency versions changed. Source and checks are recorded in the external
port plan's T13 result and `ipad-port-evidence/T13/production-*.log`.

The later color/callback checkpoint passes full CI with 2,538 tests and nine
ignored, including 30 app-host tests. Its evidence uses the `color-focus-*` and
`color-tests-final.log` names under the same evidence directory.

On 2026-10-18, workspace formatting and Clippy passed after correcting two new
test-module lint errors. The subsequent cargo xtask ci run passed the engine
suite (934 passed, four ignored), then reported multiple server test failures
and exceeded the 120-second command timeout. Full CI did not pass, and the later
WASM/layer/asset checks were not established by that run. Isolated simulator
clipboard evidence predates this production integration and remains separate.

The initial source-manifest.json records copied input hashes before integration
formatting; it is not a validator for the subsequently edited source tree.
