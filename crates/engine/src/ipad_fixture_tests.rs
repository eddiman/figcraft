//! Semantic fixtures shared by the iPad qualification cards. No platform APIs are required.
use std::io::Cursor;
use std::sync::{Arc, Once};

use figcraft_color::{ImageScaleMode, Paint};
use figcraft_doc::{Document, Effect, NodeKind};
use figcraft_format::{Branch, BranchState, Version};

fn fixture(layers: usize) -> Document {
    static FONTS: Once = Once::new();
    FONTS.call_once(figcraft_text::scan_system_fonts);
    let (mut doc, page) = crate::synthetic::large_document(layers).unwrap();
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([24, 96, 168, 255]));
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let (hash, _, _) = figcraft_format::add_image(&mut doc, png.into_inner()).unwrap();
    let id = doc.create(page, NodeKind::Rectangle, "Fixture image", 0.0, -64.0, 32.0, 32.0).unwrap();
    doc.node_mut(id).unwrap().fills = vec![Paint::image(hash, ImageScaleMode::Fill)];
    doc
}

fn assert_semantics(expected: &Document, got: &Document) {
    assert!(got.validate().is_empty());
    // Compare the parsed document representation, including relationships, text and layout.
    // Encoded archive sizes, timestamps and compression are deliberately irrelevant.
    assert_eq!(figcraft_format::to_json(got).unwrap(), figcraft_format::to_json(expected).unwrap());
    assert_eq!(got.images, expected.images, "encoded image content and dimensions");
    let page = got.first_page().unwrap();
    let button = got.nodes().find(|n| n.name == "Button" && matches!(n.kind, NodeKind::Component { .. })).unwrap();
    assert_eq!(button.parent, Some(page));
    assert!(got.nodes().any(|n| matches!(n.kind, NodeKind::Instance { component, .. } if component == button.id)));
    let header = got.nodes().find(|n| n.name == "Header").unwrap();
    assert_eq!(header.layout.as_ref().unwrap().gap, 8.0);
    let title = got.nodes().find(|n| n.name == "Title" && n.parent == Some(header.id)).unwrap();
    assert!(matches!(&title.kind, NodeKind::Text(t) if t.characters == "Screen 1"));
    let image = got.nodes().find(|n| n.name == "Fixture image").unwrap();
    assert_eq!(image.parent, Some(page));
    let data = got.images.values().next().unwrap();
    assert_eq!((data.width, data.height), (2, 2));
    assert_eq!(got.images.len(), 1);
    let decoded = image::load_from_memory(&data.bytes).unwrap().to_rgba8();
    assert!(decoded.pixels().all(|p| p.0 == [24, 96, 168, 255]));
}

#[test]
fn ipad_fixture_roundtrip() {
    for layers in [50, 1000] {
        let doc = fixture(layers);
        assert_eq!(figcraft_format::to_json(&doc).unwrap(), figcraft_format::to_json(&fixture(layers)).unwrap());
        let bytes = figcraft_format::save_bytes(&doc, None).unwrap();
        let (loaded, issues) = figcraft_format::load_bytes(&bytes).unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        assert_semantics(&doc, &loaded);
    }
}

#[test]
fn ipad_fixture_preserves_history_and_branches() {
    let doc = fixture(50);
    let snapshot = Arc::new(doc.clone());
    let mut head = doc.clone();
    head.name = "Exploration".into();
    let branch = Branch {
        id: "ipad-branch".into(),
        name: "Explore".into(),
        time: 20,
        client: 7,
        state: BranchState::Active,
        base: snapshot.clone(),
        head: Arc::new(head.clone()),
        versions: vec![Version::new("ipad-branch-v1", "Branch version", "Branch snapshot", 21, Arc::new(head.clone()))],
    };
    let version = Version::new("ipad-main-v1", "Main version", "Main snapshot", 10, snapshot);
    let bytes = figcraft_format::save_bytes_with_history(&doc, None, &[version], &[branch]).unwrap();
    let loaded = figcraft_format::load_bytes_with_versions(&bytes).unwrap();
    assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);
    assert_semantics(&doc, &loaded.doc);
    assert_eq!(loaded.versions.len(), 1);
    let version = &loaded.versions[0];
    assert_eq!(
        (version.id.as_str(), version.name.as_str(), version.description.as_str(), version.time, version.auto),
        ("ipad-main-v1", "Main version", "Main snapshot", 10, false)
    );
    assert_semantics(&doc, &version.document().unwrap());
    assert_eq!(loaded.branches.len(), 1);
    let branch = &loaded.branches[0];
    assert_eq!(
        (branch.id.as_str(), branch.name.as_str(), branch.time, branch.client, branch.state),
        ("ipad-branch", "Explore", 20, 7, BranchState::Active)
    );
    assert_semantics(&doc, &branch.base);
    assert_semantics(&head, &branch.head);
    assert_eq!(branch.versions.len(), 1);
    let version = &branch.versions[0];
    assert_eq!(
        (version.id.as_str(), version.name.as_str(), version.description.as_str(), version.time, version.auto),
        ("ipad-branch-v1", "Branch version", "Branch snapshot", 21, false)
    );
    assert_semantics(&head, &version.document().unwrap());
}

/// Explicit export recipe for later device measurements. Normal tests do not write archives.
#[test]
#[ignore = "writes large qualification fixtures to FIGCRAFT_IPAD_FIXTURE_DIR"]
fn ipad_fixture_export_large_and_effects() {
    let out = std::path::PathBuf::from(std::env::var_os("FIGCRAFT_IPAD_FIXTURE_DIR").expect("set FIGCRAFT_IPAD_FIXTURE_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    for (name, layers, effects) in [("large", 10_000, false), ("effect-heavy", 1000, true)] {
        let mut doc = fixture(layers);
        if effects {
            let ids: Vec<_> = doc.nodes().filter(|n| matches!(n.kind, NodeKind::Rectangle)).map(|n| n.id).collect();
            for id in ids {
                doc.node_mut(id).unwrap().effects = vec![Effect::drop_shadow(), Effect::layer_blur(), Effect::glass()];
            }
        }
        let bytes = figcraft_format::save_bytes(&doc, None).unwrap();
        let (loaded, issues) = figcraft_format::load_bytes(&bytes).unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        assert_semantics(&doc, &loaded);
        std::fs::write(out.join(format!("{name}.figcraft")), bytes).unwrap();
    }
}
