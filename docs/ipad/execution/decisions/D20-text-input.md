# D20 revision 1: canvas IME output lifetime

The responsible implementation session reviewed this bounded repair on
2026-10-19 after the user's continuation instruction. Status: **idle-focus repair verified in software/simulator; physical gate
blocked**.
Revision 0 below remains the historical diagnostic record.

## Reproduction and repair boundary

Create a canvas text layer through the Text tool, then redraw with an actually
empty RawInput.events vector. Canvas editing is still active, but platform
output loses its IME caret on the first idle frame. The ordinary shared test
harness inserts ModifiersChanged even into its nominally empty frames, masking
this failure. The new regression uses truly event-free redraws, separately
with a fresh edit and with transient marked text.

shortcuts::handle returns for empty events before reaching text_keys and
ime_output. egui-winit computes IME enablement from ime.is_some() on every
platform output. The existing iOS winit set_ime_allowed(false) calls
resignFirstResponder. This source trace explains how a valid canvas edit can
lose native input without any text-delivery or composition adapter defect.

Reviewed choice: evaluate active canvas text presentation before the
empty-events return. Keep field/palette ownership checks ahead of this path;
keep text event processing, transient marked strings and undo semantics
unchanged. This is a shared UI output-lifetime repair, not a native responder
override. Native field-to-canvas and idle typing must still be verified.
No native interop, event-loop owner, unsafe code, dependency or document change
is approved. T32 remains undecided and physical evidence remains required.

## Verified result

The bounded shared repair passes five `ipad_text_` tests, including two new
regressions for fresh/marked idle presentation and field-to-canvas ownership.
The test harness now exposes an event-free frame without inserting a modifier
event, while retaining its existing clock and texture-delta handling.
Full CI passes 2,591 tests, zero failures, ten ignored; both iOS target checks
pass. An earlier CI attempt exited before test summaries while available disk
was below the recorded safety margin. Its log is retained, not counted as a
pass. After removing recorded task-created compiler intermediate objects,
the complete run passed with compilation concurrency limited to two.

The three unchanged native T20 tests now pass in an optional-no-font build.
The active synthetic recovery generation contains node 53 with exactly
`T20 canvas`, rather than the empty node recorded in revision 0.
An additional pinned-font native scenario passes field entry, creation of
canvas text, four seconds of idle before typing, a return to the field and
a second canvas edit after another idle interval. The current recovery
generation contains exactly `T20 first \u{00e9}\u{1f600}` in node 53 and
`T20 second` in node 54. Field text remains independently `field-back`.
Escape sequences here describe the actual stored accent and emoji scalars.

Screenshots show the software keyboard, visible Latin/accent canvas text and
the second string. The emoji renders as a missing glyph in this font profile,
despite exact storage. This is not an emoji rendering qualification. One
earlier extended test incorrectly tried typing after an ordinary Move-tool
canvas click; it failed as expected without an edit. The corrected scenario
explicitly reselects Text. A subsequent passing run placed text too high to
remain visible after keyboard reflow; the final run changes only test tap
positions so the rendered text can be inspected. Preserve all those attempts.

The native responder explanation is a source trace plus before/after native
typing evidence, not direct instrumentation of UIKit's first-responder state.
No native responder workaround or replacement adapter was needed for this
reproduction. That result cannot decide T32 is unnecessary for full text input.
Native marked-text composition, UTF-16/grapheme range conversion, replacement,
native undo grouping, floating/split keyboards and physical accessories remain
unqualified. T20 stays blocked on those required device criteria.

Evidence is in task-local `ipad-port-evidence/T20/focus-trace/`: initial/final
shared logs, native-original and native-transfer-visible result bundles,
screenshots, current-generation recovery verification, exact source hashes,
full CI and both iOS check logs. The original native test source is unchanged.
No T23 qualification method was selected or repeated.

## Historical revision 0

# D20 revision 0: text diagnostic boundary

Recorded 2026-10-19 by the responsible implementation session under the
development-only continuation. Status: **blocked for device evidence and native
canvas focus diagnosis**. This is not approval of a native adapter or a
not-needed decision for T32.

## Independent field and canvas results

Three new shared `ipad_text_` tests pass. They cover whole multi-scalar emoji
selection replacement, accent/combining/CJK/multiline owned paste, normalized
line endings, one undo per canvas editing session, transient marked strings
with and without an existing selection, single commit delivery and numeric
field composition ownership/undo. These are synthetic egui events, not UIKit
composition or UTF-16 conversion evidence.

The first preedit assertion incorrectly expected the original selected text
to remain in the backing document. Existing engine semantics remove that
selection when composition begins; marked strings remain transient. The test
now checks this existing behavior both with and without a selection. The
initial failure is retained; no production semantics were changed.

On iPad Air 11-inch (M3), iOS 26.3.1 simulator, the isolated native field tests
pass for `cafe`, U+00E9 and U+1F600. The field reports four, one and two Unicode
scalars respectively. They exercise committed strings through XCTest, not an
actual Japanese candidate session or accessory switching.

Native canvas entry fails separately: choose Text in the touch tool menu,
tap the canvas, then call XCTest `typeText("T20 canvas")`. A new empty text
node and caret appear, but XCTest reports that neither the application nor
its descendants has keyboard focus. No requested string is delivered.
Recovery archives contain the new empty node, not the requested string.
The same three unchanged tests yield two passes/one failure both without
fonts and with the existing validated T12 font profile. Missing fonts alone
therefore do not explain this failure. It remains a native focus/bootstrap
diagnostic, not proof of a required replacement IME implementation.

## Remediation boundary

Before choosing an adapter, trace shared IME output and winit first-responder
state at canvas creation, after idle redraws and after field-to-canvas focus
transfer. Distinguish missing output, disabled IME, responder ownership and
XCTest-only dispatch limitations. Add a smallest deterministic focus-output
regression once that cause is established. Do not work around the failure by
injecting engine commands, focusing a hidden field or bypassing input ordering.

If a native adapter is subsequently required, its reviewed contract must cover
UTF-16 ranges versus Rust scalar/grapheme boundaries, marked-text cancellation,
selection synchronization, focus transfer, owned paste and exactly-once commit
delivery. No such adapter types, event producer or unsafe boundary are approved
or implemented by this revision.

Physical CJK composition, selection/replacement, floating/split keyboards,
hardware switching and native undo grouping remain unqualified. T20 stays
blocked and T32 stays undecided. Neither the passing shared tests nor passing
native field tests release T25 or another device/release gate.

## Evidence

Task-local `ipad-port-evidence/T20/` retains initial/final shared test logs,
full CI (2,583 passed, zero failed, ten ignored), both native result bundles,
screenshots, exact app commit/worktree/executable identities, test-source hashes
and synthetic recovery archives. No T23 qualification method was selected.
The native test apps/runners were removed and no persistent keyboard preference
or Simulator GUI launch argument was changed.
