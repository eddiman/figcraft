use super::*;
use figcraft_doc::{LibraryManifest, NodeKind, PublishedLibrary};
use std::io::{Cursor, Read};

fn member_bytes(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut file = zip.by_index(i).unwrap();
            let name = file.name().to_string();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            (name, data)
        })
        .collect()
}
fn content(doc: &Document, versions: &[Version], branches: &[Branch]) -> ArchiveContent {
    ArchiveContent::capture(doc, None, versions, branches).unwrap()
}
#[test]
fn ipad_archive_identity_detects_allocator_and_published_snapshot() {
    let a = Document::new("Allocator");
    let mut b = a.clone();
    let id = b.create(b.pages[0], NodeKind::Rectangle, "Temporary", 0.0, 0.0, 1.0, 1.0).unwrap();
    b.remove(id).unwrap();
    assert_eq!(a, b);
    assert!(!content(&a, &[], &[]).same_persisted_content(&content(&b, &[], &[]), true));
    let mut a = a;
    let lib = PublishedLibrary { manifest: LibraryManifest { id: "sample".into(), ..Default::default() }, doc: Arc::new(Document::new("Published")) };
    a.published = Some(Arc::new(lib));
    let mut b = a.clone();
    Arc::make_mut(&mut Arc::make_mut(b.published.as_mut().unwrap()).doc).name = "Changed library".into();
    assert_eq!(a, b);
    assert!(!content(&a, &[], &[]).same_persisted_content(&content(&b, &[], &[]), true));
    assert!(member_bytes(&content(&b, &[], &[]).encode().unwrap()).iter().any(|(n, _)| n == "library/document.json"));
}
#[test]
fn ipad_archive_identity_preserves_raw_history_and_unknown_json() {
    let doc = Document::new("Raw");
    let mut value = to_json(&doc).unwrap();
    value["futureField"] = json!({"retained": 17});
    let raw = Arc::new(serde_json::to_vec_pretty(&value).unwrap());
    let v = Version {
        id: "raw".into(),
        name: "Original".into(),
        description: String::new(),
        time: 1,
        auto: false,
        snapshot: Snapshot::Stored { json: raw.clone(), images: BTreeMap::new() },
    };
    let archived = content(&doc, std::slice::from_ref(&v), &[]);
    assert_eq!(&member_bytes(&archived.encode().unwrap()).into_iter().find(|(n, _)| n == "versions/raw.json").unwrap().1, raw.as_ref());
    let mut newer = v.clone();
    value["futureField"]["retained"] = json!(18);
    newer.snapshot = Snapshot::Stored { json: Arc::new(serde_json::to_vec(&value).unwrap()), images: BTreeMap::new() };
    assert!(!archived.same_persisted_content(&content(&doc, &[newer], &[]), false));
    let mut broken = v;
    broken.snapshot = Snapshot::Stored { json: Arc::new(b"{broken".to_vec()), images: BTreeMap::new() };
    assert!(ArchiveContent::capture(&doc, None, &[broken], &[]).is_err());
}
#[test]
fn ipad_archive_identity_ignores_only_main_navigation_and_thumbnail() {
    let a = Document::new("Pages");
    let mut b = a.clone();
    b.current_page = None;
    assert!(content(&a, &[], &[]).same_persisted_content(&content(&b, &[], &[]), true));
    assert!(!content(&a, &[], &[]).same_persisted_content(&content(&b, &[], &[]), false));
    let va = Version::new("v", "Version", "", 1, Arc::new(a.clone()));
    let vb = Version::new("v", "Version", "", 1, Arc::new(b.clone()));
    assert!(!content(&a, &[va], &[]).same_persisted_content(&content(&a, &[vb], &[]), true));
    let thumbnail = ArchiveContent::capture(&a, Some(b"derived"), &[], &[]).unwrap();
    assert!(thumbnail.same_persisted_content(&content(&a, &[], &[]), true));
}
#[test]
fn ipad_archive_identity_covers_image_bytes_and_branch_metadata() {
    let mut a = Document::new("Images");
    a.images.insert("image".into(), ImageData { bytes: Arc::new(vec![1, 2, 3]), width: 1, height: 1 });
    let mut b = a.clone();
    b.images.get_mut("image").unwrap().bytes = Arc::new(vec![4, 5, 6]);
    assert!(!content(&a, &[], &[]).same_persisted_content(&content(&b, &[], &[]), true));
    let branch = Branch {
        id: "b".into(),
        name: "Branch".into(),
        time: 2,
        client: 7,
        state: BranchState::Active,
        base: Arc::new(a.clone()),
        head: Arc::new(a.clone()),
        versions: Vec::new(),
    };
    let before = content(&a, &[], std::slice::from_ref(&branch));
    let mut later = branch;
    later.name = "Renamed".into();
    assert!(!before.same_persisted_content(&content(&a, &[], &[later]), true));
}
#[test]
fn ipad_archive_builder_preserves_existing_fixture_members() {
    // Canonical hashes of the pre-existing write_json serializer after loading each
    // original fixture. Existing archives acquire normalized/default fields on a
    // normal load/save; comparing raw original JSON would reject that behavior.
    for (name, expected) in [
        ("ridgeline-app.figcraft", "23911fa4e9bf1acfae8d6a5605c734e04b678e5e97c3e5848b39ffd615c6a76b"),
        ("ridgeline-design-system.figcraft", "2c06147e4ffbeb0ea464265dac5ec485dad3139c193e5d9bc00a9f05aa2a949d"),
        ("glass-and-vectors.figcraft", "6351f6b8358cbfedca3e4d01a3876daf2dd7b0b29df2dedb94f103fc9f0a4a77"),
    ] {
        let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)).unwrap();
        let original = member_bytes(&bytes);
        let loaded = load_bytes_with_versions(&bytes).unwrap();
        let thumbnail = original.iter().find(|(n, _)| n == "thumbnail.png").map(|(_, b)| b.as_slice());
        let next = member_bytes(&ArchiveContent::capture(&loaded.doc, thumbnail, &loaded.versions, &loaded.branches).unwrap().encode().unwrap());
        assert_eq!(original.iter().map(|(n, _)| n).collect::<BTreeSet<_>>(), next.iter().map(|(n, _)| n).collect::<BTreeSet<_>>(), "{name}");
        for (entry, bytes) in &next {
            let old = &original.iter().find(|(n, _)| n == entry).unwrap().1;
            if entry == "document.json" {
                use sha2::{Digest, Sha256};
                let value = serde_json::from_slice::<Value>(bytes).unwrap();
                assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap())), expected, "{name}/{entry}");
            } else {
                assert_eq!(old, bytes, "{name}/{entry}");
            }
        }
    }
}
