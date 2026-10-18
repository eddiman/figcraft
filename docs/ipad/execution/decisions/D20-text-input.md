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
