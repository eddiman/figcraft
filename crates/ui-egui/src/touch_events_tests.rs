use super::*;
use egui::{Context, TouchDeviceId, TouchId, pos2};
fn touch(device: u64, id: u64, phase: TouchPhase, p: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(device), id: TouchId(id), phase, pos: p, force: Some(0.7) }
}
fn button(pressed: bool, p: Pos2) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::default() }
}
fn start(device: u64, id: u64, p: Pos2) -> Vec<Event> {
    vec![touch(device, id, TouchPhase::Start, p), Event::PointerMoved(p), button(true, p)]
}
fn raw(events: Vec<Event>, time: f64) -> RawInput {
    RawInput { events, time: Some(time), ..Default::default() }
}
fn canvas(_: Pos2) -> Owner {
    Owner::Canvas { uid: 7, layout: 1 }
}
fn run(ctx: &Context, input: RawInput) {
    ctx.run_ui(input, |_| {}).drop_without_applying_deltas();
}
#[test]
fn ipad_touch_observer_preserves_every_raw_event() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut events = start(1, 9, p);
    events.push(Event::Text("Typed once".into()));
    let mut input = raw(events.clone(), 1.0);
    let frame = n.process(&mut input, Routing::Observe, canvas);
    assert_eq!(input.events, events);
    assert!(frame.issues.is_empty());
    assert!(matches!(&frame.items[0],Item::Contact(c) if c.source==Source::Unknown&&c.pressure==Some(0.7)));
}
#[test]
fn ipad_touch_canvas_suffix_is_removed_before_egui_pointer_state() {
    let ctx = Context::default();
    let mut n = Normalizer::default();
    let mut input = raw(start(1, 9, pos2(10.0, 20.0)), 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.is_empty());
    run(&ctx, input);
    assert!(!ctx.input(|i| i.pointer.primary_pressed() || i.pointer.primary_down()));
}
#[test]
fn ipad_touch_identical_position_physical_mouse_is_preserved() {
    let p = pos2(10.0, 20.0);
    let mut events = start(1, 9, p);
    events.extend([Event::PointerMoved(p), button(true, p)]);
    let mut input = raw(events, 1.0);
    let mut n = Normalizer::default();
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert_eq!(input.events, vec![Event::PointerMoved(p), button(true, p)]);
    assert!(frame.physical_primary_down);
}
#[test]
fn ipad_touch_panel_cancel_resets_egui_button_after_owner_cancellation_notice() {
    let ctx = Context::default();
    let p = pos2(10.0, 20.0);
    let mut n = Normalizer::default();
    let mut input = raw(start(1, 9, p), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    run(&ctx, input);
    assert!(ctx.input(|i| i.pointer.primary_down()));
    let mut input = raw(vec![touch(1, 9, TouchPhase::Cancel, p), Event::PointerGone], 2.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    assert!(frame.items.iter().any(|e| matches!(e, Item::PanelCancelled { .. })));
    run(&ctx, input);
    assert!(!ctx.input(|i| i.pointer.primary_down()));
}
#[test]
fn ipad_touch_panel_cancel_cannot_release_a_held_physical_mouse() {
    let ctx = Context::default();
    let p = pos2(10.0, 20.0);
    let mut n = Normalizer::default();
    let mut events = vec![Event::PointerMoved(p), button(true, p)];
    events.extend(start(1, 9, p));
    let mut input = raw(events, 1.0);
    n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    run(&ctx, input);
    let mut input = raw(vec![touch(1, 9, TouchPhase::Cancel, p), Event::PointerGone], 2.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    assert!(frame.physical_primary_down);
    assert!(!input.events.iter().any(|e| matches!(e, Event::PointerButton { pressed: false, .. })));
    run(&ctx, input);
    assert!(ctx.input(|i| i.pointer.primary_down()));
}
#[test]
fn ipad_touch_owner_is_frozen_across_panel_canvas_crossing() {
    let mut n = Normalizer::default();
    let mut input = raw(start(1, 9, pos2(10.0, 20.0)), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    let p = pos2(400.0, 400.0);
    let mut input = raw(vec![touch(1, 9, TouchPhase::Move, p), Event::PointerMoved(p)], 2.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert_eq!(input.events.len(), 2);
    assert!(matches!(&frame.items[0],Item::Contact(c) if c.owner==Owner::Panel));
}
#[test]
fn ipad_touch_end_uses_backend_cursor_position_not_contact_position() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut input = raw(start(1, 9, p), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    let mouse = pos2(90.0, 80.0);
    let mut input =
        raw(vec![Event::PointerMoved(mouse), touch(1, 9, TouchPhase::End, pos2(30.0, 40.0)), button(false, mouse), Event::PointerGone], 2.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.is_empty());
    assert_eq!(input.events, vec![Event::PointerMoved(mouse)]);
}
#[test]
fn ipad_touch_primary_loss_mirrors_non_promoting_backend_moves() {
    let mut n = Normalizer::default();
    let a = pos2(10.0, 20.0);
    let b = pos2(40.0, 50.0);
    let mut events = start(1, 1, a);
    events.push(touch(1, 2, TouchPhase::Start, b));
    let mut input = raw(events, 1.0);
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    let mut input = raw(
        vec![touch(1, 1, TouchPhase::End, a), button(false, a), Event::PointerGone, touch(1, 2, TouchPhase::Move, b), Event::PointerMoved(b)],
        2.0,
    );
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.is_empty());
    assert!(input.events.is_empty());
    assert_eq!(n.backend_primary, None);
    let mut input = raw(start(1, 3, a), 3.0);
    assert!(n.process(&mut input, Routing::CanvasConsumer, canvas).issues.is_empty());
    assert_eq!(n.backend_primary, Some(3));
}
#[test]
fn ipad_touch_bad_suffix_is_reported_without_position_heuristics() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let other = pos2(99.0, 99.0);
    let mut input = raw(vec![touch(1, 1, TouchPhase::Start, p), Event::PointerMoved(other), button(true, other)], 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.blocked && frame.issues.contains(&Issue::InvalidSuffix));
    assert_eq!(input.events, vec![Event::PointerMoved(other), button(true, other)]);
}
#[test]
fn ipad_touch_numeric_device_id_collisions_cancel_instead_of_merging() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut input = raw(start(1, 1, p), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    let mut input = raw(start(2, 1, p), 2.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.contains(&Issue::NumericIdCollision));
    assert!(frame.blocked);
    assert!(matches!(&frame.items[0],Item::Contact(c) if c.owner==Owner::Suppressed));
}
#[test]
fn ipad_touch_invalid_clock_and_coordinates_never_start_canvas_editing() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut input = raw(start(1, 1, p), f64::NAN);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.contains(&Issue::InvalidClock));
    assert!(matches!(&frame.items[0],Item::Contact(c) if c.owner==Owner::Suppressed));
    n.reset();
    let mut input = raw(start(1, 1, pos2(f32::NAN, 20.0)), 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.contains(&Issue::NonFinite));
    assert!(frame.blocked);
}
#[test]
fn ipad_touch_bounded_records_do_not_queue_or_drop_large_text() {
    let mut n = Normalizer::default();
    let text = "Original input ".repeat(1000);
    let mut events = vec![Event::PointerGone; MAX_ITEMS + 1];
    events.push(Event::Text(text.clone()));
    let mut input = raw(events.clone(), 1.0);
    let frame = n.process(&mut input, Routing::Observe, canvas);
    assert!(frame.issues.contains(&Issue::Capacity));
    assert!(frame.items.len() <= MAX_ITEMS);
    assert!(frame.issues.len() <= 8);
    assert_eq!(input.events, events);
}
#[test]
fn ipad_touch_duplicate_and_late_phases_are_explicit_errors() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut input = raw(start(1, 1, p), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    let mut input = raw(start(1, 1, p), 2.0);
    assert!(n.process(&mut input, Routing::CanvasConsumer, canvas).issues.contains(&Issue::DuplicateStart));
    n.reset();
    let mut input = raw(vec![touch(1, 1, TouchPhase::End, p), Event::PointerGone], 1.0);
    assert!(n.process(&mut input, Routing::CanvasConsumer, canvas).issues.contains(&Issue::MissingStart));
}
#[test]
fn ipad_touch_keyboard_indices_preserve_contact_text_contact_order() {
    let mut n = Normalizer::default();
    let a = pos2(10.0, 20.0);
    let b = pos2(30.0, 40.0);
    let mut events = start(1, 1, a);
    events.push(Event::Text("B".into()));
    events.push(touch(1, 2, TouchPhase::Start, b));
    let mut input = raw(events, 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(matches!(&frame.items[0],Item::Contact(c) if c.ordinal==0));
    assert!(matches!(&frame.items[1], Item::Input { ordinal: 3, index: 0, kind: InputKind::Keyboard }));
    assert!(matches!(&frame.items[2],Item::Contact(c) if c.ordinal==4));
    assert_eq!(input.events, vec![Event::Text("B".into())]);
}

#[test]
fn ipad_touch_focus_event_suppresses_later_start_in_the_same_frame() {
    let mut n = Normalizer::default();
    let mut events = vec![Event::WindowFocused(false)];
    events.extend(start(1, 1, pos2(10.0, 20.0)));
    let mut input = raw(events, 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.blocked && frame.interrupted);
    assert!(matches!(&frame.items[1], Item::Contact(c) if c.owner == Owner::Suppressed));
    assert!(n.contacts.is_empty());
}

#[test]
fn ipad_touch_invalid_sequence_stays_suppressed_until_focus_reset() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut input = raw(vec![touch(1, 1, TouchPhase::Move, p), Event::PointerMoved(p)], 1.0);
    assert!(n.process(&mut input, Routing::CanvasConsumer, canvas).blocked);
    let mut input = raw(start(1, 2, p), 2.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.blocked);
    assert!(matches!(&frame.items[0],Item::Contact(c) if c.owner == Owner::Suppressed));
    let mut input = RawInput { focused: false, ..raw(vec![], 3.0) };
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    let mut input = raw(start(1, 3, p), 4.0);
    assert!(!n.process(&mut input, Routing::CanvasConsumer, canvas).blocked);
}

#[test]
fn ipad_touch_focus_loss_preserves_backend_primary_for_a_late_end() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut input = raw(start(1, 1, p), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    let mut input = RawInput { focused: false, ..raw(vec![Event::WindowFocused(false)], 2.0) };
    n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert_eq!(n.backend_primary, Some(1));
    let mut input = raw(vec![touch(1, 1, TouchPhase::End, p), button(false, p), Event::PointerGone], 3.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert!(frame.issues.contains(&Issue::MissingStart));
    assert!(!frame.issues.contains(&Issue::InvalidSuffix));
    assert!(input.events.is_empty());
    assert_eq!(n.backend_primary, None);
}

#[test]
fn ipad_touch_contact_capacity_is_bounded_and_suppresses_the_extra_start() {
    let mut n = Normalizer::default();
    let p = pos2(10.0, 20.0);
    let mut events = start(1, 1, p);
    events.extend((2..=65).map(|id| touch(1, id, TouchPhase::Start, p)));
    let mut input = raw(events, 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, canvas);
    assert_eq!(n.contacts.len(), MAX_CONTACTS);
    assert!(frame.blocked && frame.issues.contains(&Issue::Capacity));
    assert!(matches!(frame.items.last(), Some(Item::Contact(c)) if c.owner == Owner::Suppressed));
}

#[test]
fn ipad_touch_short_panel_cancel_resets_without_clicking_the_button() {
    let ctx = Context::default();
    let mut n = Normalizer::default();
    let mut p = Pos2::ZERO;
    ctx.run_ui(raw(vec![], 0.0), |ui| {
        p = ui.button("Panel action").rect.center();
    })
    .drop_without_applying_deltas();
    let mut input = raw(start(1, 9, p), 1.0);
    n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    let mut clicks = 0;
    ctx.run_ui(input, |ui| {
        if ui.button("Panel action").clicked() {
            clicks += 1;
        }
    })
    .drop_without_applying_deltas();
    let mut input = raw(vec![touch(1, 9, TouchPhase::Cancel, p), Event::PointerGone], 1.01);
    n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    ctx.stop_dragging();
    ctx.run_ui(input, |ui| {
        if ui.button("Panel action").clicked() {
            clicks += 1;
        }
    })
    .drop_without_applying_deltas();
    assert_eq!(clicks, 0, "Cancel must not become an accepted panel button click");
    assert!(!ctx.input(|i| i.pointer.primary_down()));
}

#[test]
fn ipad_touch_unsupported_nonfinite_panel_and_pointer_values_never_enter_egui() {
    let ctx = Context::default();
    let mut n = Normalizer::default();
    let p = pos2(f32::NAN, 20.0);
    let mut events = start(1, 9, p);
    events.push(Event::Text("Retain text".into()));
    let mut input = raw(events, 1.0);
    let frame = n.process(&mut input, Routing::CanvasConsumer, |_| Owner::Panel);
    assert!(frame.blocked && frame.issues.contains(&Issue::NonFinite));
    assert_eq!(input.events, vec![Event::Text("Retain text".into())]);
    run(&ctx, input);
    assert!(!ctx.input(|i| i.pointer.primary_down()));
}

#[test]
fn ipad_touch_contact_modifiers_follow_raw_order_within_one_frame() {
    let mut n = Normalizer::default();
    let p = pos2(20.0, 30.0);
    let mut events = vec![Event::ModifiersChanged(Modifiers::SHIFT)];
    events.extend(start(1, 9, p));
    events.push(Event::ModifiersChanged(Modifiers::ALT));
    events.push(touch(1, 9, TouchPhase::Move, p));
    events.push(Event::PointerMoved(p));
    let frame = n.process(&mut raw(events, 1.0), Routing::CanvasConsumer, canvas);
    assert!(frame.issues.is_empty());
    let contacts: Vec<_> = frame
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Contact(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(contacts.len(), 2);
    assert_eq!(contacts[0].modifiers, Modifiers::SHIFT);
    assert_eq!(contacts[1].modifiers, Modifiers::ALT);
}
