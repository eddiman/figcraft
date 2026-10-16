# Local iPad port development log

## 2026-10-16: T33 owned export trees

The T06 audit found that batch exports can create icons/close.png, while the T05 dispatcher accepted only single filenames. T33 adds ExportTree beside ExportFiles, retaining a staging lease and validating a bounded set of relative paths. The desktop adapter copies the declared tree into the chosen folder and reports errors through the existing completion route. Existing picker call sites are unchanged until T06.

Tests exercise actual engine-generated nested PNG exports, regular-file overwrite, invalid paths and bounds, case-insensitive duplicate and prefix conflicts, cancellation, unsupported defaults, source preflight, destination conflicts, source/destination symlinks, cleanup and delayed UID routing. A failing self-copy test exposed macOS /var aliases; canonical root comparison fixes it. Filesystem checks reject observed links but do not lock ancestors against concurrent replacement. Tree copying is nontransactional. No native provider, persistence semantics, dependency, asset or device evidence is added.

The external execution backlog records D04 revision 3, T33 evidence and the remaining T06 migration. Physical-device checks remain deferred for development and required at qualification gates.

## 2026-10-16: T06 shared file-dialog migration

Open, Import, Place Image, Save As, layer exports, variable import/export, library import, plugin manifest import, image fills and experimental .fig export now submit owned platform requests. Encoding runs before export presentation, so later edits and tab switches cannot change the staged outputs. Imports and completions use the initiating UID. Native Save/Save As remain unsupported until durable persistence is connected; a picker result cannot mark a native document saved.

Image-fill replies retain the document snapshot and reject a changed document or paint. An initial revision-counter guard incorrectly rejected a reply after opening another tab because command bookkeeping increments that counter; the snapshot guard preserves tab switching. Desktop manifest import retains its companion module path. Other document modes reject local manifests before a picker opens. Shared command dispatch checks local hosting, plugin download and native save policy; existing single-window tab fallback remains intact.

The new regression tests exercise delayed replies with actual engine imports, PNG exports, variables, image paints and an original minimal plugin module. A synchronous desktop fake verifies that one completed import executes once and cancelled Open leaves the document intact. Full UI localization coverage also preserves the Export variables picker title. Evidence and remaining native/physical gates are recorded in the external execution backlog.

## 2026-10-16: T07 native import and export copies

The iPad service now presents UIKit document pickers on the main thread and returns owned results through the shared dispatcher. Imports receive unique persistent sandbox directories and are promoted before the engine sees their paths. Cancelled or failed imports remove their unclaimed directories. Export requests retain their captured artifacts until native dismissal completes; nested exports are packaged into one folder without flattening relative paths. The UI explains that Open imports a copy and Export creates a copy in Files.

D07 reviews the isolated platform/apple-ipad bridge and its private native module. Shared and app crates retain their unsafe-code prohibition. The bridge owns UIKit delegates, security-scoped access, coordinated reads and presentation/dismissal lifetimes. Native callbacks queue results and repaint requests without borrowing the editor. Host shutdown during presentation defers cancellation until presentation completes. Native Save As stays unsupported until T10 connects durable saving. The only new external dependency is the pinned objc2-uniform-type-identifiers 0.3.2 binding; existing dependency versions are unchanged.

The production simulator app imported the same synthetic SVG twice into distinct directories, displayed the expected rectangle, exported a 40 by 20 PNG to Files, and reopened that PNG as an image layer with identical bytes. Import cancellation removed the pending directory; export cancellation left the existing output unchanged. The isolated probe also exercised nested folder export and resource release after cancellation and shutdown during presentation. Four file-lifecycle tests and three bridge callback tests passed. Full project CI passed with 2,211 tests and eight ignored; simulator and unsigned device bundles linked, and bridge Clippy passed for both iOS targets.

T07 remains in review because the card requires physical-device evidence. Physical testing remains deferred under the user's earlier instruction. Large-file latency, third-party providers and hardware lifecycle behavior are unqualified. The native read currently copies synchronously within the coordinator callback. The external execution result records this boundary and the remaining checks.

## 2026-10-16: T09 captured file saves and acknowledgement

The engine now prepares immutable captures of the main document, complete history and branches, with open branch tabs overlaid before encoding. A checked save order and retained file incarnation prevent older or closed-file completions from replacing a newer baseline or destination. Acknowledging an earlier successful snapshot preserves later edits, selection and undo. Existing synchronous desktop Save and Save As use the same seam and retain their path/byte-count results and atomic-replacement assurance. Native synced storage remains T10 work.

The format encoder and content comparison now use one archive member builder. It includes raw lazy history JSON, image bytes, the saved ID allocator and published-library snapshots, which existing model equality does not fully compare. Main-page navigation keeps its existing dirty policy; historical and branch snapshot pages remain part of the file. Malformed lazy history JSON fails visibly before writing. Dirty checks use retained roots and complete metadata rather than encoding on every frame. Changed acknowledgements may compare the complete logical archive; large-file timing remains qualification work.

Main-file title, tab and close warnings include branch edits made before a command syncs them. Branches retain their own dirty indicators. Reopening a branch rebases a reconstructed pointer only after the full serialized content proves unchanged. Initial/welcome baseline adjustments cannot overwrite an established local-file baseline, and local saving leaves collaboration pending batches intact. Complete recovery captures and eligibility queries are available for T11; the existing recovery filesystem implementation is not replaced here.

The new regressions exercise delayed saves, history/branch-only edits, direct image insertion, allocator/published-snapshot differences, preview and page behavior, failed writes, replaced/closed tabs, save ordering, local/server independence, actual encoded archive members and shared UI close warnings. The three existing fixture archives retain their member sets and normalized JSON output. No dependency, asset, native interop or file schema changes were added. Full verification and the remaining native/recovery gates are recorded in the external T09 result.

## 2026-10-16: T11 shared recovery implementation

The desktop adapter now uses figcraft-recovery at L6. Complete T09 captures include version history, published-library members and unsynchronized open branches. Each document retains one worker and its latest desired state. Cleanup waits for that worker, and an emergency desktop write drains earlier work before publishing the latest capture. A worker retains the run lock if its host drops. Fresh exclusive run directories avoid PID reuse while legacy flat checkpoints remain readable.

Archive and online-note files publish together through a validated current-generation manifest. Interrupted writes leave the preceding committed generation available. A publication error after rename remains uncertain and cannot authorize old-run cleanup. Recovered sources stay until reconciliation and a successful complete replacement. Flush tickets expose pending, complete and failed outcomes without joining unfinished workers on normal UI paths.

Online reconciliation compares complete serialized local metadata separately from collaboration changes. Empty local categories accept recovered history, branches and published members; equivalent categories remain unchanged. Conflicts retain a complete unsaved side copy, including when the document content matches or recovered metadata was cleared. Incoming collaboration documents omit published-library snapshots, so remote installation preserves the local underlying publication. A follow-up-server-update regression demonstrated the loss before this retention fix. Checkpointing preserves pending server batches. Deterministic regressions exercise worker release, locks, failed replacement, manifest validation, interrupted publication, metadata-only edits and branch overlays. Existing desktop recovery cases remain covered under the new layout.

This is the shared portion of T11. The iPad host does not yet install periodic recovery or native suspension hooks. Lifecycle interop requires its separate reviewed boundary, and physical termination/suspension qualification remains deferred. T11 stays in progress.

Final shared verification passed cargo xtask ci with 2,252 tests passed, zero failed and eight ignored. This includes 889 engine tests (three ignored), 29 desktop tests and 15 recovery-package tests. Both iOS compile targets passed for the app and the new recovery package. Worker-transition diagnostics reproduced two failures before their fixes: an older failure blocked a newer queued capture, and failed deletion left a stale success record. Both now pass. An unreadable legacy note opens its archive as an unsaved file and keeps the original pair for inspection. No native lifecycle behavior is qualified by these checks.

## 2026-10-16: T11 native recovery integration

The iPad app now adopts completed previous-run checkpoints before displaying the editor, schedules foreground recovery through the UI clock, and writes with Synced filesystem durability. The safe coordinator retains a flush ticket while workers run and retries capture when the editor is already borrowed. Emergency saving starts a nonblocking capture. Native termination preserves completed files and worker ownership.

D11 approves the separate private lifecycle_native module. Main-thread notification observers and a 50 ms timer coordinate one UIKit background task with a five-second deadline. Blocks capture numeric owner and cycle IDs. Completion, failure, deadline, expiration, resume and destruction release the native lease. Reentrant expiration releases resources immediately and queues its notice until the app callback returns. Destruction removes observers and invalidates the timer. eframe/winit retain event-loop and graphics ownership. Existing binding versions and collaboration behavior are unchanged.

Full cargo xtask ci passed with 2,258 tests, zero failures and eight ignored. The app's ten portable tests include six recovery coordinator cases; the separate bridge passed eight tests, including five lease cases. Bridge Clippy passed with warnings denied for both iOS targets; the iOS app also passed simulator Clippy. Simulator and unsigned device bundles linked. A synthetic simulator probe using unchanged production lifecycle source passed completion, failure, callback panic, injected reentrant expiration, resume, reentrant destruction and the timer-driven deadline. These injections do not establish operating-system expiration or denial timing on hardware.

The production simulator app displayed the synthetic fixture, changed its image position to X=10, wrote a foreground checkpoint, and restored X=10 as unsaved work after Home, termination and relaunch. Completed replacement checkpoints released the previous run. T11 remains in review until physical termination/relaunch and ten suspend/resume cycles are recorded. Native Save remains gated by T10. The external T11 result contains commands, hashes, screenshots and the next handoff.

## 2026-10-16: T15 local pointer cancellation

The engine now exposes an opt-in pointer checkpoint with a checked generation and initiating UID. It captures editing and tool state before Down while retaining the existing outer text or multi-click pen interaction. Cancelling restores that gesture-start document, selection, undo/redo, caret/preedit, vector mode and transient state. It preserves destination and saved-file metadata. Normal Up still uses the existing tool commit; finish releases the checkpoint and publishes its deferred journal/UI effects once.

External command, text/IME, tool, selection, undo and mutable-document entry points cancel an enrolled pointer before acting. Save capture and acknowledgement use the same boundary. A failing delayed-acknowledgement regression demonstrated why the persistence entry points need that guard. File persistence is refused inside an owned pointer callback. Branch synchronization waits until an owned command is accepted, so cancellation cannot publish a branch preview into its parent file. Independent raw state changes detected through root/revision or selection/text/vector validation are retained and reported instead of overwritten.

Pointer-generated command journal entries and UI requests remain in bounded checkpoint-owned queues until acceptance; cancellation discards them. A late finish cannot publish them to another active tab. Stale and duplicate tokens cannot end a newer checkpoint, and a closed UID cannot reopen a tab. Callback errors and panics restore the owned state and release dispatch ownership. Existing typed text and accepted pen points keep their original undo grouping.

Existing desktop/web pointer routes remain unenrolled. T34 still needs the ordered raw-event and UI drag-owner barriers before T16 activates native touch editing. Low-level public DocState mutation remains a caller responsibility. Attached live sessions, including a host that has no synchronized document ID yet and branches of that file, are refused until T28 reviews collaboration-aware cancellation. No native interop, dependency or file-schema change is introduced. Validation and limits are recorded in the external T15 result.

Final T15 validation passed cargo xtask ci with 2,301 tests passed, zero failed and eight ignored. The engine passed 932 tests with three ignored, the shared UI passed 371 with one ignored, and the desktop passed all 29. Forty-two new cancellation regressions plus one generation-exhaustion test cover the API. Both iOS compile checks for figcraft-ipad passed. The late-finish regression failed before the active-source guard and passes after it. The final full run includes cancellation without an active tab; the earlier 2,299-test CI run predates those final ownership cases. Native touch, UI drag cleanup and physical input qualification remain future work.


## 2026-10-16: T34 ordered input and UI cancellation

The native raw_input_hook observes the pinned egui-winit event producer before
egui aggregates pointer state. Observation preserves the original pointer route.
The opt-in normalizer recognizes only contiguous synthesized touch suffixes,
retains independent mouse/panel events and records contact identity, ordinal and
frame-clock provenance. Force does not identify Pencil. Unsupported sequences
cancel and suppress ownership; capacity recovery requires focus reset. Backend
primary/cursor state survives focus loss so a late End is decoded correctly.

An explicit-consumer boundary dispatches canvas contact/text/contact in raw
order, using the existing shortcut/IME interpretation once. Its keyboard state
retains Alt-shortcut text suppression and paste lookahead within one batch.
Panel field text stays with egui. A forwarded panel or mouse event cancels the
canvas operation before egui can mutate focus/layout, then suppresses subsequent
canvas contacts in that batch. Value tracking is bounded and retains no text.
The native app does not install a consumer until T16 provides its full recognizer.

FigApp cancels UI owners before restoring its T15 checkpoint. Gradient/crop/blur/
padding/gap handles, guides and preview, comment pins and prototype connectors
cannot commit from a later reset release. Comment popover and prototype editor
state are restored on a normal cancellation of their source. Commands, text/IME,
platform/background completions, control, due autosave, layout/source changes and
lifecycle requests have barriers. A checked host interruption generation also
invalidates pending contacts that have no engine checkpoint yet. A nonempty live
poll invokes UI cancellation before applying its events; empty polls leave local
ownership alone. Attached live documents remain refused until T28.

Validation and simulator evidence are recorded in the external T34 result. The
native app still uses its existing pointer route. No Pencil identity, device touch
or collaboration qualification is claimed, and physical checks remain deferred.


The final ingress audit also guards frame-end live synchronization when deferred
work or recovery can settle. The recovery test confirms cancellation precedes
opening an independent recovered copy; idle sync preserves local ownership. The
legacy engine sync wrapper keeps its original application path. Final validation
and the local source commit are recorded in the T34 result.


Final T34 validation passed cargo xtask ci: 2,364 passed, zero failed and eight
ignored. The engine passed 934 tests (three ignored), UI 432 (one ignored) and
desktop 29. Sixty-one UI cases and two engine cases are new relative to T15.
Both iOS target checks and the simulator link passed. The iPad Air 11 M3 simulator
rendered the synthetic document; tapping the smoke field and typing T34 showed
exactly three characters. The captured runtime log had no unsupported-touch
warning or panic. This is simulator evidence; physical input remains deferred.
