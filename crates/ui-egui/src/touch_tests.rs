use super::*;
use crate::touch_events::{Clock, Source};
use egui::pos2;
use figcraft_engine::tools::{PointerEvent, PointerKind};
use figcraft_engine::{PointerGestureToken, Session};
use serde_json::json;
use std::sync::Arc;

fn c(id: u64, phase: TouchPhase, x: f32, y: f32, time: f64, frame: u64, ordinal: usize) -> Contact {
    Contact {
        id: ContactId { device: 1, contact: id },
        source: Source::Unknown,
        phase,
        position: pos2(x, y),
        pressure: Some(0.7),
        modifiers: egui::Modifiers::NONE,
        time,
        clock: Clock::Frame,
        frame,
        ordinal,
        owner: Owner::Canvas { uid: 1, layout: 1 },
    }
}
fn apply(session: &mut Session, token: &mut Option<PointerGestureToken>, outputs: Vec<Output>) {
    for output in outputs {
        match output {
            Output::Begin(c) => {
                *token = Some(session.begin_pointer_gesture().unwrap());
                session
                    .with_pointer_gesture(token.as_ref().unwrap(), |s| {
                        s.pointer(&PointerEvent::at(PointerKind::Down, f64::from(c.position.x), f64::from(c.position.y), 1.0))
                    })
                    .unwrap();
            }
            Output::Move(c) => {
                session
                    .with_pointer_gesture(token.as_ref().unwrap(), |s| {
                        s.pointer(&PointerEvent::at(PointerKind::Move, f64::from(c.position.x), f64::from(c.position.y), 1.0))
                    })
                    .unwrap();
            }
            Output::Up(c) => {
                session
                    .with_pointer_gesture(token.as_ref().unwrap(), |s| {
                        s.pointer(&PointerEvent::at(PointerKind::Up, f64::from(c.position.x), f64::from(c.position.y), 1.0))
                    })
                    .unwrap();
            }
            Output::Finish => {
                session.finish_pointer_gesture(&token.take().unwrap()).unwrap();
            }
            Output::Cancel => {
                if let Some(t) = token.take() {
                    session.cancel_pointer_gesture(&t).unwrap();
                }
            }
            _ => {}
        }
    }
}
#[test]
fn ipad_pinch_cancels_pending_draw() {
    let mut g = Gesture::default();
    let mut s = Session::new();
    s.execute("tool.rectangle", &json!({})).unwrap();
    let before = s.ds().unwrap().doc.clone();
    let depth = s.ds().unwrap().history.undo.len();
    let mut token = None;
    apply(&mut s, &mut token, g.event(&c(1, TouchPhase::Start, 100.0, 100.0, 0.0, 1, 0), Policy::Drag));
    assert!(token.is_none());
    apply(&mut s, &mut token, g.event(&c(1, TouchPhase::Move, 160.0, 160.0, 0.1, 2, 0), Policy::Drag));
    assert!(!Arc::ptr_eq(&before, &s.ds().unwrap().doc));
    let output = g.event(&c(2, TouchPhase::Start, 200.0, 200.0, 0.2, 3, 0), Policy::Drag);
    assert!(matches!(output.as_slice(), [Output::Cancel, Output::NavigationStart { .. }]));
    apply(&mut s, &mut token, output);
    assert!(Arc::ptr_eq(&before, &s.ds().unwrap().doc));
    assert_eq!(s.ds().unwrap().history.undo.len(), depth);
    assert!(token.is_none());
    assert_eq!(g.phase(), Phase::Navigating);
}
#[test]
fn ipad_one_remaining_finger_does_not_draw() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0), Policy::Drag);
    g.event(&c(2, TouchPhase::Start, 20.0, 0.0, 0.1, 1, 1), Policy::Drag);
    assert_eq!(g.event(&c(2, TouchPhase::End, 20.0, 0.0, 0.2, 1, 2), Policy::Drag), vec![Output::NavigationEnd]);
    assert!(g.event(&c(1, TouchPhase::Move, 100.0, 0.0, 0.3, 1, 3), Policy::Drag).is_empty());
    assert_eq!(g.phase(), Phase::SuppressedUntilAllReleased);
    g.event(&c(1, TouchPhase::End, 100.0, 0.0, 0.4, 1, 4), Policy::Drag);
    assert_eq!(g.phase(), Phase::Idle);
}
#[test]
fn ipad_touch_threshold_is_strict_and_preserves_the_original_down() {
    let mut g = Gesture::default();
    let start = c(1, TouchPhase::Start, 10.0, 10.0, 0.0, 1, 0);
    assert!(g.event(&start, Policy::Drag).is_empty());
    assert!(g.event(&c(1, TouchPhase::Move, 16.0, 10.0, 0.1, 1, 1), Policy::Drag).is_empty());
    let outputs = g.event(&c(1, TouchPhase::Move, 16.01, 10.0, 0.2, 1, 2), Policy::Drag);
    assert!(matches!(&outputs[..],[Output::Begin(a),Output::Move(_)] if a==&start));
}
#[test]
fn ipad_touch_tap_accepts_once_and_undoes_once() {
    let mut g = Gesture::default();
    let mut s = Session::new();
    s.execute("tool.rectangle", &json!({})).unwrap();
    let before = s.ds().unwrap().doc.clone();
    let depth = s.ds().unwrap().history.undo.len();
    let mut token = None;
    apply(&mut s, &mut token, g.event(&c(1, TouchPhase::Start, 100.0, 100.0, 0.0, 1, 0), Policy::Drag));
    let outputs = g.event(&c(1, TouchPhase::End, 100.0, 100.0, 0.1, 1, 1), Policy::Drag);
    assert!(matches!(&outputs[..], [Output::Begin(_), Output::Up(_), Output::Finish]));
    apply(&mut s, &mut token, outputs);
    assert_eq!(s.ds().unwrap().history.undo.len(), depth + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap(), before.as_ref());
}
#[test]
fn ipad_touch_long_press_fires_once_and_never_begins_an_edit() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 10.0, 10.0, 0.0, 1, 0), Policy::Drag);
    assert!(g.tick(0.49, 1).is_empty());
    assert!(matches!(&g.tick(0.5, 2)[..], [Output::LongPress(_)]));
    assert!(g.tick(0.6, 3).is_empty());
    assert!(g.event(&c(1, TouchPhase::End, 10.0, 10.0, 0.7, 4, 0), Policy::Drag).is_empty());
    assert_eq!(g.phase(), Phase::Idle);
}
#[test]
fn ipad_touch_equal_deadline_raw_event_runs_before_the_timer() {
    for phase in [TouchPhase::Move, TouchPhase::End] {
        let mut g = Gesture::default();
        g.event(&c(1, TouchPhase::Start, 10.0, 10.0, 0.0, 1, 0), Policy::Drag);
        let output = g.event(&c(1, phase, if phase == TouchPhase::Move { 20.0 } else { 10.0 }, 10.0, 0.5, 2, 0), Policy::Drag);
        assert!(matches!(output.first(), Some(Output::Begin(_))));
        assert!(g.tick(0.5, 2).is_empty());
    }
}
#[test]
fn ipad_touch_overdue_long_press_precedes_a_later_end() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0), Policy::Drag);
    assert!(matches!(&g.event(&c(1, TouchPhase::End, 0.0, 0.0, 0.501, 2, 0), Policy::Drag)[..], [Output::LongPress(_)]));
}
#[test]
fn ipad_panel_touch_does_not_edit() {
    let mut g = Gesture::default();
    let mut panel = c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0);
    panel.owner = Owner::Panel;
    assert!(g.event(&panel, Policy::Drag).is_empty());
    assert_eq!(g.phase(), Phase::Idle);
}
#[test]
fn ipad_touch_third_finger_cannot_reassign_a_navigation_pair() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0), Policy::Drag);
    g.event(&c(2, TouchPhase::Start, 20.0, 0.0, 0.1, 1, 1), Policy::Drag);
    assert_eq!(g.event(&c(3, TouchPhase::Start, 40.0, 0.0, 0.2, 1, 2), Policy::Drag), vec![Output::NavigationEnd]);
    assert!(g.event(&c(3, TouchPhase::End, 40.0, 0.0, 0.3, 1, 3), Policy::Drag).is_empty());
    assert!(g.event(&c(2, TouchPhase::Move, 30.0, 0.0, 0.4, 1, 4), Policy::Drag).is_empty());
    assert_eq!(g.phase(), Phase::SuppressedUntilAllReleased);
}
#[test]
fn ipad_touch_commit_only_intent_is_cancelled_by_movement_or_navigation() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0), Policy::TapOnly);
    assert!(g.event(&c(1, TouchPhase::Move, 20.0, 0.0, 0.1, 1, 1), Policy::TapOnly).is_empty());
    assert!(g.tick(0.6, 2).is_empty());
    let output = g.event(&c(2, TouchPhase::Start, 40.0, 0.0, 0.7, 3, 0), Policy::TapOnly);
    assert!(matches!(output.as_slice(), [Output::NavigationStart { .. }]));
}
#[test]
fn ipad_touch_external_interrupt_suppresses_the_held_contact_until_end() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0), Policy::Drag);
    g.event(&c(1, TouchPhase::Move, 10.0, 0.0, 0.1, 1, 1), Policy::Drag);
    assert_eq!(g.interrupt(), vec![Output::Cancel]);
    assert!(g.event(&c(1, TouchPhase::Move, 100.0, 0.0, 0.2, 1, 2), Policy::Drag).is_empty());
    assert!(g.event(&c(1, TouchPhase::End, 100.0, 0.0, 0.3, 1, 3), Policy::Drag).is_empty());
    assert_eq!(g.phase(), Phase::Idle);
}
#[test]
fn ipad_touch_bad_clock_sequence_and_capacity_are_explicit_and_bounded() {
    let mut g = Gesture::default();
    g.event(&c(1, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 0), Policy::Drag);
    let output = g.event(&c(1, TouchPhase::Move, 10.0, 0.0, f64::NAN, 1, 1), Policy::Drag);
    assert!(output.contains(&Output::Unsupported(Error::Clock)));
    g.reset();
    for id in 0..64 {
        g.event(&c(id, TouchPhase::Start, 0.0, 0.0, 0.0, 1, id as usize), Policy::Drag);
    }
    assert!(g.event(&c(64, TouchPhase::Start, 0.0, 0.0, 0.0, 1, 64), Policy::Drag).contains(&Output::Unsupported(Error::Capacity)));
    assert_eq!(g.contacts.len(), 64);
}
#[test]
fn ipad_pinch_anchor_stays_fixed() {
    let mut view = Session::new().ds().unwrap().view.clone();
    view.vw = 800.0;
    view.vh = 600.0;
    view.cx = 60.0;
    view.cy = 80.0;
    view.zoom = 2.0;
    let origin = pos2(200.0, 100.0);
    let a = pos2(400.0, 300.0);
    let b = pos2(600.0, 300.0);
    let before = view.screen_to_page(500.0 - 200.0, 300.0 - 100.0);
    let nav = Navigation::begin(a, b, &view, origin).unwrap();
    let (cx, cy, zoom) = nav.transform(pos2(450.0, 340.0), pos2(850.0, 340.0)).unwrap();
    view.cx = cx;
    view.cy = cy;
    view.zoom = zoom;
    let after = view.screen_to_page(650.0 - 200.0, 340.0 - 100.0);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
    assert_eq!(zoom, 4.0);
}
#[test]
fn ipad_pinch_waits_for_a_valid_baseline_and_clamps_zoom() {
    let mut view = Session::new().ds().unwrap().view.clone();
    view.vw = 800.0;
    view.vh = 600.0;
    assert!(Navigation::begin(pos2(0.0, 0.0), pos2(0.5, 0.0), &view, Pos2::ZERO).is_none());
    let nav = Navigation::begin(pos2(0.0, 0.0), pos2(1.0, 0.0), &view, Pos2::ZERO).unwrap();
    assert_eq!(nav.transform(pos2(0.0, 0.0), pos2(1.0e6, 0.0)).unwrap().2, ViewState::MAX_ZOOM);
    assert_eq!(nav.transform(Pos2::ZERO, Pos2::ZERO).unwrap().2, ViewState::MIN_ZOOM);
    assert!(nav.transform(pos2(f32::NAN, 0.0), Pos2::ZERO).is_none());
}

#[test]
fn ipad_touch_end_supplies_a_final_move_before_up() {
    let mut g = Gesture::default();
    let mut s = Session::new();
    s.execute("tool.rectangle", &json!({})).unwrap();
    let mut token = None;
    apply(&mut s, &mut token, g.event(&c(1, TouchPhase::Start, 100.0, 100.0, 0.0, 1, 0), Policy::Drag));
    let outputs = g.event(&c(1, TouchPhase::End, 200.0, 200.0, 0.1, 1, 1), Policy::Drag);
    assert!(matches!(&outputs[..], [Output::Begin(_), Output::Move(_), Output::Up(_), Output::Finish]));
    apply(&mut s, &mut token, outputs);
    let node = s.doc().unwrap().node(s.selection()[0]).unwrap();
    assert_eq!((node.width, node.height), (100.0, 100.0));
}

#[test]
fn ipad_touch_value_pointer_cancels_without_legacy_egui_down() {
    use crate::{FigApp, NoServices};
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |_| {}).drop_without_applying_deltas();
    let mut app = FigApp::new(Session::new(), Box::new(NoServices));
    app.canvas.rect = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
    app.run("tool.rectangle", json!({})).unwrap();
    let before = app.session.ds().unwrap().doc.clone();
    let depth = app.session.ds().unwrap().history.undo.len();
    app.begin_canvas_pointer(&ctx).unwrap();
    crate::canvas::pointer_value(&mut app, &ctx, PointerKind::Down, pos2(100.0, 100.0), egui::Modifiers::NONE, None).unwrap();
    crate::canvas::pointer_value(&mut app, &ctx, PointerKind::Move, pos2(200.0, 200.0), egui::Modifiers::NONE, None).unwrap();
    assert!(!Arc::ptr_eq(&before, &app.session.ds().unwrap().doc));
    assert!(!ctx.input(|i| i.pointer.primary_down()));
    app.cancel_canvas_pointer();
    assert!(Arc::ptr_eq(&before, &app.session.ds().unwrap().doc));
    assert_eq!(app.session.ds().unwrap().history.undo.len(), depth);
    assert!(crate::canvas::pointer_value(&mut app, &ctx, PointerKind::Up, pos2(200.0, 200.0), egui::Modifiers::NONE, None).is_err());
}
