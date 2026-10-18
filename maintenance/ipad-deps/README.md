# T13 development checkpoint

These local copies pin egui, egui-winit and eframe 0.36.2 and winit 0.30.13.
Their upstream licenses remain alongside their sources. The patches add owned
clipboard outcomes, confirmed writes and native keyboard routing for iOS.
The app uses one revocable clipboard owner and the existing Native coordinator.

This checkpoint is incomplete. The production integration has not completed its
iOS build/runtime qualification. Physical-device qualification remains deferred.
Do not treat the presence of these sources as T13 acceptance.

On 2026-10-18, workspace formatting and Clippy passed after correcting two new
test-module lint errors. The subsequent cargo xtask ci run passed the engine
suite (934 passed, four ignored), then reported multiple server test failures
and exceeded the 120-second command timeout. Full CI did not pass, and the later
WASM/layer/asset checks were not established by that run. Isolated simulator
clipboard evidence predates this production integration and remains separate.

The initial source-manifest.json records copied input hashes before integration
formatting; it is not a validator for the subsequently edited source tree.
