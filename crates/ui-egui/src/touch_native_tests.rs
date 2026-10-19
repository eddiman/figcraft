//! End-to-end software values through the pinned producer boundary and host.
//! These tests do not qualify hardware or activate the native shell.
use super::*;
use crate::{layers_drag_tests::Ui, ordered_input::Boundary};
use egui::{Event, Modifiers, PointerButton, RawInput, TouchDeviceId, TouchId, TouchPhase, pos2, vec2};
use figcraft_engine::Session;
use serde_json::json;
use std::sync::Arc;

struct Host {
    ui: Ui,
    boundary: Boundary,
    finger: Finger,
    layout: Layout,
    generation: Option<u64>,
    time: f64,
    pointer: Pos2,
}
impl Host {
    fn new(tool: &str) -> Self {
        let mut session = Session::new();
        session.execute(tool, &json!({})).unwrap();
        let ui = Ui::new(session);
        let mut generation = Some(0);
        let layout = Layout::capture(&ui.app, &ui.ctx, None, &mut generation).unwrap();
        let pointer = ui.app.canvas.rect.center();
        Self { ui, boundary: Boundary::default(), finger: Finger::default(), layout, generation, time: 1.0, pointer }
    }
    fn run(&mut self, events: Vec<Event>) {
        self.run_checked(events, &[]);
    }
    fn run_checked(&mut self, events: Vec<Event>, expected: &[crate::touch_events::Issue]) {
        self.time += 0.05;
        let mut raw = RawInput { time: Some(self.time), events, ..Default::default() };
        let frame = self.boundary.process(
            &mut self.ui.app,
            &self.ui.ctx,
            &mut raw,
            Some(&self.layout.target),
            |p| self.layout.classify(&self.ui.ctx, p),
            &mut self.finger,
        );
        assert_eq!(frame.issues, expected);
        self.ui.frame(raw.events);
        if let Some(layout) = Layout::capture(&self.ui.app, &self.ui.ctx, Some(&self.layout), &mut self.generation) {
            self.layout = layout;
        }
    }
    fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
        Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: Some(0.6) }
    }
    fn button(pressed: bool, pos: Pos2) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
    }
    fn start(&mut self, pos: Pos2) {
        self.pointer = pos;
        self.run(vec![Self::touch(1, TouchPhase::Start, pos), Event::PointerMoved(pos), Self::button(true, pos)]);
    }
    fn moved(&mut self, pos: Pos2) {
        self.pointer = pos;
        self.run(vec![Self::touch(1, TouchPhase::Move, pos), Event::PointerMoved(pos)]);
    }
    fn end(&mut self, pos: Pos2) {
        self.run(vec![Self::touch(1, TouchPhase::End, pos), Self::button(false, self.pointer), Event::PointerGone]);
    }
}
#[test]
fn ipad_touch_mouse_event_not_duplicated() {
    let mut h = Host::new("tool.rectangle");
    let before = h.ui.app.session.ds().unwrap().history.undo.len();
    let nodes = h.ui.app.session.doc().unwrap().nodes().count();
    let p = h.pointer;
    h.start(p);
    assert!(h.ui.app.canvas_pointer.is_none());
    h.moved(p + vec2(50.0, 40.0));
    assert!(h.ui.app.canvas_pointer.is_some());
    h.end(p + vec2(50.0, 40.0));
    assert_eq!(h.finger.phase(), Phase::Idle);
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), nodes + 1);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), before + 1);
    assert!(!h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.run(vec![]);
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), nodes + 1);
}
#[test]
fn ipad_touch_host_second_finger_restores_document_before_anchored_navigation() {
    let mut h = Host::new("tool.rectangle");
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    let p = h.pointer;
    h.start(p);
    h.moved(p + vec2(40.0, 40.0));
    assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    let a = h.pointer;
    let b = a + vec2(40.0, 0.0);
    h.run(vec![Host::touch(2, TouchPhase::Start, b)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
    let baseline = h.ui.app.session.ds().unwrap().view.clone();
    let centre = a + (b - a) * 0.5;
    let anchor = h.ui.app.to_page(centre);
    let moved = b + vec2(50.0, 30.0);
    h.run(vec![Host::touch(2, TouchPhase::Move, moved)]);
    let current = a + (moved - a) * 0.5;
    let on_page = h.ui.app.to_page(current);
    assert!((anchor.x - on_page.x).abs() < 1e-5 && (anchor.y - on_page.y).abs() < 1e-5);
    assert_ne!(h.ui.app.session.ds().unwrap().view.zoom, baseline.zoom);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    h.run(vec![Host::touch(2, TouchPhase::End, moved)]);
    h.moved(p + vec2(100.0, 100.0));
    h.end(p + vec2(100.0, 100.0));
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    assert_eq!(h.finger.phase(), Phase::Idle);
}
#[test]
fn ipad_touch_host_long_press_opens_menu_once_without_an_edit() {
    let mut h = Host::new("tool.rectangle");
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.start(h.pointer);
    h.time += 0.5;
    h.run(vec![]);
    assert!(h.ui.app.ui.context_menu.is_some());
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.finger.phase(), Phase::SuppressedUntilAllReleased);
    h.end(h.pointer);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
}
#[test]
fn ipad_touch_host_comment_movement_discards_draft_but_tap_opens_one() {
    let mut h = Host::new("tool.comment");
    let p = h.pointer;
    h.start(p);
    h.moved(p + vec2(30.0, 0.0));
    assert!(h.ui.app.ui.comments.draft.is_none());
    h.end(h.pointer);
    assert!(h.ui.app.ui.comments.draft.is_none());
    h.start(p);
    h.end(p);
    assert!(h.ui.app.ui.comments.draft.is_some());
    assert!(h.ui.app.canvas_pointer.is_none());
}
#[test]
fn ipad_touch_host_native_cancel_restores_preview_and_late_end_cannot_commit() {
    let mut h = Host::new("tool.pencil");
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    let p = h.pointer;
    h.start(p);
    h.moved(p + vec2(30.0, 30.0));
    h.run(vec![Host::touch(1, TouchPhase::Cancel, h.pointer), Event::PointerGone]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.finger.phase(), Phase::Idle);
    h.start(p);
    h.moved(p + vec2(40.0, 40.0));
    h.end(h.pointer);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
}
#[test]
fn ipad_touch_host_pending_keyboard_interruption_cannot_later_draw() {
    let mut h = Host::new("tool.rectangle");
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let p = h.pointer;
    h.start(p);
    h.run(vec![Event::Text("ignored outside text".into())]);
    h.moved(p + vec2(70.0, 20.0));
    h.end(h.pointer);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert_eq!(h.finger.phase(), Phase::Idle);
}
#[test]
fn ipad_touch_host_panel_owner_stays_outside_canvas_when_crossing_it() {
    let mut h = Host::new("tool.rectangle");
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.start(pos2(1.0, 1.0));
    let p = h.ui.app.canvas.rect.center();
    h.moved(p);
    h.end(p);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.finger.phase(), Phase::Idle);
}

#[test]
fn ipad_touch_host_focus_reset_discards_lost_contacts_and_accepts_a_new_start() {
    let mut h = Host::new("tool.rectangle");
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    let p = h.pointer;
    h.start(p);
    h.moved(p + vec2(30.0, 30.0));
    h.run(vec![Event::WindowFocused(false)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.finger.phase(), Phase::Idle);
    h.run(vec![Event::WindowFocused(true)]);
    h.start(p);
    h.moved(p + vec2(40.0, 40.0));
    h.end(h.pointer);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
    assert_eq!(h.finger.phase(), Phase::Idle);
}

#[test]
fn ipad_touch_host_tool_cancellation_matrix_restores_source_and_rejects_late_up() {
    // Exercise actual tool previews through the recognizer, rather than injecting
    // private drag state. Specialized handle/vector/text fixtures extend this
    // matrix separately; this test covers draws, marquee and the Hand tool.
    #[derive(Clone, Copy)]
    enum Cause {
        Second,
        Cancel,
        Focus,
        Viewport,
        Modal,
        Switch,
        Close,
    }
    use Cause::*;
    for tool in ["rectangle", "ellipse", "frame", "section", "slice", "line", "arrow", "polygon", "star", "pencil", "pen", "text", "move", "hand"] {
        for cause in [Second, Cancel, Focus, Viewport, Modal, Switch, Close] {
            let interruption = match cause {
                Second => "second",
                Cancel => "cancel",
                Focus => "focus",
                Viewport => "viewport",
                Modal => "modal",
                Switch => "switch",
                Close => "close",
            };
            let mut h = Host::new(&format!("tool.{tool}"));
            if interruption == "switch" {
                h.ui.app.run("file.new", json!({})).unwrap();
                h.ui.app.run("doc.switch", json!({"index":0})).unwrap();
                h.ui.frame(vec![]);
                h.layout = Layout::capture(&h.ui.app, &h.ui.ctx, Some(&h.layout), &mut h.generation).unwrap();
            }
            let source = h.ui.app.session.ds().unwrap();
            let uid = source.uid;
            let before = source.doc.clone();
            let selection = source.selection.clone();
            let undo = source.history.undo.len();
            let redo = source.history.redo.len();
            let view = (source.view.cx, source.view.cy, source.view.zoom);
            let p = h.pointer;
            h.start(p);
            h.moved(p + vec2(40.0, 30.0));
            assert!(h.ui.app.canvas_pointer.is_some(), "{tool}/{interruption} must exercise an enrolled preview");
            match cause {
                Second => h.run(vec![Host::touch(2, TouchPhase::Start, h.pointer + vec2(50.0, 0.0))]),
                Cancel => h.run(vec![Host::touch(1, TouchPhase::Cancel, h.pointer), Event::PointerGone]),
                Focus => h.run(vec![Event::WindowFocused(false)]),
                Viewport => {
                    h.ui.app.canvas.rect.max.x -= 20.0;
                    h.run(vec![]);
                }
                Modal => {
                    h.ui.app.run("help.about", json!({})).unwrap();
                    h.run(vec![]);
                }
                Switch => {
                    h.ui.app.run("doc.switch", json!({"index":1})).unwrap();
                    h.run(vec![]);
                }
                Close => {
                    h.ui.app.run("file.close", json!({"force":true})).unwrap();
                    h.run(vec![]);
                }
            }
            assert!(h.ui.app.canvas_pointer.is_none(), "{tool}/{interruption}");
            if let Some(source) = h.ui.app.session.docs.iter().find(|d| d.uid == uid) {
                assert!(Arc::ptr_eq(&before, &source.doc), "{tool}/{interruption}: preview removed");
                assert_eq!(source.selection, selection, "{tool}/{interruption}: selection restored");
                assert_eq!((source.history.undo.len(), source.history.redo.len()), (undo, redo), "{tool}/{interruption}");
                assert_eq!((source.view.cx, source.view.cy, source.view.zoom), view, "{tool}/{interruption}: no navigation move yet");
            } else {
                assert_eq!(interruption, "close");
            }
            let destinations: Vec<_> = h.ui.app.session.docs.iter().map(|d| (d.uid, d.doc.clone(), d.history.undo.len())).collect();
            if interruption == "focus" {
                // T34 deliberately diagnoses a terminal value whose Start was
                // discarded by focus reset. Verify that it remains suppressed.
                h.run_checked(
                    vec![Host::touch(1, TouchPhase::End, h.pointer), Host::button(false, h.pointer), Event::PointerGone],
                    &[crate::touch_events::Issue::MissingStart],
                );
            } else if interruption != "cancel" {
                h.end(h.pointer);
            }
            for (uid, doc, depth) in destinations {
                let source = h.ui.app.session.docs.iter().find(|d| d.uid == uid).unwrap();
                assert!(Arc::ptr_eq(&doc, &source.doc), "{tool}/{interruption}: late Up edited a document");
                assert_eq!(source.history.undo.len(), depth);
            }
        }
    }
}

#[test]
fn ipad_touch_host_comment_pin_remains_draggable_under_commit_only_tool_policy() {
    for cancel in [false, true] {
        let mut h = Host::new("tool.comment");
        h.ui.app.run("comment.add", json!({"x":0,"y":0,"text":"Synthetic pin"})).unwrap();
        h.ui.frame(vec![]);
        h.layout = Layout::capture(&h.ui.app, &h.ui.ctx, Some(&h.layout), &mut h.generation).unwrap();
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        let undo = h.ui.app.session.ds().unwrap().history.undo.len();
        let p = h.ui.app.to_screen(0.0, 0.0) + vec2(14.0, -14.0);
        assert!(crate::comments::pointer_hit(&h.ui.app, p));
        h.start(p);
        h.moved(p + vec2(50.0, 40.0));
        assert!(h.ui.app.ui.comments.dragging());
        assert!(h.ui.app.canvas_pointer.is_some());
        h.run(vec![]);
        assert!(h.ui.app.ui.comments.dragging(), "a frame without egui primary_down cannot release the pin");
        if cancel {
            h.run(vec![Host::touch(2, TouchPhase::Start, h.pointer + vec2(50.0, 0.0))]);
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
            assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
            assert!(!h.ui.app.ui.comments.dragging());
            assert!(h.ui.app.ui.comments.open.is_none());
        } else {
            h.end(h.pointer);
            assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
            assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
            assert!(!h.ui.app.ui.comments.dragging());
        }
        assert!(h.ui.app.ui.comments.draft.is_none());
    }
}
