//! Specialized Native integration. A cloned pre-Down engine supplies the normal
//! external command outcome; cancellation must preserve that outcome exactly.
use super::combined_tests::Host;
use super::*;
use egui::{Event, Modifiers, PointerButton, TouchDeviceId, TouchId, TouchPhase, vec2};
use figcraft_doc::NodeId;
use figcraft_engine::{
    Session,
    tools::{PointerEvent, PointerKind},
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cause {
    Second,
    Cancel,
    Focus,
    Allocation,
    Viewport,
    Modal,
    Switch,
    Close,
    Unavailable,
}
const CAUSES: [Cause; 9] =
    [Cause::Second, Cause::Cancel, Cause::Focus, Cause::Allocation, Cause::Viewport, Cause::Modal, Cause::Switch, Cause::Close, Cause::Unavailable];
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fixture {
    Prototype,
    Pen,
    Text,
    VectorBend,
    VectorHandle,
    CommentPin,
}
fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: None }
}
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}
fn id(value: serde_json::Value) -> NodeId {
    NodeId(value["id"].as_u64().unwrap())
}
fn seed(fixture: Fixture, cause: Cause) -> (Host, Pos2, Pos2) {
    let mut h = Host::new();
    h.ui.app.run("tool.move", json!({})).unwrap();
    // Create the destination before any multi-click pen/text interaction exists.
    if cause == Cause::Switch {
        h.ui.app.run("file.new", json!({})).unwrap();
        h.run(vec![]);
        h.ui.app.run("doc.switch", json!({"index":0})).unwrap();
        h.run(vec![]);
    }
    {
        let view = &mut h.ui.app.session.ds_mut().unwrap().view;
        view.cx = 0.0;
        view.cy = 0.0;
    }
    let (from, to) = match fixture {
        Fixture::Prototype => {
            let source = id(h.ui.app.run("shape.rectangle", json!({"x":-150,"y":-50,"width":100,"height":100})).unwrap());
            h.ui.app.run("frame.create", json!({"x":100,"y":-50,"width":120,"height":100})).unwrap();
            h.ui.app.run("select.set", json!({"ids":[source]})).unwrap();
            h.ui.app.ui.right_tab = crate::RightTab::Prototype;
            (h.ui.app.to_screen(-50.0, 0.0) + vec2(11.0, 0.0), h.ui.app.to_screen(160.0, 0.0))
        }
        Fixture::Pen => {
            h.ui.app.run("tool.pen", json!({})).unwrap();
            h.run(vec![]);
            for point in [(-150.0, -60.0), (-80.0, -60.0)] {
                h.pointer = h.ui.app.to_screen(point.0, point.1);
                h.start();
                h.end();
            }
            assert_eq!(h.ui.app.session.tool.pen.as_ref().unwrap().points.len(), 2);
            (h.ui.app.to_screen(0.0, 0.0), h.ui.app.to_screen(30.0, 40.0))
        }
        Fixture::Text => {
            let text = id(h.ui.app.run("text.create", json!({"text":"Before","x":-150,"y":-30,"width":300,"props":{"fontSize":24}})).unwrap());
            figcraft_engine::textedit::begin(&mut h.ui.app.session, text, Some(6), false).unwrap();
            h.run(vec![Event::Text("A".into()), Event::Ime(egui::ImeEvent::Preedit { text: "Synthetic".into(), active_range_chars: Some(2..2) })]);
            assert!(h.ui.app.session.ds().unwrap().text_edit.as_ref().unwrap().preedit.is_some());
            (h.ui.app.to_screen(-145.0, -18.0), h.ui.app.to_screen(-80.0, -18.0))
        }
        Fixture::VectorBend | Fixture::VectorHandle => {
            let shape = id(h.ui.app.run("shape.rectangle", json!({"x":-50,"y":-50,"width":100,"height":100})).unwrap());
            h.ui.app.run("vector.edit.begin", json!({"id":shape})).unwrap();
            h.ui.app.run("vector.edit.tool", json!({"tool":"bend"})).unwrap();
            if fixture == Fixture::VectorHandle {
                for (kind, x, y) in [(PointerKind::Down, 50.0, -50.0), (PointerKind::Move, 80.0, -50.0), (PointerKind::Up, 80.0, -50.0)] {
                    h.ui.app.session.pointer(&PointerEvent::at(kind, x, y, 1.0)).unwrap();
                }
                h.ui.app.run("vector.edit.tool", json!({"tool":"move"})).unwrap();
                let ds = h.ui.app.session.ds().unwrap();
                let overlay = figcraft_engine::vectoredit::overlay(&ds.doc, ds.vector_edit.as_ref().unwrap()).unwrap();
                let p = overlay.handles.first().unwrap().handle;
                (h.ui.app.to_screen(p[0], p[1]), h.ui.app.to_screen(p[0] + 30.0, p[1] + 30.0))
            } else {
                (h.ui.app.to_screen(50.0, -50.0), h.ui.app.to_screen(80.0, -50.0))
            }
        }
        Fixture::CommentPin => {
            h.ui.app.run("tool.comment", json!({})).unwrap();
            h.ui.app.run("comment.add", json!({"x":0,"y":0,"text":"Synthetic matrix pin"})).unwrap();
            let p = h.ui.app.to_screen(0.0, 0.0) + vec2(14.0, -14.0);
            (p, p + vec2(70.0, 50.0))
        }
    };
    h.run(vec![]);
    h.pointer = from;
    (h, from, to)
}
fn control(h: &Host) -> Session {
    let mut s = Session::empty();
    s.docs = h.ui.app.session.docs.clone();
    s.active = h.ui.app.session.active;
    s.tool = h.ui.app.session.tool.clone();
    s.prefs = h.ui.app.session.prefs.clone();
    s
}
fn interrupt(h: &mut Host, expected: &mut Session, cause: Cause) {
    match cause {
        Cause::Second => h.run(vec![touch(2, TouchPhase::Start, h.pointer + vec2(60.0, 20.0))]),
        Cause::Cancel => h.run(vec![touch(1, TouchPhase::Cancel, h.pointer), Event::PointerGone]),
        Cause::Focus => h.run(vec![Event::WindowFocused(false)]),
        Cause::Allocation => {
            h.ui.app.canvas.rect.max.x -= 20.0;
            h.run(vec![]);
        }
        Cause::Viewport => h.run_custom(vec![], &[], |raw| raw.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(900.0, 1440.0)))),
        Cause::Modal | Cause::Switch | Cause::Close => {
            let (command, params) = match cause {
                Cause::Modal => ("help.about", json!({})),
                Cause::Switch => ("doc.switch", json!({"index":1})),
                _ => ("file.close", json!({"force":true})),
            };
            expected.execute(command, &params).unwrap();
            h.ui.app.run(command, params).unwrap();
            h.run(vec![]);
        }
        Cause::Unavailable => {
            h.unavailable(vec![]);
            h.run(vec![]);
        }
    }
}
fn assert_engine(actual: &Session, expected: &Session, fixture: Fixture, cause: Cause) {
    assert_eq!(actual.docs.len(), expected.docs.len(), "{fixture:?}/{cause:?}");
    for e in &expected.docs {
        let a = actual.docs.iter().find(|a| a.uid == e.uid).unwrap();
        assert_eq!(*a.doc, *e.doc, "{fixture:?}/{cause:?}: document");
        assert_eq!(a.selection, e.selection, "{fixture:?}/{cause:?}: selection");
        assert_eq!(a.text_edit, e.text_edit, "{fixture:?}/{cause:?}: text/preedit");
        assert_eq!(a.vector_edit, e.vector_edit, "{fixture:?}/{cause:?}: vector sub-selection");
        assert_eq!((a.history.undo.len(), a.history.redo.len()), (e.history.undo.len(), e.history.redo.len()), "{fixture:?}/{cause:?}: history");
        assert_eq!((a.view.cx, a.view.cy, a.view.zoom), (e.view.cx, e.view.cy, e.view.zoom), "{fixture:?}/{cause:?}: view");
        match (&a.interaction, &e.interaction) {
            (Some(a), Some(e)) => assert!(Arc::ptr_eq(&a.doc, &e.doc), "{fixture:?}/{cause:?}: original interaction"),
            (None, None) => {}
            _ => panic!("{fixture:?}/{cause:?}: interaction changed"),
        }
    }
    assert_eq!(actual.tool.tool, expected.tool.tool, "{fixture:?}/{cause:?}: tool");
    assert_eq!(
        actual.tool.pen.as_ref().map(|p| (&p.id, &p.points, p.closed, p.join)),
        expected.tool.pen.as_ref().map(|p| (&p.id, &p.points, p.closed, p.join)),
        "{fixture:?}/{cause:?}: accepted pen path"
    );
    assert!(!actual.tool.busy(), "{fixture:?}/{cause:?}: no finished drag remains");
}
fn matrix(fixture: Fixture) {
    for cause in CAUSES {
        let (mut h, from, to) = seed(fixture, cause);
        let mut expected = control(&h);
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        h.pointer = from;
        h.start();
        h.moved(to);
        assert!(
            h.ui.app.canvas_pointer.is_some(),
            "{fixture:?}/{cause:?}: exercise actual enrolled preview; phase={:?}, hit={:?}, tool={:?}",
            h.native.phase(),
            h.native.consumer.layout.as_ref().map(|layout| layout.classify(&h.ui.ctx, from)),
            h.ui.app.session.tool.tool
        );
        if matches!(fixture, Fixture::Pen | Fixture::VectorBend | Fixture::VectorHandle) {
            assert_ne!(*before, *h.ui.app.session.ds().unwrap().doc, "{fixture:?}/{cause:?}: actual geometry preview");
        } else if fixture == Fixture::CommentPin {
            assert!(h.ui.app.ui.comments.dragging());
        } else if fixture == Fixture::Prototype {
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc), "prototype preview is UI only");
        }
        interrupt(&mut h, &mut expected, cause);
        assert!(h.ui.app.canvas_pointer.is_none(), "{fixture:?}/{cause:?}: owner released");
        assert_engine(&h.ui.app.session, &expected, fixture, cause);
        if cause == Cause::Focus {
            h.run_checked(
                vec![touch(1, TouchPhase::End, h.pointer), button(h.pointer, false), Event::PointerGone],
                &[crate::touch_events::Issue::MissingStart, crate::touch_events::Issue::PhysicalInput],
            );
        } else if cause == Cause::Cancel {
            h.run(vec![button(h.pointer, false)]);
        } else {
            h.end();
        }
        if cause == Cause::Second {
            h.run(vec![touch(2, TouchPhase::End, to + vec2(60.0, 20.0)), Event::PointerGone]);
        }
        assert_engine(&h.ui.app.session, &expected, fixture, cause);
        assert!(!h.ui.app.ui.comments.dragging());
    }
}
#[test]
fn ipad_native_matrix_prototype_connector() {
    matrix(Fixture::Prototype);
}
#[test]
fn ipad_native_matrix_prior_pen_anchors() {
    matrix(Fixture::Pen);
}
#[test]
fn ipad_native_matrix_prior_text_and_preedit() {
    matrix(Fixture::Text);
}
#[test]
fn ipad_native_matrix_vector_bend() {
    matrix(Fixture::VectorBend);
}
#[test]
fn ipad_native_matrix_vector_bezier_handle() {
    matrix(Fixture::VectorHandle);
}
#[test]
fn ipad_native_matrix_comment_pin() {
    matrix(Fixture::CommentPin);
}
