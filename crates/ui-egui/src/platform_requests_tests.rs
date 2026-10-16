//! Delayed service tests exercise real FigApp dispatch and engine mutations.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use figcraft_engine::Session;
use serde_json::json;

use crate::platform::PlatformCapabilities;
use crate::platform_requests::*;
use crate::{FigApp, NoServices, Services};

#[derive(Default)]
struct State {
    submitted: Vec<PlatformRequest>,
    ready: VecDeque<Completion>,
    cancelled: Vec<RequestId>,
    next: Option<Submission>,
    repaint: Option<egui::Context>,
}
struct Fake(Rc<RefCell<State>>);
impl Services for Fake {
    fn submit_request(&mut self, request: PlatformRequest) -> Submission {
        let mut s = self.0.borrow_mut();
        s.submitted.push(request);
        s.next.take().unwrap_or(Submission::Pending)
    }
    fn take_completion(&mut self) -> Option<Completion> {
        self.0.borrow_mut().ready.pop_front()
    }
    fn cancel_request(&mut self, id: RequestId) {
        self.0.borrow_mut().cancelled.push(id);
    }
    fn set_request_repaint(&mut self, ctx: egui::Context) {
        self.0.borrow_mut().repaint = Some(ctx);
    }
}
struct Captured(Rc<Cell<usize>>);
impl Drop for Captured {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn setup() -> (FigApp, Rc<RefCell<State>>, egui::Context) {
    let state = Rc::new(RefCell::new(State::default()));
    (FigApp::new(Session::new(), Box::new(Fake(state.clone()))), state, egui::Context::default())
}
fn spec() -> PickerSpec {
    PickerSpec { title: "Test".into(), suggested_name: None, extensions: vec!["figcraft".into()] }
}
fn target(app: &FigApp) -> DocumentRef {
    DocumentRef { uid: app.session.ds().unwrap().uid }
}
fn selected() -> PlatformResult {
    PlatformResult::Completed(Payload::Selected(SelectedCopy { path: "synthetic.figcraft".into(), display_name: "synthetic".into(), lease: None }))
}
fn edit(drops: Rc<Cell<usize>>) -> PlatformDone {
    let capture = Captured(drops);
    Box::new(move |app, _| {
        let _hold = &capture;
        assert!(app.run("shape.rectangle", json!({"x": 1, "y": 2, "width": 3, "height": 4})).is_some());
    })
}
fn submit(app: &mut FigApp, ctx: &egui::Context, drops: Rc<Cell<usize>>) -> RequestId {
    app.submit_platform_request(ctx, Some(target(app)), Operation::ImportCopy(spec()), edit(drops)).unwrap()
}
fn complete(state: &Rc<RefCell<State>>, id: RequestId, result: PlatformResult) {
    let mut s = state.borrow_mut();
    s.ready.push_back(Completion { id, result });
    s.repaint.as_ref().unwrap().request_repaint();
}

#[test]
fn ipad_request_tab_switch() {
    let (mut app, state, ctx) = setup();
    let uid = target(&app).uid;
    let drops = Rc::new(Cell::new(0));
    let id = submit(&mut app, &ctx, drops.clone());
    app.run("file.new", json!({})).unwrap();
    let other = target(&app).uid;
    complete(&state, id, selected());
    app.poll_platform_requests(&ctx);
    assert_eq!(target(&app).uid, other);
    assert_eq!(app.session.docs.iter().find(|d| d.uid == uid).unwrap().history.undo.len(), 1);
    assert!(app.session.ds().unwrap().history.undo.is_empty());
    assert_eq!(app.session.docs.iter().find(|d| d.uid == uid).unwrap().doc.nodes().count(), app.session.doc().unwrap().nodes().count() + 1);
    assert_eq!(drops.get(), 1);
}

#[test]
fn ipad_request_late_completion_after_close() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let id = submit(&mut app, &ctx, drops.clone());
    app.run("file.close", json!({"force": true})).unwrap();
    assert_eq!(state.borrow().cancelled, vec![id]);
    assert_eq!(drops.get(), 1);
    complete(&state, id, selected());
    app.poll_platform_requests(&ctx);
    assert!(app.session.docs.is_empty());
    assert_eq!(drops.get(), 1);
}

#[test]
fn ipad_request_cancel() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let original = app.session.doc().unwrap().clone();
    let id = submit(&mut app, &ctx, drops.clone());
    complete(&state, id, PlatformResult::Cancelled);
    app.poll_platform_requests(&ctx);
    assert_eq!(*app.session.doc().unwrap(), original);
    assert!(app.session.ds().unwrap().history.undo.is_empty());
    assert!(app.ui.last_error.is_none());
    assert_eq!(drops.get(), 1);
}

#[test]
fn ipad_request_duplicate() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let id = submit(&mut app, &ctx, drops.clone());
    complete(&state, id, selected());
    complete(&state, id, selected());
    app.poll_platform_requests(&ctx);
    assert_eq!(app.session.ds().unwrap().history.undo.len(), 1);
    assert_eq!(drops.get(), 1);
}

#[test]
fn ipad_request_queue_full() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let ids: Vec<_> = (0..8).map(|_| submit(&mut app, &ctx, drops.clone())).collect();
    let ninth = app.submit_platform_request(&ctx, Some(target(&app)), Operation::ImportCopy(spec()), edit(drops.clone()));
    assert_eq!(ninth, Err(RejectReason::Busy));
    assert_eq!(state.borrow().submitted.len(), 8);
    assert_eq!(drops.get(), 1);
    // All reserved completions survive queue pressure, in reverse arrival order.
    for id in ids.into_iter().rev() {
        complete(&state, id, selected());
    }
    app.poll_platform_requests(&ctx);
    assert_eq!(app.session.ds().unwrap().history.undo.len(), 8);
    assert_eq!(drops.get(), 9);
}

#[test]
fn ipad_unsupported_distinct_from_cancel() {
    let (mut app, state, ctx) = setup();
    state.borrow_mut().next = Some(Submission::Rejected(RejectReason::Unsupported));
    let drops = Rc::new(Cell::new(0));
    assert_eq!(
        app.submit_platform_request(&ctx, Some(target(&app)), Operation::ImportCopy(spec()), edit(drops.clone())),
        Err(RejectReason::Unsupported)
    );
    assert!(app.ui.last_error.as_ref().unwrap().contains("Unsupported"));
    assert_eq!(drops.get(), 1);
    assert!(app.session.ds().unwrap().history.undo.is_empty());
    app.ui.last_error = None;
    state.borrow_mut().next = Some(Submission::Completed(PlatformResult::Cancelled));
    submit(&mut app, &ctx, drops.clone());
    assert!(app.ui.last_error.is_none());
    assert_eq!(drops.get(), 2);
}

#[test]
fn ipad_request_failed_and_malformed_do_not_edit() {
    for result in [PlatformResult::Failed("permission denied".into()), PlatformResult::Completed(Payload::Exported)] {
        let (mut app, state, ctx) = setup();
        let drops = Rc::new(Cell::new(0));
        let before = app.session.doc().unwrap().clone();
        let id = submit(&mut app, &ctx, drops.clone());
        complete(&state, id, result);
        app.poll_platform_requests(&ctx);
        assert_eq!(*app.session.doc().unwrap(), before);
        assert!(app.session.ds().unwrap().history.undo.is_empty());
        assert!(app.ui.last_error.is_some());
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn ipad_request_shutdown_and_explicit_cancel() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let id = submit(&mut app, &ctx, drops.clone());
    app.cancel_platform_request(id);
    app.cancel_platform_request(id);
    let second = submit(&mut app, &ctx, drops.clone());
    drop(app);
    assert_eq!(state.borrow().cancelled, vec![id, second]);
    assert_eq!(drops.get(), 2);
}

#[test]
fn ipad_request_immediate_and_open_copy() {
    let (mut app, state, ctx) = setup();
    state.borrow_mut().next = Some(Submission::Completed(selected()));
    app.submit_platform_request(
        &ctx,
        None,
        Operation::OpenCopy(spec()),
        Box::new(|app, _| {
            app.run("file.new", json!({})).unwrap();
        }),
    )
    .unwrap();
    assert_eq!(app.session.docs.len(), 2);
    assert_eq!(app.session.active, 1);
}

#[test]
fn ipad_request_ids_and_invalid_target() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let first = submit(&mut app, &ctx, drops.clone());
    let second = submit(&mut app, &ctx, drops.clone());
    assert!(second.0 > first.0);
    assert_eq!(exhausted_id_for_test(), Err(RejectReason::IdExhausted));
    assert!(app.submit_platform_request(&ctx, None, Operation::ImportCopy(spec()), edit(drops.clone())).is_err());
    assert_eq!(state.borrow().submitted.len(), 2);
}

#[test]
fn ipad_request_staging_lives_until_consumer_releases_it() {
    let (mut app, state, ctx) = setup();
    let stage = StagingLease::create(&std::env::temp_dir()).unwrap();
    assert!(stage.file("../escape".into()).is_err());
    let file = stage.file("sample.png".into()).unwrap();
    let path = file.path.clone();
    std::fs::write(&path, b"original synthetic content").unwrap();
    let id = app
        .submit_platform_request(&ctx, Some(target(&app)), Operation::ExportFiles { picker: spec(), files: vec![file] }, Box::new(|_, _| {}))
        .unwrap();
    drop(stage);
    app.cancel_platform_request(id);
    // Native/service ownership can outlive UI cancellation.
    assert!(path.exists());
    state.borrow_mut().submitted.clear();
    assert!(!path.exists());
    // Late completion payload owns another lease and must also be released.
    let late = StagingLease::create(&std::env::temp_dir()).unwrap();
    let file = late.file("late.figcraft".into()).unwrap();
    let late_path = file.path.clone();
    std::fs::write(&late_path, b"synthetic").unwrap();
    complete(
        &state,
        id,
        PlatformResult::Completed(Payload::Selected(SelectedCopy { path: late_path.clone(), display_name: "late".into(), lease: Some(late) })),
    );
    drop(file);
    app.poll_platform_requests(&ctx);
    assert!(!late_path.exists());
}

#[test]
fn ipad_capabilities_and_window_dispatch() {
    struct IPad;
    impl Services for IPad {
        fn capabilities(&self) -> PlatformCapabilities {
            PlatformCapabilities::ipad()
        }
    }
    let mut app = FigApp::new(Session::new(), Box::new(IPad));
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    app.run("window.new", json!({})).unwrap();
    crate::windows::requests(&mut app, &ctx);
    assert!(app.windows.list.is_empty());
    assert_eq!(app.session.docs.len(), 2); // existing one-window tab fallback
    assert!(!PlatformCapabilities::web().local_session_hosting);
    assert!(NoServices.capabilities().multiple_windows); // legacy embedding behavior
    assert!(matches!(
        NoServices.submit_request(PlatformRequest { id: RequestId(0), initiating_document: None, operation: Operation::OpenCopy(spec()) }),
        Submission::Rejected(RejectReason::Unsupported)
    ));
}

#[test]
fn ipad_request_logic_drains_and_direct_close_is_safe() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let id = submit(&mut app, &ctx, drops.clone());
    complete(&state, id, selected());
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.logic(ui.ctx()));
    output.textures_delta.clear();
    assert_eq!(app.session.ds().unwrap().history.undo.len(), 1);
    let second = submit(&mut app, &ctx, drops.clone());
    app.session.execute("file.close", &json!({"force": true})).unwrap();
    complete(&state, second, selected());
    app.poll_platform_requests(&ctx);
    assert!(app.session.docs.is_empty());
    assert_eq!(state.borrow().cancelled, vec![second]);
    assert_eq!(drops.get(), 2);
}

#[test]
fn ipad_desktop_adapter_exports_captured_bytes_and_cancels() {
    struct Desktop {
        output: Option<std::path::PathBuf>,
    }
    impl Services for Desktop {
        fn pick_save(&mut self, _: &str, _: &str, _: &[&str]) -> Option<std::path::PathBuf> {
            self.output.take()
        }
    }
    let stage = StagingLease::create(&std::env::temp_dir()).unwrap();
    let output = stage.file("output.png".into()).unwrap();
    let source = stage.file("snapshot.png".into()).unwrap();
    std::fs::write(&source.path, b"captured export bytes").unwrap();
    let request = PlatformRequest {
        id: RequestId(0),
        initiating_document: Some(DocumentRef { uid: 0 }),
        operation: Operation::ExportFiles { picker: spec(), files: vec![source] },
    };
    assert!(matches!(
        submit_desktop(&mut Desktop { output: Some(output.path.clone()) }, request),
        Submission::Completed(PlatformResult::Completed(Payload::Exported))
    ));
    assert_eq!(std::fs::read(&output.path).unwrap(), b"captured export bytes");
    let request = PlatformRequest { id: RequestId(1), initiating_document: Some(DocumentRef { uid: 0 }), operation: Operation::SaveAs(spec()) };
    assert!(matches!(submit_desktop(&mut Desktop { output: None }, request), Submission::Completed(PlatformResult::Cancelled)));
}

#[test]
fn ipad_request_poll_bounds_duplicate_stream() {
    let (mut app, state, ctx) = setup();
    let drops = Rc::new(Cell::new(0));
    let id = submit(&mut app, &ctx, drops.clone());
    for _ in 0..100 {
        complete(&state, id, selected());
    }
    app.poll_platform_requests(&ctx);
    assert!(!state.borrow().ready.is_empty());
    assert_eq!(app.session.ds().unwrap().history.undo.len(), 1);
    while !state.borrow().ready.is_empty() {
        app.poll_platform_requests(&ctx);
    }
    assert_eq!(app.session.ds().unwrap().history.undo.len(), 1);
    assert_eq!(drops.get(), 1);
}

#[test]
fn ipad_export_tree_request_routes_completion_to_initiating_document() {
    use crate::platform_export_tree::ExportTree;
    let (mut app, state, ctx) = setup();
    let origin = target(&app);
    let stage = StagingLease::create(&std::env::temp_dir()).unwrap();
    let tree = ExportTree::new(stage, vec!["icons/close.png".into()]).unwrap();
    let drops = Rc::new(Cell::new(0));
    let id = app.submit_platform_request(&ctx, Some(origin), Operation::ExportTree { picker: spec(), tree }, edit(drops.clone())).unwrap();
    app.run("file.new", json!({})).unwrap();
    let other = target(&app);
    complete(&state, id, PlatformResult::Completed(Payload::Exported));
    app.poll_platform_requests(&ctx);
    assert_eq!(target(&app), other);
    assert!(app.session.ds().unwrap().history.undo.is_empty());
    assert_eq!(app.session.docs.iter().find(|d| d.uid == origin.uid).unwrap().history.undo.len(), 1);
    assert_eq!(drops.get(), 1);
}
