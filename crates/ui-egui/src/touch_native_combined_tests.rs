use super::*;
use crate::layers_drag_tests::Ui;
use egui::{Event, Modifiers, PointerButton, RawInput, TouchDeviceId, TouchId, TouchPhase, pos2, vec2};
use figcraft_engine::Session;
use serde_json::json;
use std::sync::Arc;
pub(super) struct Host {
    pub(super) ui: Ui,
    pub(super) native: Native,
    pub(super) time: f64,
    pub(super) pointer: Pos2,
    show_field: bool,
    field: String,
    field_rect: Option<egui::Rect>,
    cancelled_button_rect: Option<egui::Rect>,
    clicked: bool,
}
impl Host {
    pub(super) fn new() -> Self {
        let mut session = Session::new();
        session.execute("tool.rectangle", &json!({})).unwrap();
        let ui = Ui::new(session);
        let mut native = Native::default();
        native.finish_frame(&ui.app, &ui.ctx, true);
        let pointer = ui.app.canvas.rect.center();
        Self {
            ui,
            native,
            time: 1.0,
            pointer,
            show_field: false,
            field: String::new(),
            field_rect: None,
            cancelled_button_rect: None,
            clicked: false,
        }
    }
    pub(super) fn run(&mut self, events: Vec<Event>) {
        self.run_checked(events, &[]);
    }
    pub(super) fn run_checked(&mut self, events: Vec<Event>, expected: &[crate::touch_events::Issue]) {
        self.run_custom(events, expected, |_| {});
    }
    pub(super) fn run_custom(&mut self, events: Vec<Event>, expected: &[crate::touch_events::Issue], configure: impl FnOnce(&mut RawInput)) {
        self.time += 0.05;
        let mut raw = RawInput {
            events,
            time: Some(self.time),
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))),
            ..Default::default()
        };
        configure(&mut raw);
        let frame = self.native.process(&mut self.ui.app, &self.ui.ctx, &mut raw);
        assert_eq!(frame.issues, expected);
        let app = &mut self.ui.app;
        self.ui
            .ctx
            .run_ui(raw, |ui| {
                if self.show_field {
                    ui.horizontal(|ui| {
                        self.field_rect = Some(ui.text_edit_singleline(&mut self.field).rect);
                        let response = ui.button("Synthetic cancel target");
                        self.cancelled_button_rect = Some(response.rect);
                        self.clicked |= response.clicked();
                    });
                }
                app.ui(ui);
            })
            .drop_without_applying_deltas();
        self.native.finish_frame(&self.ui.app, &self.ui.ctx, true);
    }
    pub(super) fn start(&mut self) {
        let p = self.pointer;
        self.run(vec![touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(p, true)]);
    }
    pub(super) fn moved(&mut self, p: Pos2) {
        self.pointer = p;
        self.run(vec![touch(1, TouchPhase::Move, p), Event::PointerMoved(p)]);
    }
    pub(super) fn end(&mut self) {
        let p = self.pointer;
        self.run(vec![touch(1, TouchPhase::End, p), button(p, false), Event::PointerGone]);
    }
}
fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: None }
}
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}
#[test]
fn ipad_native_combined_mouse_drag_keeps_scope_across_egui_frames() {
    let mut h = Host::new();
    let p = h.pointer;
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    let count = h.ui.app.session.doc().unwrap().nodes().count();
    h.run(vec![Event::PointerMoved(p), button(p, true)]);
    assert!(h.ui.app.canvas_pointer.is_some());
    h.run(vec![Event::PointerMoved(p + vec2(60.0, 40.0))]);
    assert!(h.ui.app.canvas_pointer.is_some());
    h.run(vec![button(p + vec2(60.0, 40.0), false)]);
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), count + 1);
}
#[test]
fn ipad_native_combined_touch_tap_then_mouse_click_has_no_legacy_duplicate() {
    let mut h = Host::new();
    h.ui.app.session.prefs.keep_tool = true;
    let p = h.pointer;
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    let count = h.ui.app.session.doc().unwrap().nodes().count();
    h.run(vec![
        touch(1, TouchPhase::Start, p),
        Event::PointerMoved(p),
        button(p, true),
        touch(1, TouchPhase::End, p),
        button(p, false),
        Event::PointerGone,
        Event::PointerMoved(p),
        button(p, true),
        Event::PointerMoved(p + vec2(40.0, 40.0)),
        button(p + vec2(40.0, 40.0), false),
    ]);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 2);
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), count + 2);
    assert!(h.ui.app.canvas_pointer.is_none());
}
#[test]
fn ipad_native_combined_cancelled_finger_end_cannot_cancel_a_new_mouse_scope() {
    let mut h = Host::new();
    let p = h.pointer;
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.start();
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(50.0, 40.0))]);
    assert!(h.ui.app.canvas_pointer.is_some());
    h.pointer = p + vec2(50.0, 40.0);
    h.end();
    assert!(h.ui.app.canvas_pointer.is_some(), "old suppressed finger Up cannot cancel independent mouse");
    h.run(vec![button(h.pointer, false)]);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
}
#[test]
fn ipad_native_combined_panel_touch_cancel_rolls_back_mouse_without_releasing_its_button() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(40.0, 30.0))]);
    let panel = pos2(1.0, 1.0);
    h.run(vec![touch(1, TouchPhase::Start, panel), Event::PointerMoved(panel), button(panel, true)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    h.run(vec![touch(1, TouchPhase::Cancel, panel), Event::PointerGone]);
    assert!(h.ui.ctx.input(|i| i.pointer.primary_down()), "panel cancellation cannot release independently held mouse");
    h.run(vec![Event::PointerMoved(p + vec2(80.0, 60.0)), button(p + vec2(80.0, 60.0), false)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}
#[test]
fn ipad_native_combined_finger_pinch_cancels_draw_and_only_changes_view() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.start();
    h.moved(p + vec2(50.0, 40.0));
    h.run(vec![touch(2, TouchPhase::Start, p + vec2(100.0, 40.0))]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    let zoom = h.ui.app.session.ds().unwrap().view.zoom;
    h.run(vec![touch(2, TouchPhase::Move, p + vec2(160.0, 60.0))]);
    assert!(h.ui.app.session.ds().unwrap().view.zoom > zoom);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    h.run(vec![touch(2, TouchPhase::End, p + vec2(160.0, 60.0))]);
    h.moved(p + vec2(180.0, 80.0));
    h.end();
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}
#[test]
fn ipad_native_combined_pinch_release_either_contact_then_fresh_draw() {
    for primary_first in [false, true] {
        let mut h = Host::new();
        let p = h.pointer;
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        let undo = h.ui.app.session.ds().unwrap().history.undo.len();
        let count = before.nodes().count();
        h.start();
        h.moved(p + vec2(50.0, 40.0));
        let second = p + vec2(100.0, 40.0);
        h.run(vec![touch(2, TouchPhase::Start, second)]);
        let second = second + vec2(60.0, 20.0);
        h.run(vec![touch(2, TouchPhase::Move, second)]);
        assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        assert!(!h.ui.app.session.tool.busy());
        if primary_first {
            h.end();
            // The pinned producer resumes pointer emulation for the remaining
            // contact. Native must still suppress editing until that contact ends.
            let second = second + vec2(40.0, 30.0);
            h.run(vec![touch(2, TouchPhase::Move, second), Event::PointerMoved(second)]);
            h.run(vec![touch(2, TouchPhase::End, second), button(second, false), Event::PointerGone]);
        } else {
            h.run(vec![touch(2, TouchPhase::End, second)]);
            h.moved(p + vec2(180.0, 80.0));
            h.end();
        }
        assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
        assert!(h.ui.app.canvas_pointer.is_none());
        assert!(!h.ui.app.session.tool.busy());
        h.pointer = h.ui.app.canvas.rect.center();
        h.start();
        h.moved(h.pointer + vec2(60.0, 40.0));
        assert!(h.ui.app.canvas_pointer.is_some(), "fresh contact must enroll after both navigation contacts release");
        h.end();
        assert!(h.ui.app.canvas_pointer.is_none());
        assert!(!h.ui.app.session.tool.busy());
        assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), count + 1);
        assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
    }
}
#[test]
fn ipad_native_combined_panel_pinch_does_not_zoom_the_canvas() {
    let mut h = Host::new();
    let p = h.pointer;
    h.run(vec![Event::PointerMoved(p)]);
    let view = h.ui.app.session.ds().unwrap().view.clone();
    let panel = pos2(80.0, 150.0);
    h.run(vec![
        touch(1, TouchPhase::Start, panel),
        Event::PointerMoved(panel),
        button(panel, true),
        touch(2, TouchPhase::Start, panel + vec2(20.0, 0.0)),
    ]);
    h.run(vec![touch(2, TouchPhase::Move, panel + vec2(200.0, 0.0))]);
    let next = &h.ui.app.session.ds().unwrap().view;
    assert_eq!((next.cx, next.cy, next.zoom), (view.cx, view.cy, view.zoom));
}

#[test]
fn ipad_native_combined_pointer_enters_from_panel_and_clicks_in_same_batch() {
    let mut h = Host::new();
    let p = h.pointer;
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.run(vec![
        Event::PointerMoved(pos2(1.0, 1.0)),
        Event::PointerMoved(p),
        button(p, true),
        Event::PointerMoved(p + vec2(40.0, 40.0)),
        button(p + vec2(40.0, 40.0), false),
    ]);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
}

#[test]
fn ipad_native_combined_secondary_panel_contact_accepts_text_while_canvas_is_held() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.start();
    let panel = h.field_rect.unwrap().center();
    // egui-winit emits no pointer suffix for this non-primary contact.
    h.run(vec![touch(2, TouchPhase::Start, panel)]);
    h.run(vec![touch(2, TouchPhase::End, panel)]);
    h.run(vec![Event::Text("Panel".into())]);
    assert_eq!(h.field, "Panel");
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
    h.end();
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}
#[test]
fn ipad_native_combined_panel_touch_field_accepts_text_exactly_once() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let p = h.field_rect.unwrap().center();
    h.run(vec![touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(p, true)]);
    h.run(vec![touch(1, TouchPhase::End, p), button(p, false), Event::PointerGone]);
    h.run(vec![Event::Text("T16".into())]);
    assert_eq!(h.field, "T16");
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
}
#[test]
fn ipad_native_combined_held_mouse_panel_cancel_cannot_accept_button_on_later_mouse_up() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    let p = h.ui.app.canvas.rect.center();
    h.run(vec![Event::PointerMoved(p), button(p, true)]);
    let panel = h.cancelled_button_rect.unwrap().center();
    h.run(vec![touch(1, TouchPhase::Start, panel), Event::PointerMoved(panel), button(panel, true)]);
    h.run(vec![touch(1, TouchPhase::Cancel, panel), Event::PointerGone]);
    assert!(!h.clicked);
    h.run(vec![button(panel, false)]);
    assert!(!h.clicked, "cancelled panel touch cannot become a button click on an independent mouse release");
}
#[test]
fn ipad_native_combined_physical_zoom_is_applied_once_while_panel_touch_is_live() {
    let mut h = Host::new();
    let p = h.pointer;
    h.run(vec![Event::PointerMoved(p)]);
    let panel = pos2(80.0, 150.0);
    h.run(vec![
        touch(1, TouchPhase::Start, panel),
        Event::PointerMoved(panel),
        button(panel, true),
        touch(2, TouchPhase::Start, panel + vec2(20.0, 0.0)),
    ]);
    let zoom = h.ui.app.session.ds().unwrap().view.zoom;
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.run(vec![Event::Zoom(1.1), touch(2, TouchPhase::Move, panel + vec2(200.0, 0.0))]);
    let ds = h.ui.app.session.ds().unwrap();
    assert!((ds.view.zoom - zoom * 1.1_f32 as f64).abs() < 1e-6);
    assert_eq!(ds.history.undo.len(), undo);
}

#[test]
fn ipad_native_combined_keyboard_between_mouse_and_touch_keeps_prior_typing_once() {
    let mut h = Host::new();
    h.ui.app.run("tool.move", json!({})).unwrap();
    let id = figcraft_doc::NodeId(
        h.ui.app.run("text.create", json!({"text":"Before","x":0,"y":0,"width":260,"props":{"fontSize":24}})).unwrap()["id"].as_u64().unwrap(),
    );
    figcraft_engine::textedit::begin(&mut h.ui.app.session, id, Some(6), false).unwrap();
    h.run(vec![Event::Text("A".into())]);
    let p = h.ui.app.to_screen(20.0, 12.0);
    h.run(vec![
        button(p, true),
        Event::PointerMoved(p + vec2(30.0, 0.0)),
        Event::Text("B".into()),
        touch(1, TouchPhase::Start, p),
        Event::PointerMoved(p),
        button(p, true),
        touch(1, TouchPhase::End, p),
        button(p, false),
        Event::PointerGone,
    ]);
    let figcraft_doc::NodeKind::Text(text) = &h.ui.app.session.doc().unwrap().node(id).unwrap().kind else { panic!("text") };
    assert_eq!(text.characters, "BeforeAB");
    assert!(h.ui.app.canvas_pointer.is_none());
    assert!(!h.ui.ctx.input(|i| i.events.iter().any(|e| matches!(e, Event::Text(_)))));
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.run(vec![button(p, false)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}
#[test]
fn ipad_native_combined_mode_keys_and_middle_pan_use_physical_values() {
    for mode in [0, 1, 2] {
        let mut h = Host::new();
        let p = h.pointer;
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        let view = h.ui.app.session.ds().unwrap().view.clone();
        let undo = h.ui.app.session.ds().unwrap().history.undo.len();
        let key = if mode == 0 { egui::Key::Space } else { egui::Key::Z };
        let b = if mode == 2 { PointerButton::Middle } else { PointerButton::Primary };
        let button = |pos, pressed| Event::PointerButton { pos, button: b, pressed, modifiers: Modifiers::NONE };
        let mut events = vec![];
        if mode != 2 {
            events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        }
        events.push(button(p, true));
        h.run(events);
        h.run(vec![Event::PointerMoved(p + vec2(60.0, 40.0))]);
        h.run(vec![button(p + vec2(60.0, 40.0), false)]);
        let ds = h.ui.app.session.ds().unwrap();
        if mode == 1 {
            assert!(ds.view.zoom > view.zoom);
        } else {
            assert!((ds.view.cx - (view.cx - 60.0 / view.zoom)).abs() < 1e-6);
        }
        assert!(Arc::ptr_eq(&before, &ds.doc));
        assert_eq!(ds.history.undo.len(), undo);
        assert!(h.ui.app.canvas_pointer.is_none());
    }
}
#[test]
fn ipad_native_combined_nonfinite_zoom_blocks_canvas_until_focus_reset() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.run_checked(
        vec![Event::Zoom(f32::NAN), touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(p, true)],
        &[crate::touch_events::Issue::PhysicalInput],
    );
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_input_blocked);
    assert!(!h.ui.ctx.input(|i| i.events.iter().any(|e| matches!(e,Event::Zoom(z) if !z.is_finite()))));
    h.run_checked(vec![], &[crate::touch_events::Issue::PhysicalInput]);
    h.run(vec![Event::WindowFocused(false)]);
    h.run(vec![Event::WindowFocused(true)]);
    h.start();
    h.moved(p + vec2(40.0, 40.0));
    h.end();
    assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(!h.ui.app.canvas_input_blocked);
}

#[test]
fn ipad_native_combined_invalid_prediction_and_geometry_never_enter_egui() {
    for bad in [0, 1, 2, 3] {
        let mut h = Host::new();
        let mut raw = RawInput { time: Some(2.0), events: vec![Event::Text("Synthetic panel text".into())], ..Default::default() };
        match bad {
            0 => raw.predicted_dt = f32::NAN,
            1 => raw.screen_rect = Some(egui::Rect::from_min_max(pos2(f32::NAN, 0.0), pos2(1440.0, 900.0))),
            2 => raw.viewports.entry(raw.viewport_id).or_default().native_pixels_per_point = Some(f32::NAN),
            _ => {
                let mut insets = egui::SafeAreaInsets::default();
                insets.0.top = f32::NAN;
                raw.safe_area_insets = Some(insets);
            }
        }
        let frame = h.native.process(&mut h.ui.app, &h.ui.ctx, &mut raw);
        assert!(frame.blocked && h.ui.app.canvas_input_blocked);
        assert!(frame.issues.contains(&crate::touch_events::Issue::PhysicalInput));
        assert!(raw.predicted_dt.is_finite());
        assert!(raw.screen_rect.is_none_or(|r| r.is_finite()));
        assert!(raw.safe_area_insets.is_none_or(|a| a.0.top.is_finite()));
        assert!(raw.viewports.get(&raw.viewport_id).and_then(|v| v.native_pixels_per_point).is_none_or(f32::is_finite));
        assert!(raw.events.iter().any(|e| matches!(e,Event::Text(t) if t=="Synthetic panel text")));
        let app = &mut h.ui.app;
        h.ui.ctx.run_ui(raw, |ui| app.ui(ui)).drop_without_applying_deltas();
    }
}

#[test]
fn ipad_native_combined_missing_canvas_map_keeps_panel_touch_operational() {
    let mut h = Host::new();
    h.show_field = true;
    h.ui.app.ui.home = true;
    h.run(vec![]);
    h.run(vec![]);
    assert!(h.native.consumer.layout.is_none());
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let panel = h.field_rect.unwrap().center();
    h.run(vec![touch(1, TouchPhase::Start, panel), Event::PointerMoved(panel), button(panel, true)]);
    h.run(vec![touch(1, TouchPhase::End, panel), button(panel, false), Event::PointerGone]);
    h.run(vec![Event::Text("Panel".into())]);
    assert_eq!(h.field, "Panel", "home or modal state must retain panel interaction without a canvas map");
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
}

#[test]
fn ipad_native_combined_missing_map_cannot_start_canvas_edit_or_transfer_a_held_contact() {
    let mut h = Host::new();
    h.native = Native::default();
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let p = h.pointer;
    h.start();
    h.moved(p + vec2(40.0, 40.0));
    h.end();
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
    h.start();
    h.moved(p + vec2(80.0, 80.0));
    h.end();
    assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}

impl Host {
    pub(super) fn unavailable(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let mut raw = RawInput {
            events,
            time: Some(self.time),
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))),
            ..Default::default()
        };
        let frame = self.native.unavailable(&self.ui.ctx, &mut raw);
        assert!(frame.issues.is_empty(), "{:?}", frame.issues);
        self.ui.ctx.run_ui(raw, |_| {}).drop_without_applying_deltas();
    }
}
#[test]
fn ipad_native_unavailable_finger_end_queues_rollback_before_editor_resumes() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.start();
    h.moved(p + vec2(50.0, 40.0));
    assert!(h.ui.app.canvas_pointer.is_some());
    assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    h.unavailable(vec![touch(1, TouchPhase::End, h.pointer), button(h.pointer, false), Event::PointerGone]);
    assert!(h.ui.app.canvas_pointer.is_some(), "unavailable callback cannot borrow the app to restore immediately");
    assert!(h.native.consumer.layout.is_none());
    h.run(vec![]);
    assert!(h.ui.app.canvas_pointer.is_none());
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    h.start();
    h.moved(p + vec2(90.0, 70.0));
    h.end();
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
}
#[test]
fn ipad_native_unavailable_physical_up_does_not_leave_a_false_held_mouse() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(50.0, 40.0))]);
    h.unavailable(vec![button(p + vec2(50.0, 40.0), false)]);
    assert!(!h.native.consumer.mouse.any_down());
    h.run(vec![]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    h.start();
    h.moved(p + vec2(70.0, 70.0));
    h.end();
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
}
#[test]
fn ipad_native_unavailable_still_held_mouse_cannot_restart_its_edit() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(40.0, 30.0))]);
    h.unavailable(vec![Event::PointerMoved(p + vec2(70.0, 60.0))]);
    assert!(h.native.consumer.mouse.any_down());
    h.run(vec![]);
    h.run(vec![Event::PointerMoved(p + vec2(100.0, 90.0)), button(p + vec2(100.0, 90.0), false)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
}
#[test]
fn ipad_native_unavailable_first_hook_cannot_fall_back_to_legacy_canvas() {
    let mut h = Host::new();
    h.native = Native::default();
    Native::enable(&mut h.ui.app);
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let mut raw = RawInput {
        time: Some(2.0),
        events: vec![
            touch(1, TouchPhase::Start, p),
            Event::PointerMoved(p),
            button(p, true),
            touch(1, TouchPhase::End, p),
            button(p, false),
            Event::PointerGone,
        ],
        ..Default::default()
    };
    let frame = h.native.unavailable(&h.ui.ctx, &mut raw);
    assert!(frame.issues.is_empty());
    // The temporary borrow can end before ui() in this same pass. Even a raw
    // panel-owned touch over the canvas must not reach the legacy editor route.
    let app = &mut h.ui.app;
    h.ui.ctx.run_ui(raw, |ui| app.ui(ui)).drop_without_applying_deltas();
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
}
#[test]
fn ipad_native_unavailable_actual_nested_app_borrow_takes_the_guard_path() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.start();
    h.moved(p + vec2(40.0, 30.0));
    let ctx = h.ui.ctx.clone();
    let mut native = h.native;
    crate::windows::install(h.ui.app);
    let mut raw = RawInput {
        time: Some(3.0),
        events: vec![touch(1, TouchPhase::End, p + vec2(40.0, 30.0)), button(p + vec2(40.0, 30.0), false), Event::PointerGone],
        ..Default::default()
    };
    crate::windows::with_app(|app| {
        assert!(crate::windows::with_app(|_| ()).is_none(), "nested borrow is unavailable");
        let frame = native.unavailable(&ctx, &mut raw);
        assert!(frame.issues.is_empty());
        assert!(app.canvas_pointer.is_some());
    })
    .unwrap();
    crate::windows::with_app(|app| {
        ctx.run_ui(raw, |ui| app.ui(ui)).drop_without_applying_deltas();
        assert!(app.canvas_pointer.is_none());
        assert!(Arc::ptr_eq(&before, &app.session.ds().unwrap().doc));
    })
    .unwrap();
}
#[test]
fn ipad_native_unavailable_bad_input_is_explicit_and_keeps_panel_text() {
    let mut h = Host::new();
    let mut raw = RawInput { time: Some(2.0), events: vec![Event::Zoom(f32::NAN), Event::Text("Synthetic panel text".into())], ..Default::default() };
    let frame = h.native.unavailable(&h.ui.ctx, &mut raw);
    assert!(frame.blocked && frame.issues.contains(&crate::touch_events::Issue::PhysicalInput));
    assert!(raw.events.iter().any(|event| matches!(event,Event::Text(text) if text=="Synthetic panel text")));
    assert!(!raw.events.iter().any(|event| matches!(event,Event::Zoom(value) if !value.is_finite())));
    h.time = 2.0;
    h.run_checked(vec![], &[crate::touch_events::Issue::PhysicalInput]);
    h.unavailable(vec![Event::WindowFocused(false)]);
    h.run(vec![Event::WindowFocused(true)]);
    assert!(!h.ui.app.canvas_input_blocked);
}

#[test]
fn ipad_native_queued_lifecycle_cancel_suppresses_pending_contact_before_down() {
    let mut h = Host::new();
    let p = h.pointer;
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.start();
    assert!(h.ui.app.canvas_pointer.is_none());
    crate::FigApp::request_canvas_pointer_cancel(&h.ui.ctx);
    h.moved(p + vec2(50.0, 40.0));
    h.end();
    assert!(
        Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc),
        "queued lifecycle cancellation must invalidate pending intent before the first Down"
    );
    assert!(h.ui.app.canvas_pointer.is_none());
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    h.start();
    h.moved(p + vec2(90.0, 80.0));
    h.end();
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
}

#[test]
fn ipad_native_combined_secondary_panel_cancel_does_not_click_or_release_canvas_into_edit() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.start();
    h.moved(h.pointer + vec2(40.0, 30.0));
    let panel = h.cancelled_button_rect.unwrap().center();
    h.run(vec![touch(2, TouchPhase::Start, panel)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.run(vec![touch(2, TouchPhase::Cancel, panel)]);
    assert!(!h.clicked);
    assert!(!h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.end();
    assert!(!h.clicked);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}

#[test]
fn ipad_native_combined_canvas_primary_end_keeps_secondary_panel_pointer_down() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.start();
    let panel = h.field_rect.unwrap().center();
    h.run(vec![touch(2, TouchPhase::Start, panel)]);
    h.end();
    assert!(h.ui.ctx.input(|i| i.pointer.primary_down()));
    // The backend has cleared its primary/cursor; this End has only Gone.
    h.run(vec![touch(2, TouchPhase::End, panel), Event::PointerGone]);
    assert!(!h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.run(vec![Event::Text("Panel".into())]);
    assert_eq!(h.field, "Panel");
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
}

#[test]
fn ipad_native_combined_physical_takeover_cancels_secondary_panel_click() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    h.start();
    let panel = h.cancelled_button_rect.unwrap().center();
    h.run(vec![touch(2, TouchPhase::Start, panel)]);
    h.run(vec![Event::PointerMoved(panel), button(panel, true)]);
    assert!(!h.clicked);
    h.run(vec![touch(2, TouchPhase::End, panel)]);
    assert!(h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.run(vec![button(panel, false)]);
    // The independent mouse can perform its own click.
    assert!(h.clicked);
    // egui-winit uses the last backend cursor, including physical motion.
    h.run(vec![touch(1, TouchPhase::End, h.pointer), button(panel, false), Event::PointerGone]);
}

#[test]
fn ipad_native_combined_unsupported_physical_metadata_releases_panel_pointer() {
    for unavailable in [false, true] {
        let mut h = Host::new();
        h.show_field = true;
        h.run(vec![]);
        h.run(vec![]);
        let p = h.cancelled_button_rect.unwrap().center();
        h.run(vec![touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(p, true)]);
        assert!(h.ui.ctx.input(|i| i.pointer.primary_down()));
        if unavailable {
            h.time += 0.05;
            let mut raw = RawInput { time: Some(h.time), ..Default::default() };
            raw.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(f32::NAN);
            let frame = h.native.unavailable(&h.ui.ctx, &mut raw);
            assert!(frame.blocked);
            h.ui.ctx.run_ui(raw, |_| {}).drop_without_applying_deltas();
        } else {
            h.run_custom(vec![], &[crate::touch_events::Issue::PhysicalInput], |raw| {
                raw.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(f32::NAN);
            });
        }
        assert!(!h.ui.ctx.input(|i| i.pointer.primary_down()));
        assert!(!h.clicked);
    }
}

struct PanicRecoveryProbe {
    expected: Arc<figcraft_doc::Document>,
    saves: std::rc::Rc<std::cell::Cell<usize>>,
}
impl crate::Services for PanicRecoveryProbe {
    fn emergency_save(&mut self, session: &Session) {
        self.saves.set(self.saves.get() + 1);
        assert!(Arc::ptr_eq(&self.expected, &session.ds().unwrap().doc), "recovery must capture the pre-Down document");
        assert!(session.ds().unwrap().interaction.is_none(), "recovery must not capture the provisional interaction");
    }
}

#[test]
fn ipad_native_ui_panic_restores_preview_before_emergency_recovery_capture() {
    for physical in [false, true] {
        let mut h = Host::new();
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        let undo = h.ui.app.session.ds().unwrap().history.undo.len();
        let saves = std::rc::Rc::new(std::cell::Cell::new(0));
        h.ui.app.services = Box::new(PanicRecoveryProbe { expected: before.clone(), saves: saves.clone() });
        let p = h.pointer;
        if physical {
            h.run(vec![Event::PointerMoved(p), button(p, true), Event::PointerMoved(p + vec2(50.0, 40.0))]);
        } else {
            h.start();
            h.moved(p + vec2(50.0, 40.0));
        }
        assert!(h.ui.app.canvas_pointer.is_some());
        assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        crate::windows::guard(&mut h.ui.app, &h.ui.ctx, |_| panic!("synthetic UI panic after Native preview"));
        assert_eq!(saves.get(), 1);
        assert!(h.ui.app.canvas_pointer.is_none());
        if physical {
            h.run(vec![button(p + vec2(50.0, 40.0), false)]);
        } else {
            h.end();
        }
        assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    }
}

#[test]
fn ipad_native_ui_panic_interrupts_pending_contact_without_engine_token() {
    let mut h = Host::new();
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.start();
    assert_eq!(h.native.phase(), Phase::PendingFinger);
    assert!(h.ui.app.canvas_pointer.is_none());
    crate::windows::guard(&mut h.ui.app, &h.ui.ctx, |_| panic!("synthetic UI panic while Native intent is pending"));
    h.moved(h.pointer + vec2(50.0, 40.0));
    h.end();
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
}

fn guarded_native_frame(h: &mut Host, fail_after_drawing: bool) -> bool {
    h.time += 0.05;
    let mut raw =
        RawInput { time: Some(h.time), screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))), ..Default::default() };
    assert!(h.native.process(&mut h.ui.app, &h.ui.ctx, &mut raw).issues.is_empty());
    let ctx = h.ui.ctx.clone();
    let mut completed = false;
    h.ui.ctx
        .run_ui(raw, |ui| {
            completed = crate::windows::guard(&mut h.ui.app, &ctx, |app| {
                app.ui(ui);
                assert!(!fail_after_drawing, "synthetic UI panic after canvas drawing");
            });
        })
        .drop_without_applying_deltas();
    h.native.finish_frame(&h.ui.app, &h.ui.ctx, completed);
    completed
}

#[test]
fn ipad_native_aborted_ui_frame_discards_partial_layout_and_requires_new_canvas_start() {
    let mut h = Host::new();
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let count = before.nodes().count();
    let old_owner = h.native.consumer.layout.as_ref().unwrap().target.owner();
    assert!(!guarded_native_frame(&mut h, true));
    assert!(h.native.consumer.layout.is_none());
    // This Start occurs before a new completed layout, so it cannot edit even
    // after drawing resumes and publishes a different layout generation.
    h.start();
    assert_ne!(h.native.consumer.layout.as_ref().unwrap().target.owner(), old_owner);
    h.moved(h.pointer + vec2(50.0, 40.0));
    h.end();
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(guarded_native_frame(&mut h, false));
    assert!(h.native.consumer.layout.is_some());
    h.start();
    h.moved(h.pointer + vec2(50.0, 40.0));
    h.end();
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), count + 1);
}

#[test]
fn ipad_native_aborted_ui_frame_keeps_held_physical_mouse_suppressed_until_release() {
    let mut h = Host::new();
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let p = h.pointer;
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(50.0, 40.0))]);
    assert!(h.ui.app.canvas_pointer.is_some());
    assert!(!guarded_native_frame(&mut h, true));
    assert!(h.ui.app.canvas_pointer.is_none());
    assert!(h.native.consumer.mouse.any_down());
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(guarded_native_frame(&mut h, false));
    h.run(vec![Event::PointerMoved(p + vec2(90.0, 70.0))]);
    assert!(h.ui.app.canvas_pointer.is_none());
    h.run(vec![button(p + vec2(90.0, 70.0), false)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    let count = before.nodes().count();
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(50.0, 40.0)), button(p + vec2(50.0, 40.0), false)]);
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), count + 1);
}

#[test]
fn ipad_native_abort_without_app_access_resets_panel_click_on_next_actual_hook() {
    let mut h = Host::new();
    h.show_field = true;
    h.run(vec![]);
    h.run(vec![]);
    let panel = h.cancelled_button_rect.unwrap().center();
    h.run(vec![touch(1, TouchPhase::Start, panel), Event::PointerMoved(panel), button(panel, true)]);
    assert!(h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.native.abort_frame(&h.ui.ctx);
    assert!(h.native.consumer.layout.is_none());
    // Abort has no RawInput to process. The next genuine release carries the
    // cancellation controls without resetting backend suffix provenance.
    h.run(vec![touch(1, TouchPhase::End, panel), button(panel, false), Event::PointerGone]);
    assert!(!h.clicked);
    assert!(!h.ui.ctx.input(|i| i.pointer.primary_down()));
    h.run(vec![touch(2, TouchPhase::Start, panel), Event::PointerMoved(panel), button(panel, true)]);
    h.run(vec![touch(2, TouchPhase::End, panel), button(panel, false), Event::PointerGone]);
    assert!(h.clicked);
}
