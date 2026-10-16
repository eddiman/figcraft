# Native iPad copy picker

This private bridge is a separate Rust workspace so its reviewed UIKit interop stays outside the shared workspace's unsafe-code prohibition. D07 in the local iPad execution plan records the exact exception. Only src/native.rs allows unsafe code; src/callback.rs contains portable panic-containment tests. The crate creates no event loop or renderer.

Host must be created and used on the main thread. Requests contain owned paths and values. A request's resource owner survives native presentation and dismissal, including Host destruction during presentation. Poll results on the UI thread; callbacks send messages and request repaint without accessing editor state. The app adapter promotes successful imports before delivering their paths and retains captured exports through native termination.

Imports and exports use the system picker's copy mode. Imports coordinate the read and balance security-scoped access. Unknown extension types fall back to public.data and are checked after selection. Folder export preserves relative paths. Provider failures that keep the picker visible remain pending until the system UI resolves them or the user cancels. Native Save As is outside this API.

Run packaging/ipad/check-bridge.sh simulator and packaging/ipad/check-bridge.sh device from the repository root with DEVELOPER_DIR pointing to Xcode. These check bridge formatting, portable callback tests and target-specific Clippy because root host CI excludes this workspace. The app's four portable file-lifecycle tests remain in root CI.

Simulator evidence does not qualify physical lifecycle or third-party providers. Coordinated imports currently copy synchronously on the main thread; large-file responsiveness requires measurement before qualification.
