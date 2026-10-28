# Local iPad port development log

## 2026-10-28: T20 focused-field IME interruption guard

The local `egui-winit` patch no longer interrupts composition for an already
focused iOS field. The previous winit path resigns and then restores first
responder, which restarts the software keyboard after a repeated field tap.
Other targets retain the upstream composition-interruption path.

This adjustment does not settle canvas input delivery, native selection, range
mapping, undo semantics or physical keyboard variants. The iOS simulator target
build validates the compile path; a targeted simulator interaction and physical
field check remain required before T20 can leave review.

## 2026-10-23: T35 shared-editor presentation review

T35 adds a presentation-only 100, 150 and 200 percent text setting, measured
header and tool-row fitting, separate dock visibility, a compact Home layout,
an explicit active tool and prompt-local owned-save failures. Touch More now
dispatches Save As through the existing menu path, and Present uses matching
touch hit, hover and paint rectangles. The pass keeps the engine, renderer,
document, command identifiers, save acknowledgement, recovery and event-loop
boundaries unchanged.

The targeted adaptive, owned-save, save-prompt and catalog checks pass, as does
`cargo check -p figcraft-ipad`. Full UI coverage still lacks the optional
`../craft-fonts` checkout, and `cargo xtask ci` stops at the pre-existing
formatter order for the untracked `edit_menu.rs` registration. Xcode 26.3 at
`/Volumes/XcodeDev/Xcode-26.3.0.app` built, signed and installed the development
app on an iPad Air 11-inch (M3) Simulator. Direct smoke launch and raw image
capture succeeded. The packaging test runner stopped after installing its
runner and did not write a summary or test captures, so it does not qualify the
interaction workflow. Physical-device checks, T20's focused-field keyboard
restart and touch selection remain open.

## 2026-10-19: T20 canvas IME output survives idle frames

A fresh canvas edit lost IME output on the first redraw with no input events.
The usual shared test harness inserted ModifiersChanged into "empty" frames,
so earlier tests did not exercise that case. egui-winit disables IME without
output, and the existing iOS winit implementation then resigns first responder.
D20 revision 1 reviews moving the empty-event guard after active text handling,
without changing field ownership, text semantics or native interfaces.

Two new regressions use event-free frames with the existing harness clock:
fresh/marked canvas edits and numeric-field-to-canvas focus. All five text
checks pass; full CI passes 2,591/0/10 and both iOS checks pass. Initial
test-harness texture/clock issues and an interrupted CI attempt are retained
separately from final success. Task-created compiler intermediates were
removed to restore the disk margin; final CI used two build jobs.

The three unchanged native tests now pass, including canvas entry without
optional fonts. Current recovery contains exactly T20 canvas. A pinned-font
workflow verifies four-second idle periods and field/canvas transfers,
with exact accent/emoji and second-string storage in two new nodes.
The visible emoji remains a missing glyph. Earlier Move-tool test setup and
offscreen tap-position attempts remain recorded. This is not physical IME
composition, native range mapping, native undo or keyboard-variant acceptance.
No native adapter was introduced. T20 remains blocked; T32 remains undecided.
T23 qualification was not repeated.

## 2026-10-19: T22 retained labels, focus, sizing and Save route

Initial headless checks exposed a 37-point Text target, unnamed touch-toolbar
focus and lost modal Tab ownership. Shared repairs allocate 44-point tool
groups, retain focus names/active-tool preference, scroll focused controls,
keep modal Tab with egui and preserve layer-arrow commands/restoration.
The owned-save prompt has named touch controls and restores keyboard origin.
No snapshot/acknowledgement or native interface changed.

Native testing additionally exposed touch Save bypassing the owned-copy UI
route; the button now uses the existing menu dispatcher. The final isolated
native workflow passes: create/select rectangle, keyboard-edit Height to 88,
open/close Layers, reject a slash-containing name and recover to Saved after
correction. Earlier OCR, orientation, routing and newline-submission failures
remain recorded. One GUI restart retained original launch arguments/settings.
Six shared regressions include real iPad-capability Save click, error recovery,
rotation/pointer closure, layer arrows and 150-percent toolbar font traversal.
T22 stays review; T20 native canvas focus and physical workflows remain open.
VoiceOver, contrast-specific and reduced-motion scope remains excluded.

## 2026-10-19: T20 separate field and canvas diagnostics

Three shared tests cover Unicode/emoji selection replacement, combining/CJK
multiline paste, composition/commit, one-session undo and numeric-field focus.
Full CI passes 2,583/0/10. An initial composition assertion was corrected to
the existing engine selection-removal behavior; production text semantics
are unchanged. Native simulator fields accept Latin, accent and emoji strings.
Native canvas typing fails before delivery because XCTest finds no keyboard
focus, despite a visible new text caret. The same two-pass/one-fail result with
existing validated fonts excludes font absence as the sole explanation.
D20 revision 0 records the reproduction without approving a replacement
adapter. T20 remains blocked on native focus diagnosis and physical evidence;
T32 is undecided. T23 qualification was not repeated.

## 2026-10-19: T24 patch register and compatibility preparation

Added maintenance/upstream.toml, maintenance/patches.md and
docs/ipad/compatibility/README.md. The records name the integrated upstream,
development checkpoint, archive version, toolchain/font/dependency pins,
high-exposure shared seams, preserved invariants and patch retirement checks.
The inventory identifies 64 modified existing baseline files, separately from
additive native/vendor files. The recorded baseline remains an ancestor.
No upstream fetch/merge, main/release change, workflow dispatch or hardware
smoke was performed. T24 remains review for the actual integration rehearsal
and required acceptance. The preparation does not rerun T23 qualification.

## 2026-10-19: T21 bounded render mailboxes and host profiling

A deterministic paused-consumer regression exposed unbounded render-job
retention: 1,000 submissions kept 1,003 document references. Native workers now
use one pending job and one completed-image slot, with a bounded wake signal.
The same burst retains four references. Raster work remains outside the mailbox
lock on the existing worker thread, and intermediate completed images remain
available while newer work is pending. Failure and viewport wakeup semantics
are retained; shutdown clears queued resources without a UI-thread join.
The web inline worker is unchanged.

Four new regressions cover queue bounds/latest completion, failed-view recovery,
completed-image retention/shutdown and adaptive idle behavior. A separately
invoked ignored host diagnostic executes 20 in-memory editing/export cycles on
each of four corpus cases. Detailed means and limitations are recorded in
performance/README.md. These are host CPU observations, not device p95 or
GPU presentation measurements. Full CI passes 2,580/0/10; both iOS target checks
and a linked native simulator rendering/drawer/rotation smoke pass.
The native screenshot was inspected. T23 qualification was not repeated.
Memory-pressure integration requires a further scoped lifecycle/cache-ownership
review; physical peak/settled memory and thermal qualification remain deferred.

## 2026-10-18: T19 responsive presentation, simulator checks complete

The iPad host now opts into the shared T18 presentation policy. Wide layouts
dock both shared panels, Medium docks Properties with a Layers/Assets drawer,
and Narrow uses modal drawers. The canvas and document state are reused.
Drawer activation cancels held canvas intent and excludes native canvas targets.
Primary Save/Undo/Redo, tab close and error reporting remain reachable.
Touch tools use the existing command catalog. Multi-select and Constrain merge
Shift into finger contacts only, preserving physical mouse modifiers.

Density-aware controls allocate larger rows and separate hit regions rather
than inflating rectangles over adjacent controls. A compact tool row preserves
44-point controls when supplied keyboard bounds leave little height.
Shared panel commands reveal their drawer; Assets does not depend on the
desktop icon rail. Desktop/browser presentation defaults remain unchanged.

D19 records the bounded architecture review for a synchronous safe
UIKeyboardLayoutGuide query. It tracks undocked geometry and disables the
hidden keyboard's bottom-safe-area strip, converts to window points and returns
owned numeric bounds. The host converts to egui points, cancels canvas intent
on changes and explicitly reports unavailable geometry. There is no new unsafe
allowance, observer, event loop, renderer or engine/document dependency.
The isolated UIKit feature set now explicitly enables core-foundation support;
the lock adds that existing-version dependency and dispatch2, with no upgrades.

The UI suite passes 595 tests with one ignored, including eight new adaptive
integration/control regressions. Both iOS target checks and isolated bridge
checks pass. Explicit WASM and full CI pass, with 2,576 passed, zero failed and
nine ignored across the full root/plugin run. An external GPU-rendered
shared-editor host captured 24 layouts at
600/800/1200 points, including both orientations, drawers and supplied keyboard
bounds. The largest portrait request was limited by macOS to 1200x1387 points.
These supplied rectangles are not native keyboard evidence.

Earlier T19-only isolated XCTest attempts retained failures: the first reached the
portrait drawer and focused field but reported a zero-height keyboard frame;
later portrait requests left the app at 1180x820, preventing the intended
rotation/drawer assertions. The system Settings app independently reproduced
both hidden-keyboard and unchanged-orientation observations. A Simulator GUI
restart with the process-only `-ConnectHardwareKeyboard NO` launch argument
restored system rotation and a visible software keyboard. The combined
intervention establishes an environment contribution, without isolating each
setting's effect. No production text/input or event-loop change was needed.

The final T19-only XCTest run passes both tests, without skips: portrait drawer
open/close, landscape reflow and visible native keyboard avoidance in both
orientations. Keyboard heights are 279 points in portrait and 364 in landscape;
the Multi-select labels end above their respective keyboard top edges.
The captured drawer and both native keyboard screenshots were inspected.
The temporary GUI override was removed, persistent keyboard preferences were
verified unchanged, and the original development app/container was preserved
and relaunched. Clipboard contents were not accessed. T23 qualification was
not rerun. Physical touch-only editing and floating/split keyboard qualification
remain deferred, so T19 is in review and no dependent gate is released.

## 2026-10-18: T23 hosted qualification complete

All five fork PR checks pass on `c1eff4e`: unsigned device and simulator jobs,
Linux/workspace tests, WASM compilation and packaging lint. Hosted iPad CI
run 39160778732 retains the passing editor XCTest and exactly one deliberate
negative assertion, whose exit 65 is required by the workflow. Downloaded
screenshots show typed `abc`, inspector X=10 and X=10 after cold restart with
an empty field. The hosted destination was iPad (10th generation), iOS 26.1;
the toolchain was Rust 1.98.1, Xcode 26.3 and SDK 26.2 on macOS 15.7.9.
Linux CI run 39160778857 passes 2,563/0/8 on Rust 1.99.1; local full CI is
2,568/0/9. T23 is done. No physical qualification, PR merge or release occurred.

## 2026-10-18: T23 current-stable atomic API compatibility

The first hosted Linux Clippy job on Rust 1.99.1 rejected the downstream atomic allocators'
deprecated `fetch_update` spelling. The eight iPad-related call sites now use
`try_update`, with identical ordering, checked increments and error handling.
The replacement is stable since Rust 1.95, matching the workspace minimum, and
compiles on the pinned 1.98.1 toolchain. No warning suppression, dependency or
toolchain change is introduced. Local full CI passes 2,568/0/9; both iOS target
checks and the isolated bridge checks pass. Remote qualification remains in progress.

## 2026-10-18: T23 remote packaging repair

Fork PR #1 targets `ipad/upstream-baseline` at the reviewed upstream SHA,
avoiding an incidental merge of the fork main branch's newer upstream history.
The first remote packaging job found that the font helper's directory change
was not guarded when called from a conditional. It now fails explicitly before
invoking Cargo if the repository directory cannot be entered. A regression
exercises that conditional path with a missing directory and a stub Cargo.
All packaging ShellCheck checks, four prerequisite tests and local full CI pass.
Hosted iPad and Linux/web qualification is still in progress.

## 2026-10-18: T23 simulator runner and unsigned CI configuration

Continued with simulator/CI work under the user's instruction to choose a task
without physical-device requirements. T16's existing software integration was
accepted for T23 development only; hardware and later qualification gates remain.

The new runner selects an available iPad by device type, rejects phones, builds
the current Rust app and a separate XCTest target, and installs uniquely named
test apps into isolated containers. Public XCTest input and Vision recognition
exercise the rendered editor without a new production bridge or command channel.
It retains result bundles, screenshots, logs and build/destination identities,
requires nonzero passing tests and propagates failed assertions.

The final workflow checks inspector X=0, types `abc` into the ordinary field,
observes three characters, invokes the existing smoke edit and checks X=10.
After termination/relaunch, the field resets and the inspector still shows
X=10. Screenshots were inspected. A deliberate failure exits 65 and is checked
separately from unrelated infrastructure failures. Eight host regressions pass.
Initial harness failures in orientation, OCR targeting and result parsing are
retained separately; the final assertions do not use the smoke button label as
evidence of an edit.

The workflow configures standard macOS 15 arm64 jobs with Rust 1.98.1/Xcode 26.3,
target/SDK/lockfile-aware caching and unsigned device/simulator builds.
It preserves upstream Linux/wasm jobs and is disabled in the original upstream
repository. No signing secrets, release environment or production dependencies
were introduced. Hosted-runner execution remains unverified.

Both locked target checks and linked builds pass. Full CI initially failed
46 server tests when free space fell below the existing 1 GiB safety threshold.
Only device-target intermediates were cleared using
`cargo clean --target-dir target/aarch64-apple-ios`, after preserving the linked
unsigned app and evidence. The unchanged full suite then passed 2,568 tests,
zero failures and nine ignored. No safeguard or test was relaxed.

The test apps/runners are removed; the existing development app is restored at
PID 45520. No clipboard contents or keyboard-capture settings were changed.
T23 is in review for remote execution; the full port remains unfinished.

## 2026-10-18: T10 local Save/Save As and relaunch reopening

The shared Save actions now opt into a native owned-save Services boundary.
Prepared snapshots are captured at activation; Save As names a new local copy
without changing any imported original. One worker encodes/writes, bounds
outstanding requests to eight and coalesces queued writes per owned directory.
Successful synchronized replacement alone permits acknowledgement. Newer edits
stay dirty; closed-tab saves finish and update recents. Completion waits for a
new canvas gesture to finish rather than cancelling it. Pending tabs indicate
Saving, and cancellation leaves the path/baseline unchanged.

The isolated simulator app saved a generated document and reopened it from
persisted recents after relaunch. A canvas drag then moved Fixture image to
(16, -60); Save replaced the same archive, and another relaunch/reopen displayed
those exact coordinates. The stored document.json independently agrees.
The temporary app/bundle were removed, original PID 45520 restored and keyboard
capture left unchanged. No clipboard operation was used.

Eight new app tests and six shared UI tests cover writes, reopening, queue
pressure, ordering, failures, cancellation and completion ownership. A paused
worker test uses actual text editing and verifies the old archive text, newer
live characters, caret and dirty state independently. App=54,
UI=587 with one ignored; full CI=2,568 passed, zero failed, nine ignored.
Host/simulator Clippy, device compile and simulator linking pass. Existing stale
generated UI catalogs were cleared with a package-scoped Cargo clean before
the final checks. No production dependency or native FFI boundary changed.

T10 is in review for physical save/reopen/cancellation and filesystem-failure
qualification. Host destruction before UI completion does not guarantee the
recents acknowledgement of a drained job. Private owned-directory assumptions
are retained; this writer is not an external-original editing mechanism.
The full plan remains unfinished.

## 2026-10-18: T13 accepted for development; T10 owned recents started

The bounded T13 acceptance pass is green and the card moves to review with
remaining native/device qualification deferred. The user directed moving on;
T07's development prerequisite for T10 is now explicitly reviewed without
claiming physical acceptance.

Native startup now resolves owned-copy recents from persistent directory IDs
and file names instead of saving container prefixes. Completed editor frames
persist changed recent lists through the native Services adapter. The bounded
index uses exclusive staging, file sync, rename and parent sync; malformed or
unsafe references fail explicitly. Four new tests cover relocation, corrupt
indexes, failure preservation and replaced directories. All 46 app tests,
host/simulator Clippy, device compile and simulator linking pass.

Save/Save As, bounded document workers and acknowledgement wiring remain the
next T10 work. This checkpoint persists imported-file recents, not edited files.

## 2026-10-18: Canvas batching regression coverage

Five new app-host tests exercise the ordered-input helper, Native normalizer and
shared editor together. They cover mouse/touch backlogs across redraws, exact
node/undo counts, canvas/field-Paste/canvas ownership, focus-loss cancellation and
the lifecycle cancellation barrier preserving prior work against stale releases.
All 42 app tests and app all-target Clippy pass. No production code changed.
The earlier full-CI result remains separate from this package-level checkpoint.

Simulator burst automation was inconclusive: some posted key events never
reached the native keyboard handler. In the traced attempts, received pressed-A
counts matched the visible character counts. The app queue was not changed on
the basis of missing automation delivery. The isolated app was removed and the
original restored; keyboard capture retained its observed initial value of one.
Native burst latency, real suspension and physical qualification remain open.

## 2026-10-18: T13 simulator text and focus qualification

The current production simulator executable was rebuilt and exercised in an
isolated synthetic app container. Observed screenshots establish tap-to-focus,
physical key delivery, Select All, field Copy/Cut/Paste, Escape transferring
Paste to the canvas, single canvas Paste/Undo and tap-back field ownership.
One background/foreground cycle preserved the same PID and field/canvas state;
subsequent typing succeeded. No production code changed during this check.

Already-authorized public Accessibility/CoreGraphics APIs delivered input.
Process-targeted physical keys and explicit Command modifier transitions worked;
HID/Unicode and flags-only shortcut attempts are retained as automation failures.
Only selected synthetic field text was copied/pasted. The isolated app was
removed, keyboard capture restored and the original app preserved.

The external T13 result, runtime-focus-observations.json, action log and inspected
screenshots record outcomes and limitations. Physical input, clipboard images,
permission/denial, actual suspension, hidden/resumed canvas interactions and
multiple-click/backlog responsiveness remain unqualified. No gate is released.

## 2026-10-18: T13 actual host-loop regression coverage

Six new tests call the vendored eframe update and logic-only methods using its
existing headless Frame constructor. They verify that the eight-batch budget
retains a suffix before newer input and requests another redraw, hidden ticks
do not run the mutating hook, resumed input is normalized once, hidden ticks
preserve an existing suffix, per-batch outputs merge in order, and close requests
are one-shot with accepted/cancelled behavior. The iOS focus path is selected
under cfg(test) only; shipping desktop behavior and native ownership are unchanged.

The new host-input runner uses the original library source through a temporary
manifest because Cargo cannot test this excluded dependency from the root
workspace. It preserves production runtime dependency versions, checks dev-only
versions against recorded lockfiles, runs offline, rejects zero/insufficient test
matches and removes its temporary directory. Both bridge check targets invoke
it. No source manifest or production lockfile is rewritten. The unused inspection
dependency is omitted only from the temporary test manifest.

Both native bridge scripts and both app target checks pass. Full CI passes
2,545 tests with nine ignored; the six host-loop tests are separate from that
workspace count. Native bridge suites pass eight and eleven tests respectively.
Strict standalone eframe Clippy exposed existing documentation, type-complexity
and unfulfilled-expectation warnings, plus a removed lint. These are retained
and printed by the runner; diagnostics in the changed host/test files fail it.
The new tests and host code have no remaining diagnostics.

An initial full CI run failed the server's real low-disk guard with less than
1 GiB free. Scoped Cargo cleanup of egui/egui-winit/eframe artifacts freed 1.2 GiB;
the full rerun passes without changing the guard or server code. Failed logs are
retained in the external T13 evidence. Interactive responsiveness, real hidden/
resumed canvas behavior and physical qualification remain open. No gate is released.

## 2026-10-18: T13 ordered native input batches

The production reproducer sent `tap B, Paste q, tap A, Paste r` while A held
focus. egui delivered both values to A because its interaction snapshot retained
only the final click. A widget-only patch would also leave cross-field Copy/Cut
callbacks ordered by widget draw order.

The iOS-only eframe focus opt-in now partitions raw input before the existing
Native normalizer and UI pass. Pointer batches end before another click owner;
each editing, modifier or focus intent gets its own batch. Each batch completes
the existing hook/UI/finish-frame cycle. FullOutput::append merges outputs.
At most eight batches run per redraw; an unconsumed suffix stays in the existing
pending-input buffer, ahead of newer input. Hidden logic-only updates retain
unnormalized input so a canvas edit cannot run twice on resume. There is no new
responder, native owner or clipboard read.

Seven regressions use the production partition helper and verify both widget
orders, cross-field Copy/Cut callback order, Escape, repeated-click selection,
the actual Native panel normalizer, retained suffixes and one-shot metadata.
Full CI passes 2,545 tests with nine ignored, including 37 app-host tests. Device
compile, simulator Clippy/linking and the standalone focus-without-clipboard
build pass. A separately installed copy of the executable rendered the synthetic
editor; it was removed and the original simulator app restored.

Raw egui callers which bypass the native batching API retain aggregate behavior.
Interactive keyboard/clipboard, input backlog responsiveness, hidden/resumed
canvas interactions and physical devices remain unqualified. See D13 and
results/T13.md in the external execution plan. No task gate is released.

## 2026-10-18: T13 color normalization and production callback regressions

The clipboard image boundary converts supported PNG/JPEG RGB and grayscale ICC
profiles to straight sRGB RGBA, preserving alpha. PNG gamma/primaries and SDR
sRGB/Display P3 cICP are handled; unsupported HDR and malformed/discarded metadata
fail explicitly. The conversion uses the existing pinned moxcms/png libraries
directly, without depending on the renderer or changing their versions. RGB
conversion uses a 4 KiB scratch buffer instead of another full-size image.

Production dependency tests reproduced acceptance of an invalid write receipt:
the earlier isolated repair had not been adopted. Context now validates receipts
before reporting success, so a failed field Cut keeps text and selection. Image
validation avoids a full-buffer clone, distinguishes malformed content from size
limits, preserves native denial through reentry, and records bounded error notices.
Ten new app-host tests cover these cases plus tap-release field ownership in both
widget orders, two owned paste outcomes, Escape, callback panic recovery and
cross-thread reentry. These run with the production iPad feature flags.

The clipboard crate has eleven passing tests covering numerical P3 conversion,
JPEG, grayscale alpha, PNG metadata precedence, malformed/over-budget profiles,
16-bit decoding bounds and prior image tests. `check-bridge.sh` now includes both
excluded native workspaces. Interactive multiple-click ordering, actual device
permissions and IME qualification remain open; no task gate is released.

## 2026-10-18: T13 production follow-up and isolated T10 coordinator

Native Copy as PNG now uses a result-bearing service and shows success only after
acknowledgement. Denied/unavailable writes preserve the document and selection.
Desktop and browser output behavior is unchanged. Clipboard image decoding checks
the expanded RGBA size before allocating pixels; the regression uses an oversized
grayscale header without a valid pixel payload. Invalid native images retain their
error category. The excluded clipboard lockfile now names the production crate,
without changing dependency versions.

Full CI passes with 2,528 tests and nine ignored, including shared UI, engine,
desktop, WASM, layers, assets and plugins. The standalone clipboard crate passes
four tests and both target Clippy checks. Both iOS app checks pass. The linked
production executable renders the synthetic editor in an isolated simulator
container; no interactive clipboard or physical qualification is inferred.

The existing T10 coordinator remains test-only because T07 has not released its
dependency gate. It now rejects non-file roots/destinations, foreign-root keys and
poisoned locks, retains exclusive staging ownership, and tests preservation of
existing bytes on a failed staging write. Staging names also fit when the destination
uses the full 255-byte leaf allowance. Its ten tests pass within the app's
20-test CI result. Persistent IDs, recents, Save/Save As, worker bounds and device
filesystem behavior are not implemented or qualified by this checkpoint.

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


## 2026-10-17: T16 finger arbitration and shared pointer values, in progress

The pure recognizer retains the original Start until a drag exceeds six logical
points, accepts a tap on End, and opens the existing context menu after 500 ms.
A second canvas finger cancels the owned preview before establishing the pinch
anchor. Losing either finger or adding a third suppresses the remaining contacts
until they lift. Navigation changes the view without recording document undo.
Commit-only comment creation and Eyedropper intent wait for an accepted tap;
existing comment pins retain their drag policy. Unknown contacts follow finger policy even when pressure is present.

The opt-in finger host dispatches explicit values through the existing prototype,
comment, guide, handle and engine interpretation. The ordinary wrappers continue
to read egui input. A regression exposed a prototype release on the next frame
because egui had no primary button down for a value-owned gesture. The canvas now
keeps explicit ownership until its explicit Up or cancellation, including overlay
drags. Accepted overlay Down also takes canvas keyboard ownership. Contact
modifiers follow raw event order. Focus reset clears the consumer's identities
alongside the producer; ordinary interruptions retain held suppression.

Completed-layout snapshots identify the canvas source and allocation, exclude
panels, the floating toolbar and foreground layers, and refuse stale source maps.
Document previews and view transforms retain the layout epoch. Software tests
exercise recognizer thresholds, raw suffix removal, actual preview rollback,
anchored pinch, comment creation/pin movement, exactly-once drawing, long press,
and 98 combinations of 14 tools and seven interruptions. The full UI suite passes
with 463 tests and one ignored test. Full CI passes with 2,395 tests, zero
failures and eight ignored tests. Both iOS targets compile and the simulator
app links. The rebuilt simulator editor renders and accepts the smoke-field
text T16 exactly once. The matrix covers draws, marquee and Hand;
specialized vector/text/handle/prototype fixtures remain to complete T16.

T16 remains in progress. Shell::raw_input_hook still uses Observe, so these APIs
do not activate native touch editing. Independent physical mouse dispatch,
aggregate panel-touch zoom policy and the remaining integration matrix must be
completed before activation. Device qualification remains deferred. No native
bridge, dependency, asset, persistence or collaboration change is included.


## 2026-10-17: T16 combined finger and physical pointer route, in progress

The opt-in Native coordinator now routes normalized finger contacts and
independent physical pointer values in raw order. Its physical mirror uses the
pinned public egui InputState for click classification and one whole-frame
wheel/trackpad result. Panel touch suffixes never enter that mirror, including
an identical-position independent mouse event after a suffix. The mirror retains
no typed text, IME, clipboard payload or dropped-file handles. Source validation
rejects nonfinite navigation and metadata and blocks canvas editing until focus
reset; invalid values do not reach egui geometry.

Physical Down enrolls the existing checkpoint. Move/Up, comment/prototype/guide/
handle interpretation, Space/Z/middle navigation and secondary click use explicit
values. The combined route gates aggregate canvas pointer dispatch for the whole
UI pass, even after an accepted touch has finished. Panel widgets keep their raw
egui input. A source-specific hover position also drives canvas presentation and
file-drop placement. Ordinary desktop/web policy remains the default.

A regression exposed an engine text selection drag retained after DoubleClick.
The value dispatcher now completes its physical release inside the checkpoint,
retaining the word selection and permitting the next click. The shared line
selection action handles an explicit physical triple click without replaying
Context click flags. Another regression exposed a cancelled finger's late End
cancelling a newer mouse scope. The boundary now keeps that End suppressed while
a new finger Start still interrupts a held mouse. A test fixture also assumed
that a shape tool remained selected after a draw; it now explicitly enables the
existing keep-tool preference when testing two draws in one batch.

The final UI suite passes with 491 tests and one ignored test. Full CI passes
with 2,423 tests, zero failures and eight ignored tests. Both iOS target checks
and simulator linking pass. The rebuilt simulator editor renders and accepts
M16 in its smoke field exactly once. New tests cover
physical click timing/order, wheel equivalence across frames, panel suffix
isolation, scoped edits/navigation, word/line selection, mixed mouse/touch
exactly-once behavior, text ordering, panel field typing, held mouse plus panel
Cancel, panel pinch isolation and recovery after unsupported values. The native
Shell remains Observe. Specialized operation/interruption coverage, the missing-
app-borrow guard and Shell integration/runtime evidence remain before activation.
When no completed canvas map exists, new contacts remain with egui's panel
route. This keeps home/modal controls operational while the native canvas gate
prevents edits and fixed ownership prevents a held contact from transferring
when a map appears. Both cases have regression tests.
T16 and the full port goal remain incomplete; physical qualification is deferred.


## 2026-10-17: T16 unavailable-app guard and specialized cancellation cases

Native::enable selects the canvas policy before the host installs FigApp, so a
first unavailable raw hook cannot fall back to legacy editing. Native::unavailable
queues the existing rollback, clears context-owned handle/guide/prototype drags
before a panel reset release, drains the same normalizer and preserves panel
text. It updates the physical mirror and retains actual held buttons while
discarding local edit intent. Finger suppression retains unknown identities
until focus reset. The old canvas map is discarded because the editor did not
draw; resumption restores the engine before handlers and needs a new map.

Six guard tests exercise actual nested app borrowing, first-hook policy, finger
End, physical Up, a still-held button, explicit unsupported input and recovery.
A prototype test also inspects its private owner before the forwarded reset Up.
The Native matrix now crosses nine causes with prototype connectors, earlier
Pen anchors, earlier text/preedit, vector bend, vector Bezier handles and comment
pins. These 54 cases compare the source and destination documents, selection,
text/preedit, vector sub-selection, undo/redo, view and prior interaction against
the normal external command outcome on a pre-Down control engine. The fixtures
centre their synthetic geometry and initialize the destination tab's viewport;
earlier failures came from off-canvas points and an uninitialized control view.

A final lifecycle regression found that the boundary read its interruption
counter before preflight consumed a queued cancellation. An old pending contact
could then emit its first Down. The boundary now compares after preflight and
flushes queued cancellation before ordered values and before the timer. Tests
cover an initial pending contact, cancellation queued between Start/Move and a
queue during preparation before a due long press.

The final full CI passes with 2,439 tests, zero failures and eight ignored
tests; UI has 507 passes and one ignored test. Both iOS target checks and
simulator linking pass. The final Observe-mode build renders the synthetic
editor. Its smoke field displayed L16 with three characters after refocusing.
T16 remains in progress. Guide, gradient/crop/blur/padding/gap and vector width/region rows,
completed-frame/panic handling in the host and native activation/runtime evidence
remain. Shell still uses Observe, and hardware qualification stays deferred.


## 2026-10-17: T16 secondary panel pointer ownership

The combined Native regression reproduced a panel field that stayed empty when
another contact was held on the canvas. The pinned backend emits pointer events
only for its numeric primary, so forwarding a secondary panel Touch did not give
egui a pointer press. The opt-in normalizer now selects a panel contact by its
fixed device/contact ID and emits panel pointer controls. It strips only the
proven backend touch suffix; panel Touch and independent physical events remain
in order. Observe keeps the original producer stream.

The panel owner ends on its own release or cancellation. A canvas primary's End
cannot release that panel pointer, and held panel contacts never inherit another
contact's pointer. Independent physical pointer activity cancels the panel
candidate before forwarding the physical event. Any held physical button prevents
a new panel press. Cancellation uses the existing finite move/reset sequence so
a short Cancel cannot click a button. Invalid input also clears the panel owner,
including physical metadata rejected after normalization and unavailable app
access.

Nine new regression functions cover the failed field case, secondary Cancel,
canvas-first release, physical takeover, fixed ownership without promotion,
all five physical buttons, exact physical event indices and unsupported input.
The UI suite passes 516 tests with one ignored. The initial takeover fixture used
the canvas position for the backend End suffix after physical motion; it now
uses the backend's last cursor position. The original field assertion remains.
Full CI passes 2,448 tests with eight ignored. Both iOS target checks and
simulator linking pass. The linked simulator build renders the synthetic editor
and accepts P16 in its startup field. The native shell still observes input only. The remaining T16 matrix, guarded
frame completion and native activation remain open; hardware testing is deferred.


## 2026-10-17: T16 UI panic cancellation and frame completion

Two Native regressions reproduced a provisional drawing entering emergency
recovery after a UI panic and a pending finger creating a shape after the panic.
The window guard now cancels the enrolled pointer and clears current-context
owners before recovery capture. Cancellation advances the existing interruption
generation even when pending intent has no engine token. The guard returns true
only after its callback completes normally.

Native finish_frame takes the completion result and publishes a layout only on
success. An aborted pass discards the old map and suppresses held intent while
preserving producer identities and physical button state. abort_frame also works
without app access: it queues the existing cancellation notice and defers a
selected panel pointer's reset until the next actual raw hook. Generated reset
controls remain outside the physical event samples. The unavailable raw hook
reuses this cleanup before draining the same producer.

Regressions inspect the document inside Services::emergency_save, then check late
finger/mouse release and pending intent. Other cases abort after canvas drawing,
require a fresh completed map and a new Start before editing, keep a held mouse
suppressed until release, cancel panel clicks on the next raw hook, preserve
producer counters/source indices and remove an unenrolled prototype drag owner
before recovery capture. Shell remains Observe pending the remaining Native tool
matrix and activation checks. Hardware qualification remains deferred.


The final UI package run passes 524 tests with one ignored. Both iOS target checks
and simulator linking pass. The rebuilt Observe-mode simulator rendered the
recovered synthetic editor and accepted R16 (three characters) in its startup
field. The first full CI attempt stopped on the existing plugin-panel idle
repaint test. That test passed in the final package suite and in five isolated
repeats; its source, idle harness and autosave code are unchanged. The initial
failure remains recorded, and no test assertion or timing threshold was relaxed.

The repeat full CI passes 2,456 tests with zero failures and eight ignored.


## 2026-10-17: T16 combined Native cancellation matrix

The combined Native coordinator now runs 45 fixtures against second contact,
Cancel, focus loss, canvas allocation change, portrait viewport, modal, tab
switch, close, unavailable app access and UI panic. The 450 interruption cases
include nine draw tools, move/duplicate/resize/rotate/scale, arc/radius handles,
gradient end/stop/width, crop move/scale, both progressive blur handles, padding,
main/cross gap, guide create/move/copy/delete, vector point/Bezier/bend/width/
region, marquee, Hand, earlier freehand/Pen/text/preedit, new/outside text,
comment pin/cluster and prototype connector. Thirty-nine normal-release sequences
confirm the new fixtures perform their intended operations before cancellation
is tested. The six earlier fixture groups remain in the interruption matrix.

The matrix requires an actual enrolled preview. Test-only handle/guide accessors
report computed hit positions and owner/preview state; they never inject owners.
Document, active tab/page, selection, text/preedit, vector selection, guide/hover/
focus, text preview, rotation origin, history contents/limits, view, tool and
accepted Pen state are compared with a pre-Down control engine after interruption
and again after late release. Guide acceptance checks exact offsets and counts,
duplication and the second freehand stroke check node counts, and point/region
fixtures verify the intended sub-selection. Arc coordinates come from the arc
handle geometry, so the fixture cannot pass by resizing the ellipse's edge.

The initially unselected vector-point drag exposed a production layout defect.
The vector panel's unwrapped instruction extended the canvas's right edge to
1249.2; selecting a point changed it to 1200. The existing allocation barrier
then cancelled the drag. Extra warmup did not avoid it. Wrapping the same hint
within the panel keeps the allocation stable and permits that first point drag.
A counterfactual run of the unchanged final fixture with the old hint failed;
restoring the wrap passes normal release and all ten causes. Vertex preselection
was not added, and resize cancellation was retained. Temporary tracing was removed.

An initial new-text assertion incorrectly required a busy tool, while text Down
creates a provisional layer and enters text editing immediately. The final test
checks those actual states. An arc fixture initially hit the outer resize edge;
it now uses the computed arc handle and requires its overlay. The final UI suite
passes 563 tests with one ignored. Shell remains Observe until Native activation
and runtime editing/navigation checks pass; hardware qualification remains deferred.


Full CI passes 2,495 tests with zero failures and eight ignored. The first CI
attempt stopped on a Clippy test-helper ordering rule; the helper was moved before
the test module without relaxing the lint. Both final iOS checks and simulator
linking pass. In the linked Observe-mode simulator, Actions entered vector edit
and the hint visibly wrapped into two lines within the right panel. Earlier drag
and Return attempts did not immediately produce a completed rectangle; subsequent
palette interaction exposed an unfinished tall rectangle, and tool.place refused
while that drag was busy. Switching to Move and invoking vector.edit.begin reached
the inspection state. These legacy-route attempts are recorded as unsuccessful
gesture evidence. Native shell activation/runtime checks remain open.


## 2026-10-17: T16 Native shell activation and open panel state

The iPad shell now owns one Native coordinator and enables its policy before
installing the editor. The raw hook uses the available editor or the coordinator's
unavailable path. A successful guarded UI pass publishes the completed layout;
a caught panic or unavailable UI borrow aborts the frame. Desktop and web retain
their existing input policies. No dependency or Apple bridge changed.

Two new regressions exercise already-open comment and prototype editors across
ten interruption causes, comparing their state with an independent control UI
after cancellation and late release. The comment case also changes a panel search
filter during the preview and checks that cancellation preserves that change.
The UI suite passes 565 tests with one ignored. Full CI passes 2,497 tests with
zero failures and eight ignored. Both iOS target checks and simulator linking pass.

In the linked Native simulator, one drag created one 125 by 100 rectangle and
returned to Move. Undo removed the rectangle and Redo restored it once. A fresh
drag moved it from X=311/Y=44 to X=241/Y=144 without changing its dimensions.
Home and resume retained it, and a subsequent resize produced 175 by 140. Panel
text displayed N16 after refocusing and retrying; the first typing attempt showed
no text, so first-focus keyboard behavior remains unresolved. Runtime logs contain
no panic or unsupported-touch diagnostic. These checks use synthetic content.

T16 remains in progress. Actual SDK multi-touch interruption, one-finger release
and subsequent editing remain unverified at runtime. The software matrix covers
those paths, but it does not qualify native hardware. Physical testing remains
deferred, Pencil identity remains T17 work and live editing enrollment remains
restricted until T28. The full port goal remains active.


## 2026-10-17: T16 release-order recovery and runtime field audit

A combined Native regression now checks both contact release orders after pinch
cancels a provisional rectangle. Moving the remaining contact preserves the
document and undo history. After both contacts lift, a fresh drag must enroll
and create exactly one shape and undo entry. The primary-first fixture includes
the pinned producer's resumed pointer emulation for the remaining contact; Native
continues to suppress that contact. Initial fixture compilation and suffix errors
are retained as development evidence.

A temporary simulator audit logged raw events before and after Native and the
synthetic rectangle transform. The first typing attempt and standalone Return
produced no keyboard events at the raw hook. Retried typing arrived unchanged,
and touching the Y field committed X=250 in the document. The audit narrows the
missing input to delivery before Native; its cause remains unproven. Two prior
follow-ups accepted text immediately after focus. No keyboard workaround or
production logging was added. The committed production app was rebuilt and
reinstalled after the diagnostic. Runtime multi-touch and physical qualification
remain unverified.

Full CI passes 2,498 tests with zero failures and eight ignored; the UI suite
passes 566 with one ignored. The software implementation now enters review while
the required device draw/pinch/release-one sequence remains deferred. This status
does not release dependent cards or qualify hardware. The prior iOS checks and
production rebuild cover unchanged shipping source; this arc adds a test and
progress records. The independent T12 font-input and T13 clipboard/keyboard
architecture decisions remain available for further work.


## 2026-10-17: T12 pinned embedded font inputs

D12 resolves the missing Latin-manifest input decision by selecting the seven
unchanged files in craft-fonts revision 3d9ab3a132ed0eff457e424226a435a901da30d3.
The external checkout contains no Inter. iOS reads its primary manifest and
prefers BIZ UDPGothic for missing document fonts while retaining requested font
names. Substitute metrics can change line wrapping. Desktop/web keep their
existing manifest/default-font policy. Native font scans now exclude desktop
directories, and CJK language availability requires actual embedded sample glyphs.

The new ipad-fonts task checks the revision, manifest, font hashes and OFL text
hashes from maintenance/ipad-fonts.json. Native packaging validates an available
profile, requires inputs for Release or an explicit required-font build, and
bundles the verified licenses and profile. Optional development builds without
fonts still link and display an unavailable-document-font message. Rebuilding
without fonts clears the previous profile's bundle metadata. Font bytes remain
in the external repository. No dependency version or native interop changed.

A direct font probe passed all declared samples, but document layout initially
failed for Thai because its existing fallback list omitted Noto Sans Thai. Adding
that family repairs the layout regression and also supplies Thai UI glyphs.
With system fonts disabled, all 69 text tests and 568 UI tests pass, with one UI
ignore. Validator tests reject changed bytes, missing license text, unreviewed
licenses and wrong revisions. Required/release absent-input checks fail explicitly.
Both iOS target checks and simulator builds with/without the profile pass. The
linked required-font executable contains all seven exact font payloads, and the
bundle contains six OFL files, attribution and matching metadata.

The T01 same-font recipe preserves its semantic checks and requests BIZ UDPGothic
for every text node. The simulator imported the resulting archive through the
production picker and exported the page to Files. Its 360 by 360 PNG is byte-identical
to the desktop export (SHA-256 e3f5a20bc1a4c7ade55d37ab45fb56a8ad4921c9940bbe835630ba63e2b67b39).
The source app's Documents folder is not exposed by file sharing; the existing
synthetic picker probe's visible folder supplied this local fixture. No app
file-sharing setting changed. This comparison qualifies the recorded simulator
fixture only; physical font/render checks remain deferred.

An initial full UI profile run passed its library tests but failed doctests while
another Cargo process rebuilt the same host directory with different font inputs.
The serial rerun passes library and doctests. Both logs remain as evidence; no
source workaround or test relaxation was added.

Full CI passes 2,505 tests with zero failures and nine ignored, including the new
manual same-font export recipe. T12 enters review pending the deferred physical
font/render comparison; no dependent card is released. The full port remains
incomplete, and T13's modifier/clipboard architecture is the next independent
work.

## 2026-10-17: T13 Apple shortcut table prerequisite

The engine build detector now includes iOS in Apple shortcut semantics, and the
UI detector uses egui's Apple classification for macOS and iOS. The regression
checks Cmd-Z/C/V/S, distinct Control behavior and Ctrl-based mapping for Windows,
Unix, Android and unknown browser platforms without changing the global platform
cache during the test. This establishes table selection only. The pinned winit
iOS backend has no hardware press or modifier producer, and egui-winit's modifier
conversion excludes iOS from Apple semantics. Native clipboard ownership also
remains unresolved. T13 stays blocked on the reviewed native boundary; no hardware
keyboard, paste, field-focus or image clipboard qualification is claimed.
