# iPad compatibility fixtures

`manifest.json` references existing synthetic examples by repository-relative path and SHA-256. It does not duplicate or modify their archives. The generated PNG in the test builder is original test data. No asset files or fonts are added.

`crates/engine/src/ipad_fixture_tests.rs` builds small (50 requested layers) and medium (1,000 requested layers) documents with the existing `synthetic::large_document` helper, then adds one image rectangle. The helper may round the layer budget; these sizes are recipe inputs, not exact node-count assertions.

The tests save and reopen through the shared format implementation. They compare parsed document content and image bytes, then assert the component/instance link, page and header relationships, title text, auto layout gap and decoded image pixels. The history fixture has fixed identifiers and times, one main version, an active branch with a renamed head, and one branch version. Each snapshot must retain the same content, layout, component links and image. Archive byte equality and compressed size are not fidelity checks.

Run from the repository root:

```sh
cargo test --locked -p figcraft-engine ipad_fixture_roundtrip -- --nocapture
cargo test --locked -p figcraft-engine ipad_fixture_preserves_history_and_branches -- --nocapture
cargo test --locked -p figcraft-engine
cargo test --locked -p figcraft-format
```

System fonts are scanned before building, following upstream template tests. The optional `../craft-fonts` input was absent during initial setup. The fixtures assert semantics without prescribing rendered glyph metrics or preview pixels. Building twice tests determinism within that font configuration; cross-device text rendering still requires device qualification.

To export the large and effect-heavy qualification documents:

```sh
FIGCRAFT_IPAD_FIXTURE_DIR=/tmp/figcraft-ipad-fixtures cargo test --locked -p figcraft-engine ipad_fixture_export_large_and_effects -- --ignored --nocapture
```

This recipe creates a 10,000-layer synthetic document and a 1,000-layer document with drop shadow, layer blur and glass effects on each rectangle. It reopens and checks both before writing the files. The generated files are disposable qualification inputs. The recipe provides no performance measurements.

Validate the existing archive paths and hashes without Cargo:

```sh
python3 - <<'PY'
from pathlib import Path
import hashlib, json
root = Path.cwd()
manifest = json.loads((root / 'tests/ipad/fixtures/manifest.json').read_text())
for fixture in manifest['existing_archives']:
    path = root / fixture['path']
    assert path.is_file(), path
    assert path.stat().st_size == fixture['bytes'], path
    assert hashlib.sha256(path.read_bytes()).hexdigest() == fixture['sha256'], path
print('PASS: 3 archive paths, sizes and hashes')
PY
```


T12's same-font comparison reuses the 50-layer T01 recipe and sets every text
node's requested family to BIZ UDPGothic. It preserves the synthetic text and
semantic checks, writes `t01-biz-udpgothic.figcraft`, and exports a desktop PNG.
Use the pinned external profile from `maintenance/ipad-fonts.json` and disable
system fonts; the same archive remains an input for native/device comparison.

```sh
CRAFT_FONTS_DIR=/absolute/path/to/craft-fonts FIGCRAFT_EMBED_CJK=1 \
FIGCRAFT_NO_SYSTEM_FONTS=1 FIGCRAFT_IPAD_FIXTURE_DIR=/tmp/figcraft-t12 \
cargo test --locked -p figcraft-engine ipad_fixture_export_same_font_text -- --ignored --nocapture
```

This recipe creates comparison artifacts. A passing export does not establish
cross-device font metrics or raster parity.
