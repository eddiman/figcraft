use super::*;
use crate::{NoServices, layers_drag_tests::Ui};
use egui::{Event, pos2};
use figcraft_doc::NodeKind;
use figcraft_engine::tools::{PointerEvent, PointerKind};
use serde_json::json;
use std::sync::Arc;

fn context() -> Context {
    let ctx = Context::default();
    ctx.run_ui(egui::RawInput::default(), |_| {}).drop_without_applying_deltas();
    assert!(ctx.input(|input| input.focused));
    ctx
}

fn app() -> FigApp {
    FigApp::new(Session::new(), Box::new(NoServices))
}
fn provisional(app: &mut FigApp, ctx: &Context) {
    app.begin_canvas_pointer(ctx).unwrap();
    app.pointer_session(|s| {
        s.begin_interaction("UI pointer preview")?;
        s.execute("shape.rectangle", &json!({"x": 20, "y": 30, "width": 80, "height": 90}))?;
        Ok(())
    })
    .unwrap();
}
#[test]
fn ipad_ui_pointer_cancel_restores_preview_and_retains_focus() {
    let mut a = app();
    let ctx = context();
    let before = a.session.ds().unwrap().doc.clone();
    let history = a.session.ds().unwrap().history.undo.len();
    let field = egui::Id::new("unrelated field");
    ctx.memory_mut(|m| m.request_focus(field));
    provisional(&mut a, &ctx);
    assert!(!Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
    assert!(a.cancel_canvas_pointer());
    assert!(Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
    assert_eq!(a.session.ds().unwrap().history.undo.len(), history);
    assert_eq!(ctx.memory(|m| m.focused()), Some(field));
    assert!(!a.cancel_canvas_pointer());
}
#[test]
fn ipad_ui_pointer_command_cancels_before_mutation() {
    let mut a = app();
    let ctx = context();
    let count = a.session.doc().unwrap().nodes().count();
    provisional(&mut a, &ctx);
    a.run("shape.rectangle", json!({"x": 500, "y": 50, "width": 30, "height": 40})).unwrap();
    assert!(a.canvas_pointer.is_none());
    assert_eq!(a.session.doc().unwrap().nodes().count(), count + 1);
    assert_eq!(a.finish_canvas_pointer().unwrap(), PointerGestureEnd::Inactive);
}
#[test]
fn ipad_ui_pointer_layout_change_cancels_before_handlers() {
    let mut a = app();
    let ctx = context();
    let before = a.session.ds().unwrap().doc.clone();
    provisional(&mut a, &ctx);
    a.canvas.rect = Rect::from_min_max(pos2(10.0, 20.0), pos2(800.0, 700.0));
    a.canvas_pointer_preflight(&ctx);
    assert!(a.canvas_pointer.is_none());
    assert!(Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
}
#[test]
fn ipad_ui_pointer_normal_finish_publishes_one_undo_step() {
    let mut a = app();
    let ctx = context();
    let history = a.session.ds().unwrap().history.undo.len();
    provisional(&mut a, &ctx);
    a.pointer_session(|s| s.commit_interaction()).unwrap();
    assert_eq!(a.finish_canvas_pointer().unwrap(), PointerGestureEnd::Finished);
    assert_eq!(a.session.ds().unwrap().history.undo.len(), history + 1);
}
#[test]
fn ipad_ui_pointer_error_removes_owner_and_rolls_back() {
    let mut a = app();
    let ctx = context();
    let before = a.session.ds().unwrap().doc.clone();
    provisional(&mut a, &ctx);
    assert!(a.pointer_session(|s| s.execute("not.a.command", &json!({}))).is_err());
    assert!(a.canvas_pointer.is_none());
    assert!(Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
}
#[test]
fn ipad_ui_pointer_empty_live_poll_keeps_local_owner() {
    let mut a = app();
    let ctx = context();
    provisional(&mut a, &ctx);
    assert!(!a.poll_live_with_pointer_barrier());
    assert!(a.canvas_pointer.is_some());
}
#[test]
fn ipad_ui_pointer_tab_switch_restores_source_before_selecting_destination() {
    let mut a = app();
    a.run("file.new", json!({})).unwrap();
    let ctx = context();
    let source = a.session.active;
    let before = a.session.docs[source].doc.clone();
    provisional(&mut a, &ctx);
    a.run("doc.switch", json!({"index": 0})).unwrap();
    assert_eq!(a.session.active, 0);
    assert!(Arc::ptr_eq(&before, &a.session.docs[source].doc));
    assert!(a.canvas_pointer.is_none());
}
#[test]
fn ipad_ui_pointer_default_editor_keeps_legacy_pointer_route() {
    let mut u = Ui::new(Session::new());
    u.app.run("tool.rectangle", json!({})).unwrap();
    let before = u.app.session.doc().unwrap().nodes().count();
    let p = u.app.canvas.rect.center();
    u.button(p, true);
    u.frame(vec![Event::PointerMoved(p + egui::vec2(100.0, 80.0))]);
    u.button(p + egui::vec2(100.0, 80.0), false);
    assert_eq!(u.app.session.doc().unwrap().nodes().count(), before + 1);
    assert!(u.app.canvas_pointer.is_none());
}
#[test]
fn ipad_ui_pointer_external_text_preserves_prior_typing_and_composition() {
    let mut u = Ui::new(Session::new());
    let id = NodeId(u.app.run("text.create", json!({"text": "Before", "x": 20, "y": 20})).unwrap()["id"].as_u64().unwrap());
    figcraft_engine::textedit::begin(&mut u.app.session, id, Some(6), false).unwrap();
    u.frame(vec![Event::Text("A".into())]);
    u.app.begin_canvas_pointer(&u.ctx).unwrap();
    u.app.pointer_session(|s| s.pointer(&PointerEvent::at(PointerKind::Down, 22.0, 22.0, 1.0))).unwrap();
    u.frame(vec![Event::Text("B".into())]);
    let NodeKind::Text(t) = &u.app.session.doc().unwrap().node(id).unwrap().kind else {
        panic!("text");
    };
    assert_eq!(t.characters, "BeforeAB");
    assert!(u.app.canvas_pointer.is_none());
    assert!(u.app.session.ds().unwrap().text_edit.is_some());
}

/// Construct a changed preview under an enrolled checkpoint. Tests install each
/// private UI owner, cancel, then exercise its actual reset/release handler.
pub(crate) fn preview_fixture() -> (FigApp, Context, NodeId) {
    let mut a = app();
    let id = NodeId(a.run("shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap());
    let ctx = context();
    a.canvas.rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
    (a, ctx, id)
}
pub(crate) fn cancel_and_release(mut a: FigApp, ctx: Context, mut handler: impl FnMut(&mut FigApp, &mut egui::Ui)) -> FigApp {
    let before = a.session.ds().unwrap().doc.clone();
    let selection = a.session.selection();
    let depth = a.session.ds().unwrap().history.undo.len();
    a.begin_canvas_pointer(&ctx).unwrap();
    a.pointer_session(|s| {
        s.begin_interaction("UI preview")?;
        s.execute("node.set", &json!({"ids": selection, "props": {"x": 200}}))?;
        Ok(())
    })
    .unwrap();
    assert!(a.cancel_canvas_pointer());
    let p = pos2(400.0, 300.0);
    let raw = egui::RawInput {
        events: vec![
            Event::PointerMoved(p),
            Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
        ],
        ..Default::default()
    };
    ctx.run_ui(raw, |ui| handler(&mut a, ui)).drop_without_applying_deltas();
    assert!(Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
    assert_eq!(a.session.selection(), selection);
    assert_eq!(a.session.ds().unwrap().history.undo.len(), depth);
    a
}

#[test]
fn ipad_ui_pointer_ime_cancels_before_preedit_and_commits_once() {
    let mut u = Ui::new(Session::new());
    let id = NodeId(u.app.run("text.create", json!({"text": "Before", "x": 20, "y": 20})).unwrap()["id"].as_u64().unwrap());
    figcraft_engine::textedit::begin(&mut u.app.session, id, Some(6), false).unwrap();
    u.frame(vec![Event::Text("A".into())]);
    u.app.begin_canvas_pointer(&u.ctx).unwrap();
    u.app.pointer_session(|s| figcraft_engine::textedit::set_caret(s, 0, false)).unwrap();
    u.frame(vec![Event::Ime(egui::ImeEvent::Preedit { text: "候補".into(), active_range_chars: Some(1..1) })]);
    let te = u.app.session.ds().unwrap().text_edit.as_ref().unwrap();
    assert_eq!(te.caret, 7);
    assert_eq!(te.preedit.as_ref().unwrap().text, "候補");
    assert!(u.app.canvas_pointer.is_none());
    u.frame(vec![Event::Ime(egui::ImeEvent::Commit("B".into()))]);
    let NodeKind::Text(t) = &u.app.session.doc().unwrap().node(id).unwrap().kind else {
        panic!("text");
    };
    assert_eq!(t.characters, "BeforeAB");
}

#[test]
fn ipad_ui_pointer_queued_lifecycle_cancel_precedes_next_handler() {
    let mut a = app();
    let ctx = context();
    let before = a.session.ds().unwrap().doc.clone();
    provisional(&mut a, &ctx);
    FigApp::request_canvas_pointer_cancel(&ctx);
    a.canvas_pointer_preflight(&ctx);
    assert!(a.canvas_pointer.is_none());
    assert!(Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
}

#[test]
fn ipad_ui_pointer_focus_loss_restores_before_an_egui_release() {
    let mut a = app();
    let ctx = context();
    let before = a.session.ds().unwrap().doc.clone();
    provisional(&mut a, &ctx);
    ctx.run_ui(egui::RawInput { focused: false, ..Default::default() }, |_| {
        a.canvas_pointer_preflight(&ctx);
    })
    .drop_without_applying_deltas();
    assert!(a.canvas_pointer.is_none());
    assert!(Arc::ptr_eq(&before, &a.session.ds().unwrap().doc));
}

#[test]
fn ipad_ui_pointer_engine_barrier_also_invalidates_the_ui_owner() {
    let mut a = app();
    let ctx = context();
    let count = a.session.doc().unwrap().nodes().count();
    provisional(&mut a, &ctx);
    // A direct engine client has its own barrier, but cannot reach UI owners.
    a.session.execute("shape.rectangle", &json!({"x": 500, "y": 50, "width": 30, "height": 40})).unwrap();
    a.canvas_pointer_preflight(&ctx);
    assert!(a.canvas_pointer.is_none());
    assert_eq!(a.session.doc().unwrap().nodes().count(), count + 1);
    assert!(a.ui.last_error.is_some());
}

#[test]
fn ipad_ui_pointer_passive_native_observer_keeps_legacy_touch_route() {
    use crate::touch_events::{Normalizer, Owner, Routing};
    use egui::{TouchDeviceId, TouchId, TouchPhase};
    let mut u = Ui::new(Session::new());
    u.app.run("tool.rectangle", json!({})).unwrap();
    let count = u.app.session.doc().unwrap().nodes().count();
    let mut observer = Normalizer::default();
    let p = u.app.canvas.rect.center();
    let end = p + egui::vec2(100.0, 80.0);
    let touch = |phase, pos| Event::Touch { device_id: TouchDeviceId(1), id: TouchId(7), phase, pos, force: Some(0.5) };
    let button = |pressed, pos| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    for (time, events) in [
        (1.0, vec![touch(TouchPhase::Start, p), Event::PointerMoved(p), button(true, p)]),
        (2.0, vec![touch(TouchPhase::Move, end), Event::PointerMoved(end)]),
        (3.0, vec![touch(TouchPhase::End, end), button(false, end), Event::PointerGone]),
    ] {
        let mut raw = egui::RawInput { time: Some(time), events, ..Default::default() };
        let frame = observer.process(&mut raw, Routing::Observe, |_| Owner::Suppressed);
        assert!(frame.issues.is_empty());
        u.frame(raw.events);
    }
    assert!(u.app.canvas_pointer.is_none());
    assert_eq!(u.app.session.doc().unwrap().nodes().count(), count + 1);
}

#[test]
fn ipad_ui_pointer_text_selection_survives_command_modal_page_tab_and_close_barriers() {
    use figcraft_engine::textedit;
    for command in ["view.zoomIn", "help.about", "page.select", "doc.switch", "file.close"] {
        let mut u = Ui::new(Session::new());
        let id = NodeId(u.app.run("text.create", json!({"text":"Before","x":20,"y":20})).unwrap()["id"].as_u64().unwrap());
        let page = u.app.session.page().unwrap();
        u.app.run("page.add", json!({})).unwrap();
        u.app.run("page.select", json!({"id":page})).unwrap();
        u.app.run("file.new", json!({})).unwrap();
        u.app.run("doc.switch", json!({"index":0})).unwrap();
        textedit::begin(&mut u.app.session, id, Some(6), false).unwrap();
        textedit::insert(&mut u.app.session, "A").unwrap();
        let uid = u.app.session.ds().unwrap().uid;
        u.app.begin_canvas_pointer(&u.ctx).unwrap();
        u.app.pointer_session(|s| textedit::set_caret(s, 0, false)).unwrap();
        let params = match command {
            "page.select" => json!({"index":1}),
            "doc.switch" => json!({"index":1}),
            "file.close" => json!({"force":true}),
            _ => json!({}),
        };
        u.app.run(command, params).unwrap();
        assert!(u.app.canvas_pointer.is_none(), "{command}");
        if command == "view.zoomIn" {
            u.frame(vec![Event::Text("B".into())]);
            let NodeKind::Text(t) = &u.app.session.doc().unwrap().node(id).unwrap().kind else {
                panic!("text");
            };
            assert_eq!(t.characters, "BeforeAB");
        } else if let Some(source) = u.app.session.docs.iter().find(|d| d.uid == uid) {
            let NodeKind::Text(t) = &source.doc.node(id).unwrap().kind else {
                panic!("text");
            };
            assert_eq!(t.characters, "BeforeA", "{command}");
        }
        let before: Vec<_> = u.app.session.docs.iter().map(|d| (d.uid, d.doc.clone(), d.history.undo.len())).collect();
        let p = u.app.canvas.rect.center();
        u.button(p, false);
        assert_eq!(u.app.session.docs.len(), before.len());
        for (uid, doc, depth) in before {
            let ds = u.app.session.docs.iter().find(|d| d.uid == uid).unwrap();
            assert!(Arc::ptr_eq(&doc, &ds.doc), "late Up after {command}");
            assert_eq!(ds.history.undo.len(), depth);
        }
    }
}

#[test]
fn ipad_ui_pointer_cancel_restores_closed_comment_thread_and_prototype_editor() {
    let (mut a, ctx, id) = preview_fixture();
    a.ui.comments.open = Some(17);
    a.ui.comments.reply = "Keep reply".into();
    a.ui.comments.search = "Keep filter".into();
    crate::proto_panel::open_editor(&ctx, Some((id, 2)));
    a.begin_canvas_pointer(&ctx).unwrap();
    let p = egui::pos2(400.0, 300.0);
    ctx.run_ui(
        egui::RawInput {
            events: vec![
                Event::PointerMoved(p),
                Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            ],
            ..Default::default()
        },
        |_| {
            crate::comments::input(&mut a, &ctx, p);
        },
    )
    .drop_without_applying_deltas();
    assert!(a.ui.comments.open.is_none());
    crate::proto_panel::open_editor(&ctx, Some((id, 7)));
    a.cancel_canvas_pointer();
    assert_eq!(a.ui.comments.open, Some(17));
    assert_eq!(a.ui.comments.reply, "Keep reply");
    assert_eq!(a.ui.comments.search, "Keep filter");
    assert_eq!(crate::proto_panel::editor(&ctx), Some((id, 2)));
}

#[test]
fn ipad_ui_pointer_empty_live_sync_keeps_local_ownership() {
    let mut a = app();
    let ctx = context();
    provisional(&mut a, &ctx);
    a.sync_live_with_pointer_barrier();
    assert!(a.canvas_pointer.is_some());
}
