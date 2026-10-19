# iPad compatibility development checkpoint

The [upstream record](../../../maintenance/upstream.toml) and
[patch register](../../../maintenance/patches.md) describe the integrated
`102404e8` baseline and downstream checkpoint `3495727`. This is T24
preparation, not an upstream-update rehearsal or product qualification.

## Preserved contracts

- `.figcraft` archive version remains 1. Owned-device paths, recents and recovery
  metadata stay outside document semantics.
- T01 fixtures check parsed content, images, component/parent relationships,
  versions and branches. ZIP byte identity is not the compatibility criterion.
- Save receipts acknowledge the captured document state only; newer edits and
  collaboration changes cannot be silently acknowledged by a filesystem write.
- No new collaboration wire version is introduced. Compatibility with previous
  shipped clients/server versions has not been independently qualified.
- Existing desktop/web defaults and the shared engine/panels remain in use.

## Current evidence and limits

The T21 development source passes full CI (2,580 passed, zero failed, ten ignored),
including the complete format/engine/UI suites and web/plugin builds. Both iOS
targets compile; a linked iPad simulator renders the synthetic document through
the bounded worker and passes drawer/rotation smoke. Existing T23 hosted
qualification remains historical evidence; it was not rerun for preparation.

The ordinary development build intentionally lacks optional craft-fonts.
Required-font packaging validates the separate pinned profile and licenses;
Inter metrics and cross-device line wrapping remain unqualified. Do not treat
the host system-font performance diagnostic as a same-font pixel comparison.

Physical Pencil, IME/keyboard variants, memory/thermal behavior, provider access
and device round trips remain deferred. External-original editing, distribution
identity/brand compliance and release acceptance remain separate unfinished
gates. VoiceOver, contrast-specific adaptations and reduced motion are excluded
from current iPad scope; no qualification is claimed for them.

No newer upstream commit was fetched, merged or tested for this preparation.
Conflict counts, integration duration, retired patches and post-merge behavior
are therefore not available. No main/release branch, authentication, signing,
workflow or branding configuration was changed.
