use std::path::PathBuf;
use std::sync::Arc;

use crate::platform_export_tree::ExportTree;
use crate::platform_requests::*;
use crate::{NoServices, Services};

struct Folder {
    path: Option<PathBuf>,
    calls: usize,
}
impl Services for Folder {
    fn pick_folder(&mut self, _: &str) -> Option<PathBuf> {
        self.calls += 1;
        self.path.take()
    }
}
fn stage() -> Arc<StagingLease> {
    StagingLease::create(&std::env::temp_dir()).unwrap()
}
fn tree(root: Arc<StagingLease>, paths: &[&str]) -> ExportTree {
    ExportTree::new(root, paths.iter().map(|p| (*p).into()).collect()).unwrap()
}
fn populate(tree: &ExportTree) {
    for name in tree.paths() {
        let path = tree.source_path(name).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, name.as_bytes()).unwrap();
    }
}
fn request(tree: ExportTree) -> PlatformRequest {
    PlatformRequest {
        id: RequestId(0),
        initiating_document: Some(DocumentRef { uid: 0 }),
        operation: Operation::ExportTree { picker: PickerSpec { title: "Export".into(), suggested_name: None, extensions: vec![] }, tree },
    }
}

#[test]
fn ipad_export_tree_preserves_engine_nested_paths_and_overwrite() {
    use figcraft_engine::Session;
    use serde_json::json;
    let source = stage();
    let output = stage();
    let mut session = Session::new();
    session.execute("file.new", &json!({})).unwrap();
    let mut ids = Vec::new();
    for name in ["icons/close", "toolbar/close"] {
        let value = session.execute("shape.rectangle", &json!({"name": name, "width": 10, "height": 10})).unwrap();
        let id = value["id"].clone();
        session.execute("export.add", &json!({"ids": [id]})).unwrap();
        ids.push(id);
    }
    let result = session.execute("file.exportLayers", &json!({"ids": ids, "dir": source.root()})).unwrap();
    let paths = result["files"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
    let tree = ExportTree::new(source.clone(), paths).unwrap();
    assert_eq!(tree.paths(), &["icons/close.png", "toolbar/close.png"]);
    std::fs::create_dir_all(output.root().join("icons")).unwrap();
    std::fs::write(output.root().join("icons/close.png"), b"old").unwrap();
    let mut folder = Folder { path: Some(output.root().into()), calls: 0 };
    assert!(matches!(submit_desktop(&mut folder, request(tree)), Submission::Completed(PlatformResult::Completed(Payload::Exported))));
    assert_eq!(folder.calls, 1);
    for name in ["icons/close.png", "toolbar/close.png"] {
        assert_eq!(std::fs::read(output.root().join(name)).unwrap(), std::fs::read(source.root().join(name)).unwrap());
    }
}

#[test]
fn ipad_export_tree_rejects_invalid_names_and_bounds() {
    let root = stage();
    for name in ["", "/a", "a/", "a//b", "./a", "a/../b", "a/./b", "a\\b", "C:/a", "a\0b", "a\nb"] {
        assert!(ExportTree::new(root.clone(), vec![name.into()]).is_err(), "{name:?}");
    }
    assert!(ExportTree::new(root.clone(), vec![]).is_err());
    assert!(ExportTree::new(root.clone(), (0..1001).map(|i| format!("{i}.png")).collect()).is_err());
    assert!(ExportTree::new(root.clone(), vec![vec!["a"; 33].join("/")]).is_err());
    assert!(ExportTree::new(root.clone(), vec![vec!["a"; 32].join("/")]).is_ok());
    assert!(ExportTree::new(root.clone(), (0..1000).map(|i| format!("{i}.png")).collect()).is_ok());
    let tree = tree(root, &["icons/close.png"]);
    assert!(tree.source_path("../outside").is_none());
}

#[test]
fn ipad_export_tree_rejects_duplicate_and_prefix_collisions() {
    let root = stage();
    for names in [vec!["a.png", "a.png"], vec!["A.png", "a.png"], vec!["icons", "icons/close.png"], vec!["ICONS/close.png", "icons"]] {
        assert!(ExportTree::new(root.clone(), names.iter().map(|s| (*s).into()).collect()).is_err());
    }
}

#[test]
fn ipad_export_tree_cancels_and_default_service_is_unsupported() {
    let source = stage();
    let staged = tree(source.clone(), &["icons/close.png"]);
    populate(&staged);
    assert!(matches!(submit_desktop(&mut Folder { path: None, calls: 0 }, request(staged)), Submission::Completed(PlatformResult::Cancelled)));
    assert!(matches!(NoServices.submit_request(request(tree(source, &["icons/close.png"]))), Submission::Rejected(RejectReason::Unsupported)));
}

#[test]
fn ipad_export_tree_preflights_sources_and_destinations() {
    let source = stage();
    let output = stage();
    let staged = tree(source.clone(), &["first.png", "missing.png"]);
    std::fs::write(staged.source_path("first.png").unwrap(), b"first").unwrap();
    let mut folder = Folder { path: Some(output.root().into()), calls: 0 };
    assert!(matches!(submit_desktop(&mut folder, request(staged)), Submission::Completed(PlatformResult::Failed(_))));
    assert_eq!(folder.calls, 0);
    let staged = tree(source, &["first.png", "blocked.png"]);
    populate(&staged);
    std::fs::create_dir(output.root().join("blocked.png")).unwrap();
    assert!(staged.copy_to(output.root()).is_err());
    assert!(!output.root().join("first.png").exists());
}

#[test]
fn ipad_export_tree_retains_sources_until_last_consumer_drops() {
    let source = stage();
    let root = source.root().to_path_buf();
    let staged = tree(source.clone(), &["icons/close.png"]);
    populate(&staged);
    drop(source);
    assert!(root.join("icons/close.png").exists());
    drop(staged);
    assert!(!root.exists());
}

#[test]
fn ipad_export_tree_reports_copy_failure_and_rejects_self_copy() {
    let source = stage();
    let staged = tree(source.clone(), &["icons/close.png"]);
    populate(&staged);
    assert!(staged.copy_to(source.root()).is_err());
    let mut folder = Folder { path: Some(source.root().join("absent")), calls: 0 };
    assert!(matches!(submit_desktop(&mut folder, request(staged)), Submission::Completed(PlatformResult::Failed(_))));
}

#[cfg(unix)]
#[test]
fn ipad_export_tree_rejects_source_and_destination_symlinks() {
    use std::os::unix::fs::symlink;
    let source = stage();
    let output = stage();
    let outside = stage();
    let staged = tree(source.clone(), &["icons/close.png"]);
    populate(&staged);
    std::fs::write(outside.root().join("sentinel"), b"unchanged").unwrap();
    symlink(outside.root(), output.root().join("icons")).unwrap();
    assert!(staged.copy_to(output.root()).is_err());
    assert!(!outside.root().join("close.png").exists());
    std::fs::remove_file(output.root().join("icons")).unwrap();
    std::fs::create_dir(output.root().join("icons")).unwrap();
    symlink(outside.root().join("sentinel"), output.root().join("icons/close.png")).unwrap();
    assert!(staged.copy_to(output.root()).is_err());
    assert_eq!(std::fs::read(outside.root().join("sentinel")).unwrap(), b"unchanged");
    std::fs::remove_file(source.root().join("icons/close.png")).unwrap();
    symlink(outside.root().join("sentinel"), source.root().join("icons/close.png")).unwrap();
    assert!(staged.validate_sources().is_err());
    std::fs::remove_file(source.root().join("icons/close.png")).unwrap();
    std::fs::remove_dir(source.root().join("icons")).unwrap();
    symlink(outside.root(), source.root().join("icons")).unwrap();
    assert!(staged.validate_sources().is_err());
}
