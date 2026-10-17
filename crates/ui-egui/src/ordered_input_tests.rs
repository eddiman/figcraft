use super::*;
use crate::layers_drag_tests::Ui;
use egui::{Modifiers, TouchDeviceId, TouchId, pos2};
use figcraft_doc::NodeKind;
use figcraft_engine::{Session, textedit};
use serde_json::json;

fn text(app: &FigApp, id: NodeId) -> String {
    let NodeKind::Text(t) = &app.session.doc().unwrap().node(id).unwrap().kind else {
        panic!("text");
    };
    t.characters.clone()
}
fn setup() -> (Ui, NodeId, Target) {
    let mut u = Ui::new(Session::new());
    let id = NodeId(u.app.run("text.create", json!({"text":"Before", "x":20,"y":20})).unwrap()["id"].as_u64().unwrap());
    textedit::begin(&mut u.app.session, id, Some(6), false).unwrap();
    textedit::insert(&mut u.app.session, "A").unwrap();
    let target = Target::capture(&u.app, &u.ctx, 1).unwrap();
    (u, id, target)
}
fn touch(id: u64, phase: TouchPhase, p: egui::Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos: p, force: None }
}
fn button(down: bool, p: egui::Pos2) -> Event {
    Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE }
}
fn start(id: u64, p: egui::Pos2) -> Vec<Event> {
    vec![touch(id, TouchPhase::Start, p), Event::PointerMoved(p), button(true, p)]
}
fn raw(events: Vec<Event>, time: f64) -> RawInput {
    RawInput { time: Some(time), events, ..Default::default() }
}
/// A test-only consumer with an actual checkpointed caret preview and normal
/// acceptance. The production T16 recognizer is intentionally not installed.
struct Editor {
    text_id: NodeId,
    current: Option<ContactId>,
    seen: Vec<(u64, String)>,
    interruptions: Vec<Interruption>,
}
impl Editor {
    fn new(id: NodeId) -> Self {
        Self { text_id: id, current: None, seen: vec![], interruptions: vec![] }
    }
}
impl CanvasConsumer for Editor {
    fn routes_physical(&self) -> bool {
        false
    }
    fn physical_held(&self) -> bool {
        false
    }
    fn begin_frame(&mut self, _: &mut FigApp, _: &Context, _: &egui::RawInput, _: &crate::touch_events::Frame, _: bool) -> Result<bool, String> {
        Ok(false)
    }
    fn physical_input(&mut self, _: &mut FigApp, _: &Context, _: crate::ordered_input::PhysicalInput<'_>) -> bool {
        false
    }

    fn contact(&mut self, app: &mut FigApp, ctx: &Context, c: &Contact) {
        if !matches!(c.owner, Owner::Canvas { .. }) {
            return;
        }
        match c.phase {
            TouchPhase::Start => {
                self.seen.push((c.id.contact, text(app, self.text_id)));
                app.take_canvas_keyboard(ctx);
                app.begin_canvas_pointer(ctx).unwrap();
                app.pointer_session(|s| textedit::set_caret(s, 0, false)).unwrap();
                self.current = Some(c.id);
            }
            TouchPhase::End if self.current == Some(c.id) => {
                app.finish_canvas_pointer().unwrap();
                self.current = None;
            }
            TouchPhase::Cancel if self.current == Some(c.id) => {
                app.cancel_canvas_pointer();
                self.current = None;
            }
            _ => {}
        }
    }
    fn interrupt(&mut self, app: &mut FigApp, _: &Context, why: Interruption) {
        assert!(app.canvas_pointer.is_none(), "engine and UI cancel before consumer notification");
        self.current = None;
        self.interruptions.push(why);
    }
    fn producer_reset(&mut self, app: &mut FigApp, ctx: &Context) {
        self.interrupt(app, ctx, Interruption::SourceOrLayout);
    }
    fn finish_frame(&mut self, _: &mut FigApp, _: &Context, _: Option<(f64, Clock, u64)>) {}
}

#[test]
fn ipad_ordered_contacts_surround_text_in_one_batch_without_duplicate_typing() {
    let (mut u, id, target) = setup();
    let mut boundary = Boundary::default();
    let mut editor = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let mut events = start(1, p);
    events.push(Event::Text("B".into()));
    events.push(touch(2, TouchPhase::Start, p)); // Backend primary is still 1.
    let mut input = raw(events, 1.0);
    let frame = boundary.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut editor);
    assert!(frame.issues.is_empty());
    assert_eq!(editor.seen, vec![(1, "BeforeA".into()), (2, "BeforeAB".into())]);
    assert!(!input.events.iter().any(crate::touch_events::external_edit_event));
    assert!(u.app.canvas_pointer.is_some(), "later Start owns the post-typing checkpoint");
    // Older held contact was cancelled. Its late End cannot finish the newer one.
    let mut input = raw(vec![touch(1, TouchPhase::End, p), button(false, p), Event::PointerGone], 2.0);
    boundary.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut editor);
    assert!(u.app.canvas_pointer.is_some());
    assert_eq!(text(&u.app, id), "BeforeAB");
    u.app.cancel_canvas_pointer();
    u.frame(input.events);
    assert_eq!(text(&u.app, id), "BeforeAB", "legacy shortcuts cannot insert B a second time");
}

#[test]
fn ipad_ordered_text_between_frames_suppresses_only_the_cancelled_identity() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let mut input = raw(
        vec![Event::Ime(egui::ImeEvent::Commit("B".into())), touch(1, TouchPhase::Move, p), Event::PointerMoved(p), touch(2, TouchPhase::Start, p)],
        2.0,
    );
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    assert_eq!(e.seen, vec![(1, "BeforeA".into()), (2, "BeforeAB".into())]);
    assert_eq!(b.held.get(&ContactId { device: 1, contact: 1 }), Some(&true));
    assert_eq!(b.held.get(&ContactId { device: 1, contact: 2 }), Some(&false));
}

#[test]
fn ipad_ordered_panel_field_keeps_its_text_and_prevents_later_contact_from_running_early() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let field = egui::Id::new("panel field");
    u.ctx.memory_mut(|m| m.request_focus(field));
    u.app.field_focus = true;
    let mut input = raw(vec![Event::Text("Panel B".into()), touch(1, TouchPhase::Start, p), Event::PointerMoved(p), button(true, p)], 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    assert!(e.seen.is_empty());
    assert_eq!(text(&u.app, id), "BeforeA");
    assert_eq!(input.events, vec![Event::Text("Panel B".into())]);
    let mut field_text = String::new();
    u.ctx
        .run_ui(input, |ui| {
            ui.add(egui::TextEdit::singleline(&mut field_text).id(field));
        })
        .drop_without_applying_deltas();
    assert_eq!(field_text, "Panel B");
}

#[test]
fn ipad_ordered_forwarded_panel_start_cancels_before_egui_and_suppresses_later_canvas_start() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let panel = pos2(10.0, 100.0);
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let mut input = raw(vec![touch(2, TouchPhase::Start, panel), Event::Text("B".into()), touch(3, TouchPhase::Start, p)], 2.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |p| if p.x < 20.0 { Owner::Panel } else { target.owner() }, &mut e);
    assert!(u.app.canvas_pointer.is_none());
    assert_eq!(e.seen.len(), 1);
    assert!(input.events.iter().any(|e| matches!(e,Event::Text(t) if t=="B")));
    assert_eq!(text(&u.app, id), "BeforeA");
}

#[test]
fn ipad_ordered_layout_revision_cancels_before_delivering_old_contact_moves() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let next = Target::capture(&u.app, &u.ctx, 2).unwrap();
    let mut input = raw(vec![touch(1, TouchPhase::Move, p), Event::PointerMoved(p)], 2.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&next), |_| next.owner(), &mut e);
    assert!(u.app.canvas_pointer.is_none());
    assert_eq!(text(&u.app, id), "BeforeA");
    assert!(e.interruptions.contains(&Interruption::SourceOrLayout));
}

#[test]
fn ipad_ordered_alt_shortcut_text_is_swallowed_across_contact_slices() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let modifiers = Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE };
    let mut events = vec![Event::Key { key: egui::Key::B, physical_key: None, pressed: true, repeat: false, modifiers }];
    events.extend(start(1, p));
    events.push(Event::Text("b".into()));
    let mut input = raw(events, 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    assert_eq!(text(&u.app, id), "BeforeA");
    assert!(input.events.is_empty());
    assert!(u.app.canvas_pointer.is_none());
}

#[test]
fn ipad_ordered_external_command_invalidates_a_pending_contact_without_a_checkpoint() {
    struct Pending {
        pending: bool,
        cancelled: bool,
    }
    impl CanvasConsumer for Pending {
        fn routes_physical(&self) -> bool {
            false
        }
        fn physical_held(&self) -> bool {
            false
        }
        fn begin_frame(&mut self, _: &mut FigApp, _: &Context, _: &egui::RawInput, _: &crate::touch_events::Frame, _: bool) -> Result<bool, String> {
            Ok(false)
        }
        fn physical_input(&mut self, _: &mut FigApp, _: &Context, _: crate::ordered_input::PhysicalInput<'_>) -> bool {
            false
        }

        fn contact(&mut self, _: &mut FigApp, _: &Context, c: &Contact) {
            if matches!(c.owner, Owner::Canvas { .. }) && c.phase == TouchPhase::Start {
                self.pending = true;
            }
        }
        fn interrupt(&mut self, _: &mut FigApp, _: &Context, _: Interruption) {
            self.cancelled |= self.pending;
            self.pending = false;
        }
        fn producer_reset(&mut self, app: &mut FigApp, ctx: &Context) {
            self.interrupt(app, ctx, Interruption::SourceOrLayout);
        }
        fn finish_frame(&mut self, _: &mut FigApp, _: &Context, _: Option<(f64, Clock, u64)>) {}
    }
    let (mut u, _, target) = setup();
    let mut b = Boundary::default();
    let p = u.app.canvas.rect.center();
    let mut c = Pending { pending: false, cancelled: false };
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut c);
    assert!(c.pending);
    assert!(u.app.canvas_pointer.is_none());
    u.app.run("view.zoomIn", json!({})).unwrap();
    let mut input = raw(vec![touch(1, TouchPhase::Move, p), Event::PointerMoved(p)], 2.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut c);
    assert!(c.cancelled && !c.pending);
}

#[test]
fn ipad_ordered_unsupported_suffix_cannot_fall_back_to_legacy_canvas_down() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    textedit::end(&mut u.app.session).unwrap();
    u.app.run("tool.rectangle", json!({})).unwrap();
    let count = u.app.session.doc().unwrap().nodes().count();
    let other = p + egui::vec2(10.0, 10.0);
    let mut input = raw(vec![touch(1, TouchPhase::Start, p), Event::PointerMoved(other), button(true, other)], 1.0);
    let frame = b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    assert!(frame.blocked && u.app.canvas_input_blocked);
    u.frame(input.events);
    assert_eq!(u.app.session.doc().unwrap().nodes().count(), count);
}

#[test]
fn ipad_ordered_panel_cancel_cleans_guide_before_reset_up_reaches_egui() {
    let (mut u, id, _) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    u.app.session.ds_mut().unwrap().view.rulers = true;
    let target = Target::capture(&u.app, &u.ctx, 1).unwrap();
    let r = u.app.canvas.rect;
    let p = pos2(r.min.x + 5.0, r.center().y);
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| Owner::Panel, &mut e);
    u.app.begin_canvas_pointer(&u.ctx).unwrap();
    b.last_interruption = u.app.canvas_interruption;
    let before = u.app.session.ds().unwrap().doc.clone();
    let depth = u.app.session.ds().unwrap().history.undo.len();
    u.ctx
        .run_ui(input, |_| {
            crate::guides::input(&mut u.app, &u.ctx, p);
        })
        .drop_without_applying_deltas();
    let end = pos2(r.center().x, r.center().y);
    let mut input = raw(vec![touch(1, TouchPhase::Cancel, end), Event::PointerGone], 2.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| Owner::Panel, &mut e);
    assert!(u.app.canvas_pointer.is_none());
    u.ctx
        .run_ui(input, |_| {
            crate::guides::input(&mut u.app, &u.ctx, end);
        })
        .drop_without_applying_deltas();
    assert!(!u.ctx.input(|i| i.pointer.primary_down()));
    assert!(std::sync::Arc::ptr_eq(&before, &u.app.session.ds().unwrap().doc));
    assert_eq!(u.app.session.ds().unwrap().history.undo.len(), depth);
}

#[test]
fn ipad_ordered_panel_cancel_preserves_an_independently_held_mouse() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = pos2(10.0, 100.0);
    let mut events = vec![Event::PointerMoved(p), button(true, p)];
    events.extend(start(1, p));
    let mut input = raw(events, 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| Owner::Panel, &mut e);
    u.ctx.run_ui(input, |_| {}).drop_without_applying_deltas();
    let mut input = raw(vec![touch(1, TouchPhase::Cancel, p), Event::PointerGone], 2.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| Owner::Panel, &mut e);
    u.ctx.run_ui(input, |_| {}).drop_without_applying_deltas();
    assert!(u.ctx.input(|i| i.pointer.primary_down()));
}

#[test]
fn ipad_ordered_generation_exhaustion_cannot_reuse_pending_ownership() {
    let (mut u, _, _) = setup();
    u.app.canvas_interruption = Some(u64::MAX);
    assert!(u.app.begin_canvas_pointer(&u.ctx).is_err());
    assert!(u.app.canvas_interruption.is_none());
    assert!(Target::capture(&u.app, &u.ctx, 1).is_none());
    assert!(u.app.begin_canvas_pointer(&u.ctx).is_err());
}

#[test]
fn ipad_ordered_overflow_cancels_but_forwards_text_to_its_existing_route_once() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let mut events = vec![Event::PointerGone; 4097];
    events.push(Event::Text("B".into()));
    let mut input = raw(events, 2.0);
    let frame = b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    assert!(frame.blocked && u.app.canvas_pointer.is_none());
    assert_eq!(text(&u.app, id), "BeforeA");
    u.frame(input.events);
    assert_eq!(text(&u.app, id), "BeforeAB");
}

fn raw_layout_change_cancels_before_aggregation(change: impl FnOnce(&mut RawInput)) {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let p = u.app.canvas.rect.center();
    let mut input = raw(start(1, p), 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let mut input = raw(vec![touch(1, TouchPhase::Move, p), Event::PointerMoved(p)], 2.0);
    change(&mut input);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    assert!(u.app.canvas_pointer.is_none(), "raw layout changes must cancel before egui updates its context");
    assert_eq!(u.app.session.ds().unwrap().text_edit.as_ref().unwrap().caret, 7);
}
#[test]
fn ipad_ordered_raw_scale_change_cancels_before_aggregation() {
    raw_layout_change_cancels_before_aggregation(|raw| {
        raw.viewports.entry(raw.viewport_id).or_default().native_pixels_per_point = Some(4.0);
    });
}
#[test]
fn ipad_ordered_raw_safe_area_change_cancels_before_aggregation() {
    raw_layout_change_cancels_before_aggregation(|raw| {
        let mut insets = egui::SafeAreaInsets::default();
        insets.0.top = 40.0;
        raw.safe_area_insets = Some(insets);
    });
}

#[test]
fn ipad_ordered_old_frame_triple_click_is_not_replayed_after_typing() {
    let (mut u, id, _) = setup();
    let p = u.app.to_screen(25.0, 25.0);
    let events = (0..3).flat_map(|_| vec![Event::PointerMoved(p), button(true, p), button(false, p)]).collect();
    u.ctx.run_ui(raw(events, 1.0), |_| {}).drop_without_applying_deltas();
    assert!(u.ctx.input(|i| i.pointer.button_triple_clicked(egui::PointerButton::Primary)));
    let target = Target::capture(&u.app, &u.ctx, 1).unwrap();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let mut input = raw(vec![Event::Text("B".into())], 2.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let te = u.app.session.ds().unwrap().text_edit.as_ref().unwrap();
    assert_eq!((te.anchor, te.caret), (8, 8), "only the current text event can change the selection");
}

#[test]
fn ipad_ordered_old_frame_output_does_not_lose_the_current_ime_caret() {
    let (mut u, id, target) = setup();
    let mut b = Boundary::default();
    let mut e = Editor::new(id);
    let mut input = raw(vec![Event::Ime(egui::ImeEvent::Preedit { text: "候補".into(), active_range_chars: Some(1..1) })], 1.0);
    b.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut e);
    let output = u.frame_output(input.events);
    assert!(output.platform_output.ime.is_some(), "the rendered frame must report the current IME caret");
}

struct QueuedFinger {
    finger: crate::touch_native::Finger,
    on_start: bool,
    on_second_prepare: bool,
    frames: usize,
}
impl CanvasConsumer for QueuedFinger {
    fn routes_physical(&self) -> bool {
        false
    }
    fn physical_held(&self) -> bool {
        false
    }
    fn begin_frame(&mut self, app: &mut FigApp, ctx: &Context, raw: &RawInput, frame: &Frame, current: bool) -> Result<bool, String> {
        self.frames += 1;
        let result = self.finger.begin_frame(app, ctx, raw, frame, current);
        if self.on_second_prepare && self.frames == 2 {
            FigApp::request_canvas_pointer_cancel(ctx);
        }
        result
    }
    fn physical_input(&mut self, _: &mut FigApp, _: &Context, _: PhysicalInput<'_>) -> bool {
        false
    }
    fn contact(&mut self, app: &mut FigApp, ctx: &Context, contact: &Contact) {
        self.finger.contact(app, ctx, contact);
        if self.on_start && contact.phase == TouchPhase::Start {
            FigApp::request_canvas_pointer_cancel(ctx);
        }
    }
    fn interrupt(&mut self, app: &mut FigApp, ctx: &Context, why: Interruption) {
        self.finger.interrupt(app, ctx, why);
    }
    fn producer_reset(&mut self, app: &mut FigApp, ctx: &Context) {
        self.finger.producer_reset(app, ctx);
    }
    fn finish_frame(&mut self, app: &mut FigApp, ctx: &Context, time: Option<(f64, Clock, u64)>) {
        self.finger.finish_frame(app, ctx, time);
    }
}
#[test]
fn ipad_ordered_queue_between_pending_start_and_move_cannot_emit_down() {
    let mut session = Session::new();
    session.execute("tool.rectangle", &json!({})).unwrap();
    let mut u = Ui::new(session);
    let target = Target::capture(&u.app, &u.ctx, 1).unwrap();
    let p = u.app.canvas.rect.center();
    let before = u.app.session.ds().unwrap().doc.clone();
    let mut consumer = QueuedFinger { finger: Default::default(), on_start: true, on_second_prepare: false, frames: 0 };
    let mut boundary = Boundary::default();
    let mut events = start(1, p);
    events.extend([touch(1, TouchPhase::Move, p + egui::vec2(50.0, 40.0)), Event::PointerMoved(p + egui::vec2(50.0, 40.0))]);
    let mut input = raw(events, 1.0);
    assert!(boundary.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut consumer).issues.is_empty());
    assert!(std::sync::Arc::ptr_eq(&before, &u.app.session.ds().unwrap().doc));
    assert!(u.app.canvas_pointer.is_none());
    assert_eq!(consumer.finger.phase(), crate::touch::Phase::SuppressedUntilAllReleased);
}
#[test]
fn ipad_ordered_queue_during_prepare_prevents_a_due_long_press() {
    let mut u = Ui::new(Session::new());
    let target = Target::capture(&u.app, &u.ctx, 1).unwrap();
    let p = u.app.canvas.rect.center();
    let mut consumer = QueuedFinger { finger: Default::default(), on_start: false, on_second_prepare: true, frames: 0 };
    let mut boundary = Boundary::default();
    let mut input = raw(start(1, p), 1.0);
    assert!(boundary.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut consumer).issues.is_empty());
    let mut input = raw(vec![], 1.6);
    assert!(boundary.process(&mut u.app, &u.ctx, &mut input, Some(&target), |_| target.owner(), &mut consumer).issues.is_empty());
    assert!(u.app.ui.context_menu.is_none(), "cancel queued during preparation precedes the due timer");
    assert_eq!(consumer.finger.phase(), crate::touch::Phase::SuppressedUntilAllReleased);
    assert!(u.app.canvas_pointer.is_none());
}
