use crate::persistence::*;
use crate::{DocState, Session};
use figcraft_doc::{Document, ImageData, LibraryManifest, NodeId, NodeKind, PublishedLibrary};
use serde_json::json;
use std::sync::Arc;

fn uid(s: &Session) -> u64 {
    s.ds().unwrap().uid
}
fn rect(s: &mut Session, name: &str) {
    s.execute("shape.rectangle", &json!({"x":0,"y":0,"width":10,"height":10})).unwrap();
    s.execute("file.rename", &json!({"name":name})).unwrap();
}
fn ack(s: &mut Session, p: &PreparedSave) -> SaveOutcome {
    // A delayed successful writer receives exactly the captured bytes.
    let bytes = encode_save(p).unwrap();
    assert!(figcraft_format::load_bytes_with_versions(&bytes).is_ok());
    acknowledge_save(s, p, SaveDestination::SaveAs("captured.figcraft".into()), SaveAssurance::AtomicReplacement).unwrap()
}
fn prepare(s: &mut Session) -> Result<PreparedSave, crate::EngineError> {
    let id = uid(s);
    prepare_save(s, id)
}
fn saved(s: &mut Session) {
    let p = prepare_save(s, uid(s)).unwrap();
    assert_eq!(ack(s, &p), SaveOutcome::CurrentSaved);
}
#[test]
fn ipad_save_ack_keeps_new_edits_dirty() {
    let mut s = Session::new();
    rect(&mut s, "A");
    let a = uid(&s);
    let p = prepare_save(&mut s, a).unwrap();
    rect(&mut s, "B");
    let undo = s.ds().unwrap().history.undo.len();
    let selection = s.selection();
    s.execute("file.new", &json!({"name":"Other tab"})).unwrap();
    let shown = uid(&s);
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
    assert_eq!(uid(&s), shown);
    assert!(s.file_is_dirty(a));
    let ds = s.docs.iter().find(|d| d.uid == a).unwrap();
    assert_eq!(ds.doc.name, "B");
    assert_eq!(ds.history.undo.len(), undo);
    assert_eq!(ds.selection, selection);
    let written = figcraft_format::load_bytes_with_versions(&encode_save(&p).unwrap()).unwrap();
    assert_eq!(written.doc.name, "A");
    s.active = 0;
    saved(&mut s);
    assert!(!s.file_is_dirty(a));
}
#[test]
fn ipad_save_ack_keeps_history_dirty() {
    let mut s = Session::new();
    s.execute("file.saveVersion", &json!({"name":"First"})).unwrap();
    saved(&mut s);
    let p = prepare(&mut s).unwrap();
    let revision = s.ds().unwrap().revision;
    s.ds_mut().unwrap().versions[0].name = "Direct history edit".into();
    assert_eq!(s.ds().unwrap().revision, revision);
    assert!(s.ds().unwrap().is_dirty());
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
    assert!(s.file_is_dirty(uid(&s)));
    assert_eq!(figcraft_format::load_bytes_with_versions(&encode_save(&p).unwrap()).unwrap().versions[0].name, "First");
    saved(&mut s);
    assert!(!s.ds().unwrap().is_dirty());
    let p = prepare(&mut s).unwrap();
    s.ds_mut().unwrap().versions.clear();
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
}
#[test]
fn ipad_save_ack_keeps_branch_dirty() {
    let mut s = Session::new();
    let main = uid(&s);
    rect(&mut s, "Main");
    s.execute("branch.create", &json!({"name":"Explore"})).unwrap();
    let branch = uid(&s);
    s.execute("file.saveVersion", &json!({"name":"Branch history"})).unwrap();
    saved(&mut s);
    let p = prepare_save(&mut s, main).unwrap();
    s.ds_mut().unwrap().doc_mut().name = "Later branch edit".into();
    s.ds_mut().unwrap().versions[0].description = "Later description".into();
    assert!(s.file_is_dirty(main));
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
    assert!(s.file_is_dirty(main));
    assert!(s.ds().unwrap().is_dirty());
    let loaded = figcraft_format::load_bytes_with_versions(&encode_save(&p).unwrap()).unwrap();
    assert_eq!(loaded.branches[0].head.name, "Explore");
    assert_eq!(loaded.branches[0].versions[0].description, "");
    saved(&mut s);
    assert!(!s.file_is_dirty(main));
    assert!(!s.docs.iter().find(|d| d.uid == branch).unwrap().is_dirty());
    let recovered = figcraft_format::load_bytes_with_versions(&encode_recovery(&capture_recovery(&s, main).unwrap()).unwrap()).unwrap();
    assert_eq!(recovered.branches[0].head.name, "Later branch edit");
    assert_eq!(recovered.branches[0].versions[0].description, "Later description");
}
#[test]
fn ipad_export_copy_never_marks_saved() {
    let mut s = Session::new();
    rect(&mut s, "Unsaved");
    let before = s.ds().unwrap().path.clone();
    let a = capture_recovery(&s, uid(&s)).unwrap();
    let bytes = encode_recovery(&a).unwrap();
    let export = crate::cmd::file::file_bytes(s.ds().unwrap()).unwrap();
    assert!(s.ds().unwrap().is_dirty());
    assert_eq!(s.ds().unwrap().path, before);
    assert_eq!(s.ds().unwrap().local_save_assurance(), None);
    assert_eq!(figcraft_format::load_bytes(&bytes).unwrap().0.name, "Unsaved");
    assert_eq!(figcraft_format::load_bytes(&export).unwrap().0.name, "Unsaved");
    let doc = s.ds().unwrap().doc.clone();
    s.ds_mut().unwrap().versions.push(figcraft_format::Version::new("direct", "Direct", "", 1, doc));
    let b = capture_recovery(&s, uid(&s)).unwrap();
    assert!(!a.same_capture(&b));
}
#[test]
fn ipad_failed_save_keeps_baseline() {
    let mut s = Session::new();
    saved(&mut s);
    rect(&mut s, "Later");
    let path = s.ds().unwrap().path.clone();
    let saved_doc = s.ds().unwrap().saved_doc.clone();
    let missing = std::env::temp_dir().join(format!("figcraft-missing-save-parent-{}", uid(&s))).join("absent/file.figcraft");
    assert!(s.execute("file.saveAs", &json!({"path":missing})).is_err());
    assert_eq!(s.ds().unwrap().path, path);
    assert!(Arc::ptr_eq(&s.ds().unwrap().saved_doc, &saved_doc));
    assert!(s.ds().unwrap().is_dirty());
}
#[test]
fn ipad_old_ack_cannot_replace_new_baseline() {
    let mut s = Session::new();
    rect(&mut s, "A");
    let a = prepare(&mut s).unwrap();
    rect(&mut s, "B");
    let b = prepare(&mut s).unwrap();
    assert!(a.identity().write_order() < b.identity().write_order());
    assert_eq!(ack(&mut s, &b), SaveOutcome::CurrentSaved);
    assert_eq!(
        acknowledge_save(&mut s, &a, SaveDestination::SaveAs("older.figcraft".into()), SaveAssurance::AtomicReplacement).unwrap(),
        SaveOutcome::Superseded
    );
    assert_eq!(s.ds().unwrap().path.as_deref(), Some("captured.figcraft"));
    assert!(!s.file_is_dirty(uid(&s)));
    assert_eq!(s.prefs.recent_files[0], "captured.figcraft");
}
#[test]
fn ipad_save_page_navigation_and_other_tab_preview() {
    let mut s = Session::new();
    let main = uid(&s);
    let first = s.page().unwrap();
    let second = NodeId(s.execute("page.add", &json!({"name":"Two"})).unwrap()["id"].as_u64().unwrap());
    s.execute("page.select", &json!({"id":first.0})).unwrap();
    let p = prepare_save(&mut s, main).unwrap();
    s.execute("page.select", &json!({"id":second.0})).unwrap();
    assert_eq!(ack(&mut s, &p), SaveOutcome::CurrentSaved);
    assert!(!s.file_is_dirty(main));
    assert_eq!(figcraft_format::load_bytes(&encode_save(&p).unwrap()).unwrap().0.current_page, Some(first));
    let next = prepare_save(&mut s, main).unwrap();
    assert_eq!(figcraft_format::load_bytes(&encode_save(&next).unwrap()).unwrap().0.current_page, Some(second));
    s.execute("branch.create", &json!({"name":"Branch"})).unwrap();
    let branch = uid(&s);
    s.active = 0;
    s.ds_mut().unwrap().start_preview("old", "Old", 1, Arc::new(Document::new("Historical display")));
    let p = prepare_save(&mut s, branch).unwrap();
    assert!(s.docs[0].preview.is_some());
    assert_eq!(uid(&s), main);
    let loaded = figcraft_format::load_bytes_with_versions(&encode_save(&p).unwrap()).unwrap();
    assert_ne!(loaded.doc.name, "Historical display");
    assert_eq!(loaded.doc.current_page, Some(second));
    let r = capture_recovery(&s, main).unwrap();
    assert!(s.docs[0].preview.is_some());
    assert_ne!(figcraft_format::load_bytes(&encode_recovery(&r).unwrap()).unwrap().0.name, "Historical display");
}
#[test]
fn ipad_save_identity_rejects_replaced_and_closed_tabs() {
    let mut s = Session::new();
    let p = prepare(&mut s).unwrap();
    let old = uid(&s);
    let mut replacement = DocState::new(Document::new("Replacement"), None);
    replacement.uid = old;
    s.docs[0] = replacement;
    assert_eq!(ack(&mut s, &p), SaveOutcome::DocumentClosed);
    assert_eq!(s.docs[0].doc.name, "Replacement");
    let p = prepare_save(&mut s, old).unwrap();
    s.docs.clear();
    assert_eq!(ack(&mut s, &p), SaveOutcome::DocumentClosed);
    assert!(s.docs.is_empty());
}
#[test]
fn ipad_save_detects_allocator_published_and_precommand_image_edits() {
    let mut s = Session::new();
    let p = prepare(&mut s).unwrap();
    let before = s.doc().unwrap().clone();
    let page = s.page().unwrap();
    let id = s.doc_mut().unwrap().create(page, NodeKind::Rectangle, "Temporary", 0.0, 0.0, 1.0, 1.0).unwrap();
    s.doc_mut().unwrap().remove(id).unwrap();
    assert_eq!(&before, s.doc().unwrap());
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
    saved(&mut s);
    s.doc_mut().unwrap().published = Some(Arc::new(PublishedLibrary {
        manifest: LibraryManifest { id: "audit".into(), ..Default::default() },
        doc: Arc::new(Document::new("Library")),
    }));
    let p = prepare(&mut s).unwrap();
    Arc::make_mut(&mut Arc::make_mut(s.doc_mut().unwrap().published.as_mut().unwrap()).doc).name = "New library".into();
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
    saved(&mut s);
    let p = prepare(&mut s).unwrap();
    let revision = s.ds().unwrap().revision;
    s.doc_mut().unwrap().images.insert("image".into(), ImageData { bytes: Arc::new(vec![1, 2, 3]), width: 1, height: 1 });
    assert_eq!(s.ds().unwrap().revision, revision);
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
}
#[test]
fn ipad_save_ends_requested_preview_without_destroying_gesture_or_selection() {
    let mut s = Session::new();
    rect(&mut s, "Current");
    let underlying = s.ds().unwrap().doc.clone();
    s.ds_mut().unwrap().text_preview = Some(underlying.clone());
    s.ds_mut().unwrap().doc = Arc::new(Document::new("Hovered preview"));
    let p = prepare(&mut s).unwrap();
    assert!(s.ds().unwrap().text_preview.is_none());
    assert_eq!(figcraft_format::load_bytes(&encode_save(&p).unwrap()).unwrap().0.name, "Current");
    s.begin_interaction("Drag").unwrap();
    s.doc_mut().unwrap().name = "Dragging".into();
    let p = prepare(&mut s).unwrap();
    assert!(s.ds().unwrap().interaction.is_some());
    assert_eq!(ack(&mut s, &p), SaveOutcome::CurrentSaved);
    s.cancel_interaction().unwrap();
    assert!(s.file_is_dirty(uid(&s)));
}

#[test]
fn ipad_local_save_and_server_confirmation_are_independent() {
    use crate::live::Published;
    use crate::live_tests::{push, relay, serve, session};
    let (mut s, script) = session();
    s.execute("collab.host", &json!({})).unwrap();
    let (mut server, me, welcome) = serve(s.doc().unwrap());
    push(&script, vec![welcome]);
    s.live_poll();
    let address = "wss://persistence.example/";
    s.live_mut().unwrap().server = address.into();
    s.live_mut().unwrap().published = Some(Published {
        server: "persistence.example".into(),
        file: "f".into(),
        name: "Audit".into(),
        edit: String::new(),
        view: String::new(),
        owner: None,
        expires: None,
    });
    s.prefs.online_files.push(crate::online::OnlineFile { server: address.into(), file: "f".into(), ..Default::default() });
    rect(&mut s, "Pending online");
    let pending = s.live().client().unwrap().pending();
    assert!(pending > 0);
    saved(&mut s);
    let written = s.ds().unwrap().saved_doc.clone();
    assert_eq!(s.live().client().unwrap().pending(), pending);
    assert!(!s.file_is_dirty(uid(&s)));
    assert!(s.file_needs_recovery(uid(&s)));
    let events = relay(&script, &mut server, me);
    push(&script, events);
    s.live_poll();
    assert_eq!(s.live().client().unwrap().pending(), 0);
    assert!(Arc::ptr_eq(&s.ds().unwrap().saved_doc, &written));
    saved(&mut s);
    rect(&mut s, "Unsaved newer online");
    let missing = std::env::temp_dir().join(format!("figcraft-no-parent-online-{}", uid(&s))).join("absent/output.figcraft");
    assert!(s.execute("file.saveAs", &json!({"path":missing})).is_err());
    let baseline = s.ds().unwrap().saved_doc.clone();
    let events = relay(&script, &mut server, me);
    push(&script, events);
    s.live_poll();
    assert_eq!(s.live().client().unwrap().pending(), 0);
    assert!(s.file_is_dirty(uid(&s)));
    assert!(Arc::ptr_eq(&s.ds().unwrap().saved_doc, &baseline));
    // A fresh welcome still cannot replace an established local-file baseline.
    let (_, _, welcome) = serve(server.document());
    push(&script, vec![welcome]);
    s.live_poll();
    assert!(Arc::ptr_eq(&s.ds().unwrap().saved_doc, &baseline));
    assert!(s.file_is_dirty(uid(&s)));
}
#[test]
fn ipad_save_initialization_and_branch_snapshot_page_are_preserved() {
    let mut s = Session::new();
    let p = prepare(&mut s).unwrap();
    let before = s.ds().unwrap().doc.clone();
    s.set_ui_dark(true);
    assert!(Arc::ptr_eq(&s.ds().unwrap().doc, &before));
    assert_eq!(ack(&mut s, &p), SaveOutcome::CurrentSaved);
    let main = uid(&s);
    s.execute("branch.create", &json!({"name":"Branch"})).unwrap();
    saved(&mut s);
    let p = prepare(&mut s).unwrap();
    s.doc_mut().unwrap().current_page = None;
    assert_eq!(ack(&mut s, &p), SaveOutcome::OlderSnapshotSaved);
    assert!(s.ds().unwrap().is_dirty());
    assert!(s.file_is_dirty(main));
    // Cloning the same logical tab retains its incarnation without reusing orders.
    let cloned = s.ds().unwrap().clone();
    let p = prepare(&mut s).unwrap();
    s.docs[s.active] = cloned;
    let q = prepare(&mut s).unwrap();
    assert!(q.identity().write_order() > p.identity().write_order());
}

#[test]
fn ipad_save_comparison_failure_retains_success_order_and_dirty_state() {
    let mut s = Session::new();
    let older = prepare(&mut s).unwrap();
    let latest = prepare(&mut s).unwrap();
    let doc = s.ds().unwrap().doc.clone();
    s.ds_mut().unwrap().branches.push(figcraft_format::Branch {
        id: "../invalid".into(),
        name: "Later invalid metadata".into(),
        time: 1,
        client: 7,
        state: figcraft_format::BranchState::Active,
        base: doc.clone(),
        head: doc,
        versions: Vec::new(),
    });
    assert_eq!(ack(&mut s, &latest), SaveOutcome::OlderSnapshotSaved);
    assert!(s.ds().unwrap().is_dirty());
    assert!(s.message.as_deref().unwrap().contains("could not be compared"));
    assert_eq!(
        acknowledge_save(&mut s, &older, SaveDestination::SaveAs("old.figcraft".into()), SaveAssurance::AtomicReplacement).unwrap(),
        SaveOutcome::Superseded
    );
    assert_eq!(s.ds().unwrap().path.as_deref(), Some("captured.figcraft"));
    assert!(encode_save(&prepare(&mut s).unwrap()).is_err());
}
