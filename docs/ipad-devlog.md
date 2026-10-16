# Local iPad port development log

## 2026-10-16: T33 owned export trees

The T06 audit found that batch exports can create icons/close.png, while the T05 dispatcher accepted only single filenames. T33 adds ExportTree beside ExportFiles, retaining a staging lease and validating a bounded set of relative paths. The desktop adapter copies the declared tree into the chosen folder and reports errors through the existing completion route. Existing picker call sites are unchanged until T06.

Tests exercise actual engine-generated nested PNG exports, regular-file overwrite, invalid paths and bounds, case-insensitive duplicate and prefix conflicts, cancellation, unsupported defaults, source preflight, destination conflicts, source/destination symlinks, cleanup and delayed UID routing. A failing self-copy test exposed macOS /var aliases; canonical root comparison fixes it. Filesystem checks reject observed links but do not lock ancestors against concurrent replacement. Tree copying is nontransactional. No native provider, persistence semantics, dependency, asset or device evidence is added.

The external execution backlog records D04 revision 3, T33 evidence and the remaining T06 migration. Physical-device checks remain deferred for development and required at qualification gates.

## 2026-10-16: T06 shared file-dialog migration

Open, Import, Place Image, Save As, layer exports, variable import/export, library import, plugin manifest import, image fills and experimental .fig export now submit owned platform requests. Encoding runs before export presentation, so later edits and tab switches cannot change the staged outputs. Imports and completions use the initiating UID. Native Save/Save As remain unsupported until durable persistence is connected; a picker result cannot mark a native document saved.

Image-fill replies retain the document snapshot and reject a changed document or paint. An initial revision-counter guard incorrectly rejected a reply after opening another tab because command bookkeeping increments that counter; the snapshot guard preserves tab switching. Desktop manifest import retains its companion module path. Other document modes reject local manifests before a picker opens. Shared command dispatch checks local hosting, plugin download and native save policy; existing single-window tab fallback remains intact.

The new regression tests exercise delayed replies with actual engine imports, PNG exports, variables, image paints and an original minimal plugin module. A synchronous desktop fake verifies that one completed import executes once and cancelled Open leaves the document intact. Full UI localization coverage also preserves the Export variables picker title. Evidence and remaining native/physical gates are recorded in the external execution backlog.
