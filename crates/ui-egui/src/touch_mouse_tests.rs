use super::*;
use crate::touch_events::{Normalizer, Owner, Routing};
use egui::{PointerButton, TouchDeviceId, TouchId, TouchPhase, pos2, vec2};

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}
fn raw(time: f64, events: Vec<Event>) -> RawInput {
    RawInput { time: Some(time), events, ..Default::default() }
}
fn prepare(mirror: &mut Physical, normalizer: &mut Normalizer, ctx: &Context, mut raw: RawInput) -> PhysicalFrame {
    let frame = normalizer.process(&mut raw, Routing::CanvasConsumer, |_| Owner::Panel);
    assert!(frame.issues.is_empty(), "{:?}", frame.issues);
    mirror.prepare(ctx, &raw, &frame).unwrap()
}
fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: None }
}
fn key(key: Key, pressed: bool) -> Event {
    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE }
}
#[test]
fn ipad_physical_clicks_reuse_pinned_policy_in_raw_order() {
    let ctx = Context::default();
    let mut mirror = Physical::default();
    let mut n = Normalizer::default();
    let p = pos2(20.0, 30.0);
    let mut events = vec![];
    for _ in 0..3 {
        events.extend([Event::PointerMoved(p), button(p, true), button(p, false)]);
    }
    let result = prepare(&mut mirror, &mut n, &ctx, raw(1.0, events));
    let counts: Vec<_> =
        result.samples.iter().filter_map(|s| matches!(s.event, Event::PointerButton { pressed: false, .. }).then_some(s.clicks)).collect();
    assert_eq!(counts, [1, 2, 3]);
    let result = prepare(&mut mirror, &mut n, &ctx, raw(2.0, vec![Event::PointerMoved(p), button(p, true)]));
    assert!(result.samples.iter().all(|s| s.clicks == 0), "preceding-frame clicks cannot be replayed");
    let result = prepare(&mut mirror, &mut n, &ctx, raw(3.0, vec![button(p, false)]));
    assert_eq!(result.samples[0].clicks, 0, "a long-held pointer is not a click");
}
#[test]
fn ipad_physical_panel_suffix_never_becomes_an_independent_mouse() {
    let ctx = Context::default();
    let mut mirror = Physical::default();
    let mut n = Normalizer::default();
    let p = pos2(30.0, 30.0);
    let events = vec![touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(p, true), Event::PointerMoved(p), button(p, true)];
    let result = prepare(&mut mirror, &mut n, &ctx, raw(1.0, events));
    assert_eq!(result.samples.len(), 2);
    assert_eq!(result.samples[0].ordinal, 3);
    assert_eq!(result.samples[1].ordinal, 4);
    assert!(result.samples[1].any_down);
    let result = prepare(&mut mirror, &mut n, &ctx, raw(1.1, vec![touch(1, TouchPhase::Cancel, p), Event::PointerGone]));
    assert!(result.samples.is_empty(), "panel cancel cannot release physical mouse state");
    assert!(mirror.state.pointer.primary_down());
    let result = prepare(&mut mirror, &mut n, &ctx, raw(1.2, vec![button(p, false)]));
    assert_eq!(result.samples.len(), 1);
    assert!(!result.samples[0].any_down);
}
#[test]
fn ipad_physical_mode_keys_and_modifiers_follow_pointer_ordinals() {
    let ctx = Context::default();
    let mut mirror = Physical::default();
    let mut n = Normalizer::default();
    let p = pos2(20.0, 30.0);
    let events = vec![
        key(Key::Space, true),
        Event::ModifiersChanged(Modifiers::ALT),
        button(p, true),
        key(Key::Z, true),
        Event::ModifiersChanged(Modifiers::SHIFT),
        Event::PointerMoved(p),
        key(Key::Space, false),
        button(p, false),
    ];
    let frame = prepare(&mut mirror, &mut n, &ctx, raw(1.0, events));
    assert!(frame.samples[0].space && !frame.samples[0].zoom_key);
    assert_eq!(frame.samples[0].modifiers, Modifiers::NONE, "button keeps its event-local modifiers");
    assert!(frame.samples[1].space && frame.samples[1].zoom_key);
    assert_eq!(frame.samples[1].modifiers, Modifiers::SHIFT);
    assert!(!frame.samples[2].space && frame.samples[2].zoom_key);
}
#[test]
fn ipad_physical_wheel_and_zoom_match_touch_free_egui_across_frames() {
    let ctx = Context::default();
    let mut mirror = Physical::default();
    let mut n = Normalizer::default();
    let mut canonical = InputState::default();
    let p = pos2(300.0, 300.0);
    for (i, events) in [
        vec![
            Event::PointerMoved(p),
            Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: vec2(0.0, 2.0), phase: TouchPhase::Move, modifiers: Modifiers::NONE },
        ],
        vec![],
        vec![],
        vec![
            Event::ModifiersChanged(Modifiers::COMMAND),
            Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, 20.0), phase: TouchPhase::Move, modifiers: Modifiers::COMMAND },
        ],
        vec![],
        vec![Event::Zoom(1.1)],
    ]
    .into_iter()
    .enumerate()
    {
        let time = 1.0 + i as f64 / 60.0;
        let expected_raw = raw(time, events.clone());
        canonical = canonical.begin_pass(expected_raw, ctx.requested_repaint_last_pass(), ctx.pixels_per_point(), ctx.options(|o| o.input_options));
        let mut mixed = events;
        if i == 0 {
            mixed.extend([touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(p, true), touch(2, TouchPhase::Start, p + vec2(20.0, 0.0))]);
        } else {
            mixed.extend([touch(2, TouchPhase::Move, p + vec2(20.0 + i as f32 * 50.0, 0.0))]);
        }
        let result = prepare(&mut mirror, &mut n, &ctx, raw(time, mixed));
        assert_eq!(mirror.state.smooth_scroll_delta, canonical.smooth_scroll_delta);
        assert_eq!(mirror.state.zoom_delta(), canonical.zoom_delta());
        if let Some(nav) = result.navigation {
            assert_eq!(nav.scroll, canonical.smooth_scroll_delta);
            assert_eq!(nav.zoom, canonical.zoom_delta());
            assert_eq!(nav.position, Some(p));
        }
        assert!(!mirror.state.raw.events.iter().any(|e| matches!(e, Event::Touch { .. })), "the physical mirror never sees panel pinch");
    }
}
#[test]
fn ipad_physical_mirror_does_not_retain_text_ime_or_drop_handles() {
    let ctx = Context::default();
    let mut mirror = Physical::default();
    let mut n = Normalizer::default();
    let events = vec![
        Event::Text("Synthetic typing".into()),
        Event::Paste("Synthetic paste".into()),
        Event::Ime(egui::ImeEvent::Commit("Synthetic IME".into())),
        Event::Copy,
        key(Key::A, true),
        key(Key::Z, true),
    ];
    let result = prepare(&mut mirror, &mut n, &ctx, raw(1.0, events));
    assert!(result.samples.is_empty());
    assert!(mirror.state.raw.events.iter().all(|e| matches!(e, Event::Key { key: Key::Z, .. })));
    assert!(mirror.state.raw.dropped_files.is_empty());
    assert!(!mirror.state.key_down(Key::A));
}
#[test]
fn ipad_physical_invalid_navigation_latches_until_explicit_focus_reset() {
    let ctx = Context::default();
    let mut mirror = Physical::default();
    let mut n = Normalizer::default();
    let mut input = raw(1.0, vec![Event::Zoom(f32::NAN)]);
    let frame = n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    assert_eq!(mirror.prepare(&ctx, &input, &frame).err(), Some(Error::NonFinite));
    let mut input = raw(2.0, vec![]);
    let frame = n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    assert_eq!(mirror.prepare(&ctx, &input, &frame).err(), Some(Error::Unsupported));
    let frame = prepare(&mut mirror, &mut n, &ctx, raw(3.0, vec![Event::WindowFocused(false)]));
    assert!(frame.reset);
    let frame = prepare(&mut mirror, &mut n, &ctx, raw(4.0, vec![button(pos2(30.0, 30.0), true)]));
    assert_eq!(frame.samples.len(), 1);
}

use crate::layers_drag_tests::Ui;
use figcraft_engine::Session;
use serde_json::json;
use std::sync::Arc;
struct Host {
    ui: Ui,
    mirror: Physical,
    normalizer: Normalizer,
    mouse: Mouse,
    time: f64,
}
impl Host {
    fn new(tool: &str) -> Self {
        let mut session = Session::new();
        session.execute(tool, &json!({})).unwrap();
        Self { ui: Ui::new(session), mirror: Physical::default(), normalizer: Normalizer::default(), mouse: Mouse::default(), time: 1.0 }
    }
    fn run(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let frame = prepare(&mut self.mirror, &mut self.normalizer, &self.ui.ctx, raw(self.time, events));
        for sample in &frame.samples {
            let position = match sample.event {
                Event::PointerMoved(p) | Event::PointerButton { pos: p, .. } => Some(p),
                _ => None,
            };
            let hit = if position.is_some_and(|p| self.ui.app.canvas.rect.contains(p)) {
                Owner::Canvas { uid: self.ui.app.session.ds().unwrap().uid, layout: 1 }
            } else {
                Owner::Panel
            };
            self.mouse.sample(&mut self.ui.app, &self.ui.ctx, sample, hit, true).unwrap();
        }
        self.ui.frame(vec![]);
    }
    fn p(&self) -> Pos2 {
        self.ui.app.canvas.rect.center()
    }
}
#[test]
fn ipad_physical_value_draw_commits_once_and_cancel_suppresses_held_button() {
    let mut h = Host::new("tool.rectangle");
    let p = h.p();
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let undo = h.ui.app.session.ds().unwrap().history.undo.len();
    h.run(vec![Event::PointerMoved(p), button(p, true), Event::PointerMoved(p + vec2(40.0, 30.0))]);
    assert!(h.mouse.active());
    assert!(!Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    h.mouse.interrupt(&mut h.ui.app);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    h.run(vec![Event::PointerMoved(p + vec2(80.0, 50.0)), button(p + vec2(80.0, 50.0), false)]);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    h.run(vec![button(p, true), Event::PointerMoved(p + vec2(40.0, 30.0)), button(p + vec2(40.0, 30.0), false)]);
    assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo + 1);
    assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), before.nodes().count() + 1);
    assert!(h.ui.app.canvas_pointer.is_none());
    assert!(!h.mouse.active());
}
#[test]
fn ipad_physical_value_space_and_middle_pan_keep_view_without_document_undo() {
    for middle in [false, true] {
        for cancel in [false, true] {
            let mut h = Host::new("tool.rectangle");
            let p = h.p();
            let before = h.ui.app.session.ds().unwrap().doc.clone();
            let ds = h.ui.app.session.ds().unwrap();
            let origin = (ds.view.cx, ds.view.cy);
            let zoom = ds.view.zoom;
            let undo = ds.history.undo.len();
            let mut start = vec![];
            if !middle {
                start.push(key(Key::Space, true));
            }
            let button_value = |pressed| Event::PointerButton {
                pos: p,
                button: if middle { PointerButton::Middle } else { PointerButton::Primary },
                pressed,
                modifiers: Modifiers::NONE,
            };
            start.push(button_value(true));
            start.push(Event::PointerMoved(p + vec2(40.0, 30.0)));
            h.run(start);
            let view = &h.ui.app.session.ds().unwrap().view;
            assert!((view.cx - (origin.0 - 40.0 / zoom)).abs() < 1e-6);
            assert!((view.cy - (origin.1 - 30.0 / zoom)).abs() < 1e-6);
            if cancel {
                h.mouse.interrupt(&mut h.ui.app);
            }
            let mut release = button_value(false);
            if let Event::PointerButton { pos, .. } = &mut release {
                *pos = p + vec2(40.0, 30.0);
            }
            h.run(vec![release]);
            let ds = h.ui.app.session.ds().unwrap();
            if cancel {
                assert_eq!((ds.view.cx, ds.view.cy), origin);
            } else {
                assert_ne!((ds.view.cx, ds.view.cy), origin);
            }
            assert!(Arc::ptr_eq(&before, &ds.doc));
            assert_eq!(ds.history.undo.len(), undo);
            assert!(h.ui.app.canvas_pointer.is_none());
        }
    }
}
#[test]
fn ipad_physical_value_zoom_key_click_and_area_reuse_view_command() {
    for area in [false, true] {
        let mut h = Host::new("tool.rectangle");
        let p = h.p();
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        let zoom = h.ui.app.session.ds().unwrap().view.zoom;
        let undo = h.ui.app.session.ds().unwrap().history.undo.len();
        h.run(vec![key(Key::Z, true), button(p, true)]);
        assert!(h.mouse.active());
        let end = if area { p + vec2(100.0, 80.0) } else { p };
        h.run(vec![Event::PointerMoved(end), button(end, false)]);
        assert!(h.ui.app.session.ds().unwrap().view.zoom > zoom);
        assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        assert_eq!(h.ui.app.session.ds().unwrap().history.undo.len(), undo);
        assert!(h.ui.app.canvas_pointer.is_none());
    }
}
#[test]
fn ipad_physical_value_secondary_click_waits_for_release_and_drag_opens_no_menu() {
    for dragged in [false, true] {
        let mut h = Host::new("tool.rectangle");
        let p = h.p();
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        let b = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
        h.run(vec![b(p, true)]);
        assert!(h.ui.app.ui.context_menu.is_none());
        let end = if dragged { p + vec2(30.0, 30.0) } else { p };
        h.run(vec![Event::PointerMoved(end), b(end, false)]);
        assert_eq!(h.ui.app.ui.context_menu.is_some(), !dragged);
        assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    }
}
#[test]
fn ipad_physical_value_panel_press_cannot_start_edit_after_entering_canvas() {
    let mut h = Host::new("tool.rectangle");
    let p = h.p();
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    h.run(vec![button(pos2(1.0, 1.0), true), Event::PointerMoved(p), button(p, false)]);
    assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
    assert!(h.ui.app.canvas_pointer.is_none());
}
#[test]
fn ipad_physical_value_triple_click_selects_line_without_replaying_egui_flags() {
    let mut h = Host::new("tool.move");
    let id = figcraft_doc::NodeId(
        h.ui.app.run("text.create", json!({"text":"one two three","x":0,"y":0,"width":240,"props":{"fontSize":24}})).unwrap()["id"].as_u64().unwrap(),
    );
    figcraft_engine::textedit::begin(&mut h.ui.app.session, id, Some(0), false).unwrap();
    h.ui.frame(vec![]);
    let p = h.ui.app.to_screen(24.0, 16.0);
    assert!(h.ui.app.session.doc().unwrap().world_bounds(id).unwrap().contains(h.ui.app.to_page(p)));
    h.run((0..2).flat_map(|_| vec![button(p, true), button(p, false)]).collect());
    let te = h.ui.app.session.ds().unwrap().text_edit.as_ref().unwrap();
    assert!(te.anchor != te.caret && te.anchor.abs_diff(te.caret) < 13, "double click selects one word");
    assert!(!h.ui.app.session.tool.busy(), "physical double-click release clears the completed selection drag");
    h.run(vec![button(p, true), button(p, false)]);
    let te = h.ui.app.session.ds().unwrap().text_edit.as_ref().unwrap();
    assert_eq!((te.anchor.min(te.caret), te.anchor.max(te.caret)), (0, 13));
    assert!(!h.ui.ctx.input(|i| i.pointer.button_triple_clicked(PointerButton::Primary)));
    assert!(h.ui.app.canvas_pointer.is_none());
}
