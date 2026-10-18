# iPad rendering development measurements

T21 development checkpoint, 2026-10-19. Hardware qualification remains open.
The renderer and document/history semantics are unchanged.

## Bounded worker

The native render worker previously used unbounded job and completion channels.
A paused-consumer regression retained 1,003 document references after 1,000
submissions. It now retains four in the same scenario. There is one running
render, at most one pending job, and at most one completed image waiting for
the UI. A capacity-one notification channel wakes the existing worker; replacing
a pending job never blocks on rendering. Completed images remain publishable
while newer work is pending, preserving interactive progress.

Completion still wakes the correct viewport before advancing the finished
sequence. Invalid renders complete without a fabricated image and do not stall
the next valid request. Shutdown clears pending/completed slots without joining
on the UI thread. The worker releases its retained document when it exits.
Web retains its existing inline latest-job implementation.

## Host diagnostic

Run from the repository root with the existing T01 generated archives:

```sh
FIGCRAFT_IPAD_CORPUS=/absolute/path/to/ipad-port-evidence/T01/generated \
  cargo test --locked -p figcraft-ui-egui ipad_render_profile \
  -- --ignored --nocapture --test-threads=1
```

This is an optimized-test-profile macOS CPU diagnostic on Apple M4, Mac16,10,
16 GiB memory, Rust 1.98.1, with host system fonts. It is not an iPad benchmark
or a bundled-font comparison. Each case runs 20 in-memory open/edit/render/
PNG-export/archive-save/reopen/close cycles at 800x600 pixels. Small/medium
cases use the existing synthetic layer builder; large/effect-heavy cases load
the actual T01 archives. Warm UI measurements enable adaptive presentation.

| Case | Nodes before edit | Open mean ms | Raster + screen conversion mean ms | Submit to completion mean ms | Export + save + reopen mean ms | Warm UI CPU mean ms |
|---|---:|---:|---:|---:|---:|---:|
| Synthetic 50 | 51 | 0.148 | 0.812 | 1.516 | 4.759 | 0.042 |
| Synthetic 1,000 | 1,001 | 2.389 | 1.366 | 1.812 | 13.597 | 0.044 |
| T01 large | 10,002 | 25.235 | 1.919 | 3.029 | 80.838 | 0.083 |
| T01 effect-heavy | 1,002 | 3.270 | 33.896 | 34.559 | 50.485 | 0.045 |

Submit-to-completion includes scheduling and the test's 1 ms polling interval.
PNG export uses the existing full-quality renderer, not preview LOD settings.
The command-level maximum RSS was 414,023,680 bytes; this is not settled
iPad-app memory or evidence against monotonic device growth. Means do not
establish the physical p95 presentation or input-to-render targets.
GPU upload/presentation, device peak/settled memory, thermal behavior and the
30-minute device session remain unmeasured.

## Remaining memory-pressure boundary

The current lifecycle message contract has no memory-pressure event, and the
retained raster cache belongs to the render thread. This checkpoint does not
add an observer, change renderer ownership, or claim pressure handling.
A scoped follow-up must review notification lifetime and a render-thread-owned
cache-release request. Releasing preview caches must preserve document/history,
avoid a UI-thread join, preserve the latest requested view and repaint after
recovery. History eviction or a renderer replacement is outside this work.
