# iPad downstream patch register

Preparation checkpoint: 2026-10-19, through `3495727`. The integrated upstream
is `102404e8c3d23d86c3490c1a6beff3202bb73e25`; it remains an ancestor.
The corresponding [version record](upstream.toml) is a snapshot, not a claim
that a newer upstream revision was merged or qualified.

Subsequent T20 diagnostics add shared Unicode/composition/field regressions.
D20 revision 1 repairs canvas IME output disappearing on event-free redraws,
which caused the existing platform integration to disable native input.
Unchanged native text tests and pinned-font idle/field-transfer diagnostics
now pass with exact recovered strings. See [D20](../docs/ipad/execution/decisions/D20-text-input.md).
Preserve this per-frame output lifetime when merging shortcuts.rs. Physical
composition/range/keyboard gates remain open; this neither retires IPAD-004
nor decides that a native adapter is unnecessary.

T22 extends IPAD-002 with shared touch-tool minimum targets, focus names and
scrolling, modal/save-prompt keyboard restoration and the existing owned-copy
Save command dispatcher. IPAD-001 save acknowledgement remains unchanged.
Six retained-UI regressions and the isolated native create/property/drawer/
invalid-name/corrected-save workflow cover these changes. See the
[retained audit](../docs/ipad/accessibility/README.md); physical acceptance
and T20/T32 gates remain open.

T35 extends IPAD-002 through a scoped iPad presentation profile. It adds shared
100/150/200 percent chrome metrics, measured header/tool-row fitting,
independent dock restoration, Home adaptation, a direct active touch tool with
separate disclosures, touch Save As dispatch and prompt-local save failures.
It preserves desktop/web metrics, original vector icons and selection graphics,
existing command IDs and owned-save semantics. This entry has no physical
qualification or release claim. The repeated iPad Air 5 field-tap keyboard
restart and touch-selection finding remains T20 work.

The 2026-10-28 T35 continuation keeps compact tools in one horizontally
scrollable row instead of stacking them vertically behind the keyboard.
Preserve `ipad_compact_tools_remain_reachable_with_enlarged_text_and_keyboard`
when merging this branch. All 17 adaptive tests, two native simulator
diagnostics, both iOS checks and WASM pass; physical qualification remains open.

No upstream proposal is recorded for these changes. The fork's existing PR is
not an upstream acceptance. Do not resolve conflicts with blanket ours/theirs,
reset upstream ancestry, or remove patches solely because a merge is clean.
Reconcile the behavior and its regressions before retiring an entry.

| ID | Intent and source arcs | Key paths and merge exposure | Required evidence / retirement condition |
|---|---|---|---|
| IPAD-001 | T05/T06/T33 owned platform requests and capabilities | `crates/ui-egui/src/{platform_requests,lib,menus,dialogs,export_panel}.rs`, desktop/web service adapters; high UI overlap | Preserve owned-resource lifetimes, explicit capability/errors, nested export trees and desktop/web defaults. Retire only for equivalent upstream service semantics and passing platform-request tests. |
| IPAD-002 | T18/T19/T22/T35 opt-in adaptive shared presentation and editor metrics | `adaptive.rs`, `theme.rs`, `toolbar.rs`, `tabs.rs`, `layers.rs`, `right.rs`, `home*.rs`, `widgets.rs`, `icons.rs`, locales and host geometry wiring; high panel overlap | Retain one shared panel implementation, modal canvas exclusion, allocated touch targets, actual command IDs, owned Save As route, physical mouse modifiers and desktop/web defaults. Adaptive/native keyboard tests and desktop/web regression checks must pass before replacement. |
| IPAD-003 | T14/T15/T16/T34 contact arbitration and exact cancellation | UI `touch*`, `ordered_input.rs`, `canvas.rs`, guides/handles/vector/prototype/comments; engine pointer-cancellation hooks; high overlap | Preserve per-contact ownership, event ordering, pending/late-release suppression, exact rollback and helper cleanup. Keep matrix/host interruption regressions; physical multitouch/Pencil remains unqualified. |
| IPAD-004 | T12/T13 Apple shortcut policy, text/clipboard ownership and fonts | `shortcuts.rs`, `cjk.rs`, text font database/build script, `maintenance/ipad-fonts.json`, host clipboard adapters; medium/high overlap | Preserve Command versus Control, field/canvas ownership, acknowledged Copy/Cut, explicit clipboard errors, font provenance and no-font packaging behavior. Device IME and missing-font metrics remain open. |
| IPAD-005 | T09/T10/T11 whole-file captured saves and shared recovery | Engine persistence/branches/live/online, format archive entry points, `crates/recovery`, desktop recovery adapter and owned iPad saves; high persistence exposure | Preserve activation-time snapshots, ticket ordering, dirty acknowledgement, atomic generation publication, old-run retention and online reconciliation. Never replace semantic/archive/history tests with ZIP equality. |
| IPAD-006 | T02/T03/T07/T11 native app, copy pickers, lifecycle and packaging | `apps/figcraft-ipad`, `platform/apple-ipad*`, `packaging/ipad`, `.github/workflows/ipad-ci.yml`; mostly additive | Preserve reviewed FFI exceptions, one eframe/winit event loop, owned copies, isolated native-workspace checks, both target/link checks and explicit device deferral. Native bridge versions are 0.1.0. |
| IPAD-007 | T29 downstream identity/distribution policy | Not implemented; future About, bundle identity, branding and distribution configuration | No distributed-product or release readiness claim. Brand assets remain untouched; modified distributions must comply with `docs/brand/LICENSE-brand.txt` before release. |
| IPAD-008 | T13 pinned dependency overrides | Root Cargo patch table; `maintenance/ipad-deps/{egui,egui-winit,eframe,winit}`; high dependency-upgrade exposure | Preserve clipboard receipts/errors, iOS Command routing, opt-in focus/input batching, eight-batch redraw budget and retained suffix semantics. egui-winit skips the interrupt-composition resign/become-first-responder toggle on iOS (a tap on a focused field otherwise restarts the keyboard; device-observed 2026-10-21, no upstream proposal). Port and run the feature matrix and excluded-workspace host tests before changing pins or retiring copies. See [dependency record](ipad-deps/README.md). |
| IPAD-009 | T21 bounded native rendering | `render_worker.rs`, `idle_tests.rs`, `perf_tests.rs`; medium worker exposure | Preserve one pending job/one completed image, viewport wakeup/finished ordering, failed-view progress and nonblocking shutdown. Queue, replacement, idle and lifetime tests cover the contract; host timings are not device performance. See [measurements](../docs/ipad/performance/README.md). |
| IPAD-010 | T01 compatibility corpus and semantic fixtures | `tests/ipad/fixtures`, engine fixture tests, layering/test registration and attribution metadata; mostly additive | Retain images, parent/component links, version/branch snapshots and history through desktop/iPad round trips. Reconcile equivalent upstream fixtures without deleting downstream edge cases. |

## Inventory and update procedure

At this checkpoint, 64 files already present in the integrated baseline are
modified. That count includes manifests, tests and documentation; it excludes
new additive files, including vendored dependency trees. It is not a conflict
count. Generate the actual exposure list rather than assuming table key paths
are exhaustive:

```sh
git diff --numstat --diff-filter=M \
  102404e8c3d23d86c3490c1a6beff3202bb73e25 3495727
git diff --name-status \
  102404e8c3d23d86c3490c1a6beff3202bb73e25 3495727
git merge-base --is-ancestor \
  102404e8c3d23d86c3490c1a6beff3202bb73e25 HEAD
```

The task-local inventory retains every changed path and numstat row. An actual
rehearsal still needs a disposable authorized integration worktree, a newer
actual upstream commit, per-intent conflict review and the full host/web/iPad
matrix against the merged result. Preserve main/release branches and existing
ancestry. Hardware import/edit/save/reopen remains a separate acceptance gate.
This preparation performs no merge, workflow dispatch, PR action or release.
