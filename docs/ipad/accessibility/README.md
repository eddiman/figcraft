# Retained labels, keyboard focus and touch audit

T22 development audit, 2026-10-19. This covers shared labels, keyboard routes,
target sizing and visible feedback. VoiceOver/native accessibility bridging,
contrast-specific adaptations and reduced-motion adaptations are excluded.
Existing shared accessibility code remains intact.

## T35 presentation follow-up, 2026-10-23

T35 is in review after applying these retained touch and focus findings to a
scoped shared iPad presentation profile. It adds no VoiceOver bridge and does
not claim Dynamic Type support. The 100/150/200 percent choices apply only to
the adaptive iPad presentation. The shared tests cover compact, medium and wide
logical widths, threshold edges, keyboard geometry and focus restoration.
Rendered Home, tabs, layer, inspector and tool capture remains a review item.

The physical iPad Air 5 record remains preliminary. Its repeated focused-field
tap keyboard restart and touch text-selection issue stays with T20. A
presentation pass does not close that input gate or decide T32.

## Repairs and software coverage

The adaptive Text menu target was about 37 points wide. Touch tool group
buttons now allocate at least 44 by 44 points, use the existing focus-name
helper and prefer the active tool group when entering the toolbar. Focused
controls scroll into view across the horizontal tool rows. Button text, menu
state and the existing shared labels remain available; no native bridge was
added.

Modal drawers previously lost Tab focus through background region navigation.
They now leave Tab with egui's modal handling, retain the shared layer-row arrow
and rename/context commands, and restore a still-visible keyboard origin after
closure. Rotation to docked panels restores the keyboard origin; a pointer
closure does not resurrect keyboard mode. The save-name prompt participates in
the same restoration, has a named field and touch-sized field/Save/Cancel
controls. Captured save state and acknowledgement semantics are unchanged.

Native checking also demonstrated that the adaptive Save button called the
engine directly, bypassing the owned-copy UI route. It now invokes the existing
menu dispatcher with the same `file.save` ID, as keyboard/menu Save does.
Undo/Redo keep their command IDs and shared dispatcher.

Six `a11y_tests::ipad_` regressions cover:

- Label and allocated target checks at 600, 800 and 1,200 logical-point widths.
- Drawer Tab ownership and keyboard-origin restoration for Layers and Assets.
- Shared layer-row arrow selection inside a drawer.
- Rotation and pointer closure without stale focus.
- Actual touch Save with iPad capabilities, named controls, visible synthetic
  failure, retained editable prompt and keyboard return after cancellation.
- Toolbar traversal/scrolling with a persistent 16.5-point font override
  (150 percent of the ordinary 11-point button font).

The enlarged-font check covers the tool rows, not OS Dynamic Type support or a
claim that every custom-painted inspector label scales. Headless AccessKit
metadata is used as an inspection mechanism; it does not qualify VoiceOver.

## Simulator workflow

The final isolated native test passes on iPad Air 11-inch (M3), iOS 26.3.1
simulator. It creates/selects a rectangle through the touch tool menu and drag,
changes its height to 88 through the inspector keyboard field, opens/closes
Layers, invokes touch Save, submits an invalid slash-containing name, observes
the validation message, corrects the name and observes Saved with the dirty
marker removed. The error and success screenshots were inspected.

Earlier unsuccessful runs are retained: OCR could not recognize Width, one
setup failed to rotate, the original touch Save route did not open its prompt,
and XCTest newline input did not submit that prompt. The final workflow uses
the visible Height label and actual Save buttons. No native Return-key prompt
submission qualification is claimed.

The sole Simulator GUI was restarted once with its original arguments after
the orientation failure. No persistent keyboard setting, GUI keyboard override,
global Xcode selection or existing development app/container was changed.
Unique test apps/runners were removed. T23 qualification was not repeated.

## Remaining acceptance

T22 stays in review. Physical input and end-to-end keyboard accessory workflows
are unqualified; native keyboard restoration is covered by shared deterministic
checks, not a physical accessory pass. T20's canvas-text native-focus failure
remains blocked and T32 remains undecided. Shared labels/focus repairs do not
release those gates or establish alpha/distribution readiness.
