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
