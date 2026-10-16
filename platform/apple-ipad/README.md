# Native iPad services

This private bridge is a separate Rust workspace so its reviewed UIKit interop stays outside the shared workspace's unsafe-code prohibition. D07 in the local iPad execution plan records the picker exception. D11 records the separate lifecycle exception. Only src/native.rs and src/lifecycle_native.rs allow unsafe code; src/callback.rs and src/lifecycle.rs contain portable panic-containment and lease-ownership tests. The crate creates no event loop or renderer.

Host must be created and used on the main thread. Requests contain owned paths and values. A request's resource owner survives native presentation and dismissal, including Host destruction during presentation. Poll results on the UI thread; callbacks send messages and request repaint without accessing editor state. The app adapter promotes successful imports before delivering their paths and retains captured exports through native termination.

Imports and exports use the system picker's copy mode. Imports coordinate the read and balance security-scoped access. Unknown extension types fall back to public.data and are checked after selection. Folder export preserves relative paths. Provider failures that keep the picker visible remain pending until the system UI resolves them or the user cancels. Native Save As is outside this API.

Run packaging/ipad/check-bridge.sh simulator and packaging/ipad/check-bridge.sh device from the repository root with DEVELOPER_DIR pointing to Xcode. These check bridge formatting, portable callback tests and target-specific Clippy because root host CI excludes this workspace. The app's portable file and recovery coordinator tests remain in root CI.

Simulator evidence does not qualify physical lifecycle or third-party providers. Coordinated imports currently copy synchronously on the main thread; large-file responsiveness requires measurement before qualification.

## Recovery lifecycle (T11)

LifecycleHost retains main-thread notification observers, a polling timer and one bounded UIKit background task. The safe lifecycle module defines checkpoint requests, progress and terminal events. WillResignActive and DidEnterBackground start checkpoint capture and polling with a fixed five-second deadline. DidBecomeActive ends the current lease and requests foreground work. WillTerminate requests a nonblocking best-effort checkpoint. eframe/winit continue to own the event loop and graphics lifecycle.

Native blocks capture numeric owner and cycle IDs. Expiration ends the native lease immediately, including during a reentrant Rust callback; completion notices wait until the active callback returns. Host destruction removes observers, invalidates the timer and ends the remaining task. Workers and completed recovery files remain owned by the shared recovery package after a deadline or host drop. This does not guarantee another callback or a completed write before process termination.
