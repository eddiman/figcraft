//! Production pointer tools exercised through the opt-in checkpoint boundary.
use crate::persistence::{SaveAssurance, SaveDestination, SaveOutcome, acknowledge_save, prepare_save};
use crate::tools::{PointerEvent, PointerKind as Kind, ToolId};
use crate::{EngineError, NodeId, PointerGestureEnd as End, PointerGestureToken, Session, params::id_of, textedit};
use serde_json::json;
use std::sync::Arc;

fn at(s: &mut Session, t: &PointerGestureToken, kind: Kind, x: f64, y: f64) {
    s.with_pointer_gesture(t, |s| s.pointer(&PointerEvent::at(kind, x, y, 1.0))).unwrap();
}
fn rect(s: &mut Session) -> NodeId {
    let id = id_of(&s.execute("shape.rectangle", &json!({"x":100,"y":100,"width":100,"height":100})).unwrap()["id"]).unwrap();
    s.ds_mut().unwrap().view.snap_objects = false;
    s.ds_mut().unwrap().view.snap_pixel = false;
    id
}
fn edited_text() -> (Session, NodeId) {
    let mut s = Session::new();
    let id = id_of(&s.execute("text.create", &json!({"text":"Before","x":100,"y":100})).unwrap()["id"]).unwrap();
    textedit::begin(&mut s, id, Some(6), false).unwrap();
    textedit::insert(&mut s, "A").unwrap();
    (s, id)
}
fn text(s: &Session, id: NodeId) -> String {
    s.doc().unwrap().get(id).unwrap().kind.text().unwrap().characters.clone()
}
fn pen_point(s: &mut Session, x: f64, y: f64) {
    s.pointer(&PointerEvent::at(Kind::Down, x, y, 1.0)).unwrap();
    s.pointer(&PointerEvent::at(Kind::Up, x, y, 1.0)).unwrap();
}

#[test]
fn ipad_cancel_draw_restores_document() {
    let mut s = Session::new();
    let id = rect(&mut s);
    s.set_tool(ToolId::Rectangle);
    let before = s.doc().unwrap().clone();
    let selected = s.selection();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 400.0, 400.0);
    at(&mut s, &token, Kind::Move, 500.0, 460.0);
    assert_ne!(*s.doc().unwrap(), before);
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Cancelled);
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.selection(), selected);
    assert!(s.doc().unwrap().contains(id));
    assert_eq!(s.undo_depth(), depth);
    assert!(!s.tool.busy());
    let next = s.begin_pointer_gesture().unwrap();
    at(&mut s, &next, Kind::Down, 500.0, 500.0);
    at(&mut s, &next, Kind::Move, 600.0, 560.0);
    at(&mut s, &next, Kind::Up, 600.0, 560.0);
    assert_eq!(s.finish_pointer_gesture(&next).unwrap(), End::Finished);
    assert_eq!(s.undo_depth().0, depth.0 + 1);
}
#[test]
fn ipad_cancel_move_preserves_selection() {
    let mut s = Session::new();
    let id = rect(&mut s);
    let b = s.doc().unwrap().clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 260.0, 240.0);
    assert_ne!(*s.doc().unwrap(), b);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), b);
    assert_eq!(s.selection(), vec![id]);
    assert_eq!(s.undo_depth(), depth);
    assert!(!s.tool.busy());
}
#[test]
fn ipad_cancel_resize_and_rotate_keep_geometry_and_undo() {
    for (from, to) in [((200.0, 200.0), (250.0, 230.0)), ((208.0, 208.0), (230.0, 150.0))] {
        let mut s = Session::new();
        rect(&mut s);
        let b = s.doc().unwrap().clone();
        let depth = s.undo_depth();
        let token = s.begin_pointer_gesture().unwrap();
        at(&mut s, &token, Kind::Down, from.0, from.1);
        at(&mut s, &token, Kind::Move, to.0, to.1);
        assert_ne!(*s.doc().unwrap(), b, "gesture at {from:?} must change geometry");
        s.cancel_pointer_gesture(&token).unwrap();
        assert_eq!(*s.doc().unwrap(), b);
        assert_eq!(s.undo_depth(), depth);
        assert!(!s.tool.busy());
    }
}
#[test]
fn ipad_cancel_vector_handle() {
    let mut s = Session::new();
    let id = id_of(&s.execute("shape.rectangle", &json!({"x":0,"y":0,"width":100,"height":100})).unwrap()["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id":id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool":"bend"})).unwrap();
    let before = s.doc().unwrap().clone();
    let vector = s.ds().unwrap().vector_edit.clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 100.0, 0.0);
    at(&mut s, &token, Kind::Move, 130.0, 0.0);
    assert_ne!(*s.doc().unwrap(), before);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.ds().unwrap().vector_edit, vector);
    assert_eq!(s.undo_depth(), depth);
    assert!(!s.tool.busy());
}
#[test]
fn ipad_cancel_marquee_restores_pre_down_selection() {
    let mut s = Session::new();
    let id = rect(&mut s);
    let b = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 400.0, 400.0);
    at(&mut s, &token, Kind::Move, 600.0, 600.0);
    assert!(s.selection().is_empty());
    assert!(s.tool.overlay.marquee.is_some());
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.selection(), vec![id]);
    assert_eq!(*s.doc().unwrap(), b);
    assert!(s.tool.overlay.marquee.is_none());
}
#[test]
fn ipad_cancel_pen_point_preserves_accepted_path_and_outer_interaction() {
    let mut s = Session::new();
    s.set_tool(ToolId::Pen);
    pen_point(&mut s, 100.0, 100.0);
    pen_point(&mut s, 200.0, 100.0);
    let before = s.doc().unwrap().clone();
    let outer = s.ds().unwrap().interaction.clone().unwrap();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 260.0, 160.0);
    at(&mut s, &token, Kind::Move, 280.0, 200.0);
    assert_eq!(s.tool.pen.as_ref().unwrap().points.len(), 3);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.tool.pen.as_ref().unwrap().points.len(), 2);
    assert!(!s.tool.busy());
    assert!(Arc::ptr_eq(&s.ds().unwrap().interaction.as_ref().unwrap().doc, &outer.doc));
    assert_eq!(s.undo_depth(), depth);
    s.pen_finish().unwrap();
    assert_eq!(s.undo_depth().0, depth.0 + 1);
    s.undo().unwrap();
    assert_eq!(*s.doc().unwrap(), *outer.doc);
}
#[test]
fn ipad_cancel_freehand_keeps_earlier_stroke_and_no_new_undo() {
    let mut s = Session::new();
    s.set_tool(ToolId::Pencil);
    for (k, x, y) in [(Kind::Down, 100.0, 100.0), (Kind::Move, 130.0, 120.0), (Kind::Up, 160.0, 100.0)] {
        s.pointer(&PointerEvent::at(k, x, y, 1.0)).unwrap();
    }
    let b = s.doc().unwrap().clone();
    let depth = s.undo_depth();
    s.set_tool(ToolId::Pencil);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 300.0, 300.0);
    at(&mut s, &token, Kind::Move, 340.0, 320.0);
    assert!(s.tool.busy());
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), b);
    assert!(!s.tool.busy());
    assert!(s.tool.overlay.pen.is_empty());
    assert_eq!(s.undo_depth(), depth);
}
#[test]
fn ipad_cancel_text_keeps_prior_typing_caret_and_undo_group() {
    let (mut s, id) = edited_text();
    let text_edit = s.ds().unwrap().text_edit.clone();
    let baseline = s.ds().unwrap().interaction.clone().unwrap();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| textedit::set_caret(s, 0, false)).unwrap();
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(text(&s, id), "BeforeA");
    assert_eq!(s.ds().unwrap().text_edit, text_edit);
    assert!(Arc::ptr_eq(&s.ds().unwrap().interaction.as_ref().unwrap().doc, &baseline.doc));
    assert_eq!(s.undo_depth(), depth);
    textedit::end(&mut s).unwrap();
    assert_eq!(s.undo_depth().0, depth.0 + 1);
    s.undo().unwrap();
    assert_eq!(text(&s, id), "Before");
}
#[test]
fn ipad_cancel_before_new_text_and_ime_preserves_input() {
    let (mut s, id) = edited_text();
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| textedit::set_caret(s, 0, false)).unwrap();
    textedit::insert(&mut s, "B").unwrap();
    assert_eq!(text(&s, id), "BeforeAB");
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| textedit::set_caret(s, 0, false)).unwrap();
    textedit::preedit(&mut s, "候補", Some(1)).unwrap();
    assert_eq!(text(&s, id), "BeforeAB");
    let te = s.ds().unwrap().text_edit.as_ref().unwrap();
    assert_eq!(te.caret, 8);
    assert_eq!(te.preedit.as_ref().unwrap().text, "候補");
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
}
#[test]
fn ipad_cancel_new_text_down_removes_provisional_node_and_retains_tool() {
    let mut s = Session::new();
    s.set_tool(ToolId::Text);
    let b = s.doc().unwrap().clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 200.0, 200.0);
    assert!(s.ds().unwrap().text_edit.is_some());
    assert_ne!(*s.doc().unwrap(), b);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), b);
    assert!(s.ds().unwrap().text_edit.is_none());
    assert_eq!(s.tool.tool, ToolId::Text);
    assert_eq!(s.undo_depth(), depth);
}
#[test]
fn ipad_cancel_outside_text_down_restores_the_older_editing_session() {
    let (mut s, id) = edited_text();
    let te = s.ds().unwrap().text_edit.clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 900.0, 900.0);
    assert!(s.ds().unwrap().text_edit.is_none());
    assert_eq!(s.undo_depth().0, depth.0 + 1);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.ds().unwrap().text_edit, te);
    assert_eq!(text(&s, id), "BeforeA");
    assert_eq!(s.undo_depth(), depth);
}
#[test]
fn ipad_cancel_is_idempotent() {
    let mut s = Session::new();
    let first = s.begin_pointer_gesture().unwrap();
    assert_eq!(s.cancel_pointer_gesture(&first).unwrap(), End::Cancelled);
    let second = s.begin_pointer_gesture().unwrap();
    assert_eq!(s.cancel_pointer_gesture(&first).unwrap(), End::Inactive);
    assert_eq!(s.finish_pointer_gesture(&first).unwrap(), End::Inactive);
    assert!(s.with_pointer_gesture(&first, |_| Ok(())).is_err());
    assert_eq!(s.cancel_pointer_gesture(&second).unwrap(), End::Cancelled);
}
#[test]
fn ipad_cancel_does_not_add_undo() {
    let mut s = Session::new();
    rect(&mut s);
    s.execute("shape.ellipse", &json!({"x":500,"y":500,"width":30,"height":30})).unwrap();
    s.undo().unwrap();
    let depth = s.undo_depth();
    assert_eq!(depth.1, 1);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 300.0, 300.0);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.undo_depth(), depth);
    assert!(s.redo().unwrap());
}
#[test]
fn ipad_cancel_normal_up_commits_once() {
    let mut s = Session::new();
    s.set_tool(ToolId::Rectangle);
    let b = s.doc().unwrap().clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 100.0, 100.0);
    at(&mut s, &token, Kind::Move, 200.0, 180.0);
    at(&mut s, &token, Kind::Up, 200.0, 180.0);
    let after = s.doc().unwrap().clone();
    assert_ne!(after, b);
    s.finish_pointer_gesture(&token).unwrap();
    assert_eq!(s.undo_depth().0, depth.0 + 1);
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
    s.undo().unwrap();
    assert_eq!(*s.doc().unwrap(), b);
}
#[test]
fn ipad_cancel_external_command_applies_after_the_restored_geometry() {
    let mut s = Session::new();
    let id = rect(&mut s);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 280.0, 220.0);
    s.execute("node.set", &json!({"id":id,"props":{"width":123}})).unwrap();
    let n = s.doc().unwrap().get(id).unwrap();
    assert_eq!(n.width, 123.0);
    assert_eq!(n.transform.as_coeffs()[4], 100.0);
    assert!(!s.tool.busy());
    assert_eq!(s.finish_pointer_gesture(&token).unwrap(), End::Inactive);
}
#[test]
fn ipad_cancel_command_and_ui_effects_are_deferred_until_finish() {
    let mut s = Session::new();
    let id = rect(&mut s);
    s.journal = vec![("old".into(), json!({})); 5000];
    s.request_ui("existing", json!({"keep":true}));
    let journal = s.journal.clone();
    let requests = s.ui_requests.clone();
    let last = s.last_command.clone();
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| {
        s.execute("node.set", &json!({"id":id,"props":{"width":150}}))?;
        s.request_ui("test.pointer", json!({}));
        Ok(())
    })
    .unwrap();
    assert_eq!(s.journal, journal);
    assert_eq!(s.ui_requests, requests);
    assert_eq!(s.last_command, last);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.journal, journal);
    assert_eq!(s.ui_requests, requests);
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| {
        s.execute("node.set", &json!({"id":id,"props":{"width":140}}))?;
        s.request_ui("test.pointer", json!({}));
        Ok(())
    })
    .unwrap();
    s.finish_pointer_gesture(&token).unwrap();
    assert_eq!(s.journal.len(), 4001);
    assert_eq!(s.journal.last().unwrap().0, "node.set");
    assert_eq!(s.ui_requests.len(), 2);
    assert_eq!(s.ui_requests[0], requests[0]);
}
#[test]
fn ipad_cancel_overflow_fails_without_leaking_requests() {
    let mut s = Session::new();
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    assert!(
        s.with_pointer_gesture(&token, |s| {
            s.doc_mut()?.name = "Provisional".into();
            for _ in 0..1025 {
                s.request_ui("test.pointer", json!({}));
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(*s.doc().unwrap(), before);
    assert!(s.ui_requests.is_empty());
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
}
#[test]
fn ipad_cancel_error_and_panic_restore_and_clear_dispatch_ownership() {
    let mut s = Session::new();
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    let result = s.with_pointer_gesture(&token, |s| {
        s.doc_mut()?.name = "Partial".into();
        Err::<(), _>(EngineError::Other("injected error".into()))
    });
    assert_eq!(result.unwrap_err().to_string(), "injected error");
    assert_eq!(*s.doc().unwrap(), before);
    let token = s.begin_pointer_gesture().unwrap();
    let result = s.with_pointer_gesture(&token, |s| -> crate::Result<()> {
        s.doc_mut()?.name = "Partial panic".into();
        panic!("injected pointer panic");
    });
    assert!(matches!(result, Err(EngineError::Internal { .. })));
    assert_eq!(*s.doc().unwrap(), before);
    let next = s.begin_pointer_gesture().unwrap();
    s.cancel_pointer_gesture(&next).unwrap();
}
#[test]
fn ipad_cancel_nested_dispatch_and_begin_cannot_replace_the_checkpoint() {
    let mut s = Session::new();
    let token = s.begin_pointer_gesture().unwrap();
    assert!(s.begin_pointer_gesture().is_err());
    s.with_pointer_gesture(&token, |s| {
        assert!(s.with_pointer_gesture(&token, |_| Ok(())).is_err());
        assert!(s.cancel_pointer_gesture(&token).is_err());
        assert!(s.finish_pointer_gesture(&token).is_err());
        assert!(s.begin_pointer_gesture().is_err());
        Ok(())
    })
    .unwrap();
    s.cancel_pointer_gesture(&token).unwrap();
}
#[test]
fn ipad_cancel_closed_uid_never_resurrects_a_tab() {
    let mut s = Session::new();
    s.set_tool(ToolId::Rectangle);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 100.0, 100.0);
    at(&mut s, &token, Kind::Move, 200.0, 180.0);
    s.docs.clear();
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::SourceGone);
    assert!(s.docs.is_empty());
    assert!(!s.tool.busy());
    assert!(s.begin_pointer_gesture().is_err());
    assert_eq!(s.finish_pointer_gesture(&token).unwrap(), End::Inactive);
}
#[test]
fn ipad_cancel_late_uid_does_not_restore_another_tabs_tool_or_selection() {
    let mut s = Session::new();
    rect(&mut s);
    let before = s.doc().unwrap().clone();
    let uid = s.ds().unwrap().uid;
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 300.0, 300.0);
    let other = crate::DocState::new(figcraft_doc::Document::new("Other"), None);
    s.docs.push(other);
    s.active = 1;
    s.tool = crate::tools::ToolState { tool: ToolId::Hand, ..Default::default() };
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.active, 1);
    assert_eq!(s.tool.tool, ToolId::Hand);
    assert!(s.selection().is_empty());
    assert_eq!(*s.docs.iter().find(|d| d.uid == uid).unwrap().doc, before);
}
#[test]
fn ipad_cancel_detects_out_of_band_mutation_without_overwriting_it() {
    let mut s = Session::new();
    rect(&mut s);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 280.0, 220.0);
    s.ds_mut().unwrap().doc_mut().name = "Independent".into();
    let changed = s.doc().unwrap().clone();
    assert!(s.cancel_pointer_gesture(&token).is_err());
    assert_eq!(*s.doc().unwrap(), changed);
    assert!(!s.tool.busy());
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
}
#[test]
fn ipad_cancel_rejects_missing_readonly_busy_and_live_sources() {
    assert!(Session::empty().begin_pointer_gesture().is_err());
    let mut s = Session::new();
    let d = s.ds().unwrap().doc.clone();
    s.ds_mut().unwrap().start_preview("v", "Snapshot", 0, d);
    assert!(s.begin_pointer_gesture().is_err());
    s.ds_mut().unwrap().end_preview();
    s.set_tool(ToolId::Rectangle);
    s.pointer(&PointerEvent::at(Kind::Down, 100.0, 100.0, 1.0)).unwrap();
    assert!(s.begin_pointer_gesture().is_err());
    s.escape().unwrap();
    let (mut s, _) = crate::live_tests::session();
    s.execute("collab.publish", &json!({"server":"ws://127.0.0.1:47900/"})).unwrap();
    assert!(s.begin_pointer_gesture().is_err());
}
#[test]
fn ipad_cancel_does_not_overwrite_independent_destination_or_viewport_metadata() {
    let mut s = Session::new();
    let token = s.begin_pointer_gesture().unwrap();
    let ds = s.ds_mut().unwrap();
    ds.path = Some("owned.figcraft".into());
    ds.view.vw = 777.0;
    ds.view.vh = 555.0;
    s.cancel_pointer_gesture(&token).unwrap();
    let ds = s.ds().unwrap();
    assert_eq!(ds.path.as_deref(), Some("owned.figcraft"));
    assert_eq!((ds.view.vw, ds.view.vh), (777.0, 555.0));
}
#[test]
fn ipad_cancel_before_delayed_save_ack_preserves_the_successful_baseline() {
    let mut s = Session::new();
    rect(&mut s);
    let uid = s.ds().unwrap().uid;
    let prepared = prepare_save(&mut s, uid).unwrap();
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 280.0, 220.0);
    let result =
        acknowledge_save(&mut s, &prepared, SaveDestination::SaveAs("successful.figcraft".into()), SaveAssurance::SyncedReplacement).unwrap();
    assert_eq!(result, SaveOutcome::CurrentSaved);
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
    let ds = s.ds().unwrap();
    assert_eq!(ds.path.as_deref(), Some("successful.figcraft"));
    assert_eq!(ds.local_save_assurance(), Some(SaveAssurance::SyncedReplacement));
    assert!(!ds.is_dirty());
}
#[test]
fn ipad_cancel_branch_preview_does_not_leak_into_the_main_archive() {
    let mut s = Session::new();
    let id = rect(&mut s);
    s.execute("branch.create", &json!({"name":"Branch"})).unwrap();
    let main = s.docs[0].branches[0].head.clone();
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| s.execute("node.set", &json!({"id":id,"props":{"width":123}})).map(|_| ())).unwrap();
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.docs[0].branches[0].head, *main);
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| s.execute("node.set", &json!({"id":id,"props":{"width":140}})).map(|_| ())).unwrap();
    s.finish_pointer_gesture(&token).unwrap();
    assert_eq!(s.docs[0].branches[0].head.get(id).unwrap().width, 140.0);
}

#[test]
fn ipad_cancel_existing_preedit_and_caret_survive_selection_cancellation() {
    let (mut s, id) = edited_text();
    textedit::preedit(&mut s, "候補", Some(1)).unwrap();
    let before = s.ds().unwrap().text_edit.clone();
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| textedit::set_caret(s, 0, true)).unwrap();
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.ds().unwrap().text_edit, before);
    assert_eq!(text(&s, id), "BeforeA");
}
#[test]
fn ipad_cancel_before_prepare_save_captures_restored_content_and_refuses_owned_io() {
    let mut s = Session::new();
    rect(&mut s);
    let uid = s.ds().unwrap().uid;
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 280.0, 220.0);
    let prepared = prepare_save(&mut s, uid).unwrap();
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
    let written = crate::persistence::encode_save(&prepared).unwrap();
    let loaded = figcraft_format::load_bytes_with_versions(&written).unwrap();
    assert_eq!(loaded.doc, before);
    let token = s.begin_pointer_gesture().unwrap();
    let result = s.with_pointer_gesture(&token, |s| prepare_save(s, uid).map(|_| ()));
    assert!(result.unwrap_err().to_string().contains("file persistence cannot run"));
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
}
#[test]
fn ipad_cancel_active_editable_live_document_is_explicitly_refused() {
    let (mut s, _) = crate::live_tests::session();
    s.execute("collab.host", &json!({})).unwrap();
    assert_eq!(s.live().tab, Some(s.ds().unwrap().uid));
    assert!(s.active_read_only().is_none());
    assert!(s.begin_pointer_gesture().unwrap_err().to_string().contains("collaboration-aware cancellation"));
}
#[test]
fn ipad_cancel_plain_unrelated_interaction_cannot_be_nested() {
    let mut s = Session::new();
    s.begin_interaction("Other slider").unwrap();
    assert!(s.begin_pointer_gesture().is_err());
    assert_eq!(s.ds().unwrap().interaction.as_ref().unwrap().label, "Other slider");
}
#[test]
fn ipad_cancel_switch_selection_tool_and_direct_write_interrupt_before_mutating() {
    let mut s = Session::new();
    let id = rect(&mut s);
    let before = s.doc().unwrap().clone();
    for action in 0..3 {
        s.set_tool(ToolId::Move);
        s.set_selection(vec![id]);
        let token = s.begin_pointer_gesture().unwrap();
        at(&mut s, &token, Kind::Down, 150.0, 150.0);
        at(&mut s, &token, Kind::Move, 280.0, 220.0);
        match action {
            0 => s.set_selection(Vec::new()),
            1 => s.set_tool(ToolId::Hand),
            _ => s.doc_mut().unwrap().name = "External name".into(),
        }
        assert!(!s.tool.busy());
        assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Inactive);
        assert_eq!(s.doc().unwrap().get(id).unwrap().transform.as_coeffs()[4], 100.0);
        if action == 0 {
            assert!(s.selection().is_empty());
        }
        if action == 1 {
            assert_eq!(s.tool.tool, ToolId::Hand);
        }
        if action == 2 {
            assert_eq!(s.doc().unwrap().name, "External name");
        }
    }
    assert_eq!(s.doc().unwrap().node_count(), before.node_count());
}
#[test]
fn ipad_cancel_comment_request_is_not_exposed_until_the_contact_is_accepted() {
    let mut s = Session::new();
    s.set_tool(ToolId::Comment);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 300.0, 300.0);
    assert!(s.ui_requests.is_empty());
    s.cancel_pointer_gesture(&token).unwrap();
    assert!(s.ui_requests.is_empty());
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 300.0, 300.0);
    at(&mut s, &token, Kind::Up, 300.0, 300.0);
    assert!(s.ui_requests.is_empty());
    s.finish_pointer_gesture(&token).unwrap();
    assert_eq!(s.ui_requests.len(), 1);
    assert_eq!(s.ui_requests[0].kind, "comment.new");
}
#[test]
fn ipad_cancel_eyedropper_restores_tool_colours_and_notice() {
    let mut s = Session::new();
    let id = rect(&mut s);
    s.execute("node.set", &json!({"id":id,"props":{"fill":"#345678"}})).unwrap();
    s.set_tool(ToolId::Eyedropper);
    s.message = Some("Earlier notice".into());
    let colors = s.recent_colors.clone();
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    assert_eq!(s.tool.tool, ToolId::Move);
    assert!(!s.recent_colors.is_empty());
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.tool.tool, ToolId::Eyedropper);
    assert_eq!(s.recent_colors, colors);
    assert_eq!(s.message.as_deref(), Some("Earlier notice"));
    assert_eq!(*s.doc().unwrap(), before);
}
#[test]
fn ipad_cancel_vector_bezier_handle_drag_preserves_subselection() {
    let mut s = Session::new();
    let id = id_of(&s.execute("shape.rectangle", &json!({"x":0,"y":0,"width":100,"height":100})).unwrap()["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id":id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool":"bend"})).unwrap();
    for (k, x, y) in [(Kind::Down, 100.0, 0.0), (Kind::Move, 130.0, 0.0), (Kind::Up, 130.0, 0.0)] {
        s.pointer(&PointerEvent::at(k, x, y, 1.0)).unwrap();
    }
    s.execute("vector.edit.tool", &json!({"tool":"move"})).unwrap();
    let vector = s.ds().unwrap().vector_edit.clone().unwrap();
    let overlay = crate::vectoredit::overlay(s.doc().unwrap(), &vector).unwrap();
    let handle = overlay.handles.first().unwrap().handle;
    let before = s.doc().unwrap().clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, handle[0], handle[1]);
    at(&mut s, &token, Kind::Move, handle[0] + 30.0, handle[1] + 30.0);
    assert_ne!(*s.doc().unwrap(), before);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.ds().unwrap().vector_edit.as_ref(), Some(&vector));
    assert_eq!(s.undo_depth(), depth);
    assert!(!s.tool.busy());
}
#[test]
fn ipad_cancel_replaced_uid_and_cross_session_tokens_do_not_touch_new_documents() {
    let mut s = Session::new();
    let token = s.begin_pointer_gesture().unwrap();
    s.docs[0] = crate::DocState::new(figcraft_doc::Document::new("Replacement"), None);
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::SourceGone);
    assert_eq!(s.doc().unwrap().name, "Replacement");
    let first = s.begin_pointer_gesture().unwrap();
    let mut other = Session::new();
    let second = other.begin_pointer_gesture().unwrap();
    assert!(other.with_pointer_gesture(&first, |_| Ok(())).is_err());
    assert_eq!(other.cancel_pointer_gesture(&first).unwrap(), End::Inactive);
    assert_eq!(other.cancel_pointer_gesture(&second).unwrap(), End::Cancelled);
    s.cancel_pointer_gesture(&first).unwrap();
}

#[test]
fn ipad_cancel_arc_and_corner_radius_restore_shape_metadata() {
    for ellipse in [true, false] {
        let mut s = Session::new();
        s.execute(if ellipse { "shape.ellipse" } else { "shape.rectangle" }, &json!({"x":0,"y":0,"width":100,"height":100})).unwrap();
        let from = if ellipse { s.arc_handles().unwrap().sweep } else { s.shape_handles()[0].1 };
        let to = if ellipse { [50.0, 90.0] } else { [30.0, 28.0] };
        let before = s.doc().unwrap().clone();
        let depth = s.undo_depth();
        let token = s.begin_pointer_gesture().unwrap();
        at(&mut s, &token, Kind::Down, from[0], from[1]);
        at(&mut s, &token, Kind::Move, to[0], to[1]);
        assert_ne!(*s.doc().unwrap(), before);
        s.cancel_pointer_gesture(&token).unwrap();
        assert_eq!(*s.doc().unwrap(), before);
        assert_eq!(s.undo_depth(), depth);
        assert!(s.tool.overlay.arc.is_none() && s.tool.overlay.shape.is_none());
    }
}
#[test]
fn ipad_cancel_rotation_origin_and_hand_pan_restore_view_and_tool_state() {
    let mut s = Session::new();
    rect(&mut s);
    s.execute("transform.rotationOrigin", &json!({})).unwrap();
    let origin = s.ds().unwrap().rotation_origin.clone();
    let from = s.ds().unwrap().origin_handle().unwrap();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, from.x, from.y);
    at(&mut s, &token, Kind::Move, 105.0, 105.0);
    assert_ne!(s.ds().unwrap().rotation_origin, origin);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.ds().unwrap().rotation_origin, origin);
    s.set_tool(ToolId::Hand);
    let view = s.ds().unwrap().view.clone();
    let depth = s.undo_depth();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 300.0, 300.0);
    at(&mut s, &token, Kind::Move, 400.0, 350.0);
    assert_ne!(s.ds().unwrap().view, view);
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(s.ds().unwrap().view, view);
    assert_eq!(s.undo_depth(), depth);
    assert_eq!(s.tool.tool, ToolId::Hand);
}
#[test]
fn ipad_cancel_closed_pen_source_releases_all_old_path_state() {
    let mut s = Session::new();
    s.set_tool(ToolId::Pen);
    pen_point(&mut s, 100.0, 100.0);
    pen_point(&mut s, 200.0, 100.0);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 250.0, 150.0);
    s.docs.clear();
    s.cancel_pointer_gesture(&token).unwrap();
    assert!(s.tool.pen.is_none());
    assert!(!s.tool.busy());
    assert!(s.tool.overlay.pen.is_empty());
}
#[test]
fn ipad_cancel_owned_revision_exhaustion_does_not_panic_or_wrap() {
    let mut s = Session::new();
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    s.with_pointer_gesture(&token, |s| {
        s.doc_mut()?.name = "Provisional".into();
        s.ds_mut()?.revision = u64::MAX;
        Ok(())
    })
    .unwrap();
    s.cancel_pointer_gesture(&token).unwrap();
    assert_eq!(*s.doc().unwrap(), before);
    assert_eq!(s.ds().unwrap().revision, u64::MAX);
    assert!(!s.tool.busy());
}
#[test]
fn ipad_cancel_stale_drag_discards_its_interaction_before_the_next_normal_edit() {
    let mut s = Session::new();
    let id = rect(&mut s);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 280.0, 220.0);
    s.ds_mut().unwrap().doc_mut().name = "External".into();
    assert!(s.cancel_pointer_gesture(&token).is_err());
    assert!(s.ds().unwrap().interaction.is_none());
    s.execute("node.set", &json!({"id":id,"props":{"width":140}})).unwrap();
    s.undo().unwrap();
    assert_eq!(s.doc().unwrap().name, "External");
    assert_eq!(s.doc().unwrap().get(id).unwrap().width, 100.0);
}
#[test]
fn ipad_cancel_late_finish_cannot_publish_ui_to_another_tab() {
    let mut s = Session::new();
    s.set_tool(ToolId::Comment);
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 300.0, 300.0);
    at(&mut s, &token, Kind::Up, 300.0, 300.0);
    s.docs.push(crate::DocState::new(figcraft_doc::Document::new("Other"), None));
    s.active = 1;
    s.tool = crate::tools::ToolState { tool: ToolId::Hand, ..Default::default() };
    assert!(s.finish_pointer_gesture(&token).is_err());
    assert!(s.ui_requests.is_empty());
    assert_eq!(s.active, 1);
    assert_eq!(s.doc().unwrap().name, "Other");
    assert_eq!(s.tool.tool, ToolId::Hand);
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Cancelled);
    assert!(s.ui_requests.is_empty());
    assert_eq!(s.tool.tool, ToolId::Hand);
}

#[test]
fn ipad_cancel_without_an_active_tab_restores_source_and_clears_its_drag() {
    let mut s = Session::new();
    rect(&mut s);
    let before = s.doc().unwrap().clone();
    let token = s.begin_pointer_gesture().unwrap();
    at(&mut s, &token, Kind::Down, 150.0, 150.0);
    at(&mut s, &token, Kind::Move, 280.0, 220.0);
    s.active = usize::MAX;
    assert_eq!(s.cancel_pointer_gesture(&token).unwrap(), End::Cancelled);
    assert_eq!(*s.docs[0].doc, before);
    assert!(!s.tool.busy());
    assert_eq!(s.active, usize::MAX);
    s.active = 0;
    let next = s.begin_pointer_gesture().unwrap();
    s.cancel_pointer_gesture(&next).unwrap();
}
