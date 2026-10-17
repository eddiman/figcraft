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
