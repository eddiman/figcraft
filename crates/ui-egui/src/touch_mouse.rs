//! Physical values proven independent by the T34 normalizer. This module does
//! not install a native route or change the Context's aggregated input state.
use egui::{Context, Event, InputState, Key, Modifiers, Pos2, RawInput, Vec2};

use crate::{
    FigApp,
    touch_events::{Frame, Item, MAX_ITEMS, Owner},
};
use figcraft_engine::tools::PointerKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    InvalidClock,
    NonFinite,
    Capacity,
    MissingValue,
}

/// A physical pointer value with its own source state, independent of panel
/// touch's synthetic pointer. Click count comes from the pinned egui code.
#[derive(Clone, Debug)]
pub struct Sample {
    pub ordinal: usize,
    pub event: Event,
    pub modifiers: Modifiers,
    pub space: bool,
    pub zoom_key: bool,
    pub any_down: bool,
    pub clicks: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct Navigation {
    /// First physical wheel/zoom value in this batch, or None for smoothing from
    /// the preceding batch. Interpret once, using egui's whole-frame result.
    pub ordinal: Option<usize>,
    pub position: Option<Pos2>,
    pub scroll: Vec2,
    pub zoom: f32,
    pub modifiers: Modifiers,
}
#[derive(Default)]
pub struct PhysicalFrame {
    pub position: Option<Pos2>,
    pub samples: Vec<Sample>,
    pub navigation: Option<Navigation>,
    pub reset: bool,
}
#[derive(Default)]
pub struct Physical {
    state: InputState,
    blocked: bool,
    frame: Option<u64>,
}
fn accepted(event: &Event) -> bool {
    matches!(
        event,
        Event::PointerMoved(_)
            | Event::PointerButton { .. }
            | Event::PointerGone
            | Event::MouseMoved(_)
            | Event::ModifiersChanged(_)
            | Event::Key { key: Key::Space | Key::Z, .. }
            | Event::MouseWheel { .. }
            | Event::Zoom(_)
            | Event::Rotate(_)
            | Event::WindowFocused(_)
    )
}
pub(crate) fn finite(event: &Event) -> bool {
    match event {
        Event::PointerMoved(p) | Event::PointerButton { pos: p, .. } => p.x.is_finite() && p.y.is_finite(),
        Event::MouseMoved(d) | Event::MouseWheel { delta: d, .. } => d.is_finite(),
        Event::Zoom(z) => z.is_finite() && *z > 0.0,
        Event::Rotate(r) => r.is_finite(),
        _ => true,
    }
}
fn input(raw: &RawInput, time: f64, events: Vec<Event>) -> RawInput {
    // Deliberately construct rather than cloning RawInput. This mirror never
    // owns text, IME, dropped files or platform file handles.
    RawInput {
        viewport_id: raw.viewport_id,
        viewports: raw.viewports.clone(),
        screen_rect: raw.screen_rect,
        safe_area_insets: raw.safe_area_insets,
        time: Some(time),
        predicted_dt: raw.predicted_dt,
        focused: raw.focused,
        events,
        ..Default::default()
    }
}
/// Refuse bad metadata without passing nonfinite geometry into egui. The
/// boundary has already blocked editing for this source frame; None preserves
/// egui's preceding geometry until a supported frame arrives.
pub(crate) fn sanitize_metadata(raw: &mut RawInput) {
    if !raw.predicted_dt.is_finite() || raw.predicted_dt < 0.0 {
        raw.predicted_dt = 1.0 / 60.0;
    }
    if raw.screen_rect.is_some_and(|rect| !rect.is_finite()) {
        raw.screen_rect = None;
    }
    if raw.safe_area_insets.is_some_and(|insets| ![insets.0.top, insets.0.bottom, insets.0.left, insets.0.right].iter().all(|v| v.is_finite())) {
        raw.safe_area_insets = None;
    }
    if let Some(info) = raw.viewports.get_mut(&raw.viewport_id)
        && info.native_pixels_per_point.is_some_and(|scale| !scale.is_finite() || scale <= 0.0)
    {
        info.native_pixels_per_point = None;
    }
}

impl Physical {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Call immediately after Normalizer, before Boundary consumes keyboard
    /// events. The indices in Frame cease to identify raw values after that.
    pub fn prepare(&mut self, ctx: &Context, raw: &RawInput, frame: &Frame) -> Result<PhysicalFrame, Error> {
        let result = self.prepare_inner(ctx, raw, frame);
        if result.is_err() {
            self.blocked = true;
        }
        result
    }
    fn prepare_inner(&mut self, ctx: &Context, raw: &RawInput, frame: &Frame) -> Result<PhysicalFrame, Error> {
        if !raw.focused || raw.events.iter().any(|event| matches!(event, Event::WindowFocused(false))) {
            self.reset();
            return Ok(PhysicalFrame { reset: true, ..Default::default() });
        }
        if self.blocked || frame.blocked {
            return Err(Error::Unsupported);
        }
        if !raw.predicted_dt.is_finite() || raw.predicted_dt < 0.0 {
            return Err(Error::InvalidClock);
        }
        if raw.screen_rect.is_some_and(|rect| !rect.is_finite())
            || raw
                .safe_area_insets
                .is_some_and(|insets| ![insets.0.top, insets.0.bottom, insets.0.left, insets.0.right].iter().all(|v| v.is_finite()))
            || raw
                .viewports
                .get(&raw.viewport_id)
                .and_then(|info| info.native_pixels_per_point)
                .is_some_and(|scale| !scale.is_finite() || scale <= 0.0)
        {
            return Err(Error::NonFinite);
        }
        let (time, _, number) = frame.timestamp.ok_or(Error::InvalidClock)?;
        if !time.is_finite() || self.frame.is_some_and(|n| number <= n) || time < self.state.time {
            return Err(Error::InvalidClock);
        }
        if frame.items.len() > MAX_ITEMS || raw.events.len() > MAX_ITEMS {
            return Err(Error::Capacity);
        }
        let mut values = Vec::new();
        for item in &frame.items {
            if let Item::Input { ordinal, index, .. } = item {
                let event = raw.events.get(*index).ok_or(Error::MissingValue)?;
                if accepted(event) {
                    if !finite(event) {
                        return Err(Error::NonFinite);
                    }
                    values.push((*ordinal, event.clone()));
                }
            }
        }
        let options = ctx.options(|options| options.input_options);
        let repaint = ctx.requested_repaint_last_pass();
        let ppp = ctx.pixels_per_point();
        // Only the whole-frame state supplies wheel smoothing. The scratch state
        // below is used for per-value click/key state, never wheel interpretation.
        let next = self.state.clone().begin_pass(input(raw, time, values.iter().map(|(_, e)| e.clone()).collect()), repaint, ppp, options);
        let mut scratch = self.state.clone();
        let mut samples = Vec::new();
        for (ordinal, event) in &values {
            if matches!(event, Event::MouseWheel { .. } | Event::Zoom(_) | Event::Rotate(_)) {
                continue;
            }
            scratch = scratch.begin_pass(input(raw, time, vec![event.clone()]), repaint, ppp, options);
            if matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone | Event::MouseMoved(_)) {
                let clicks = match event {
                    Event::PointerButton { button, pressed: false, .. } if scratch.pointer.button_triple_clicked(*button) => 3,
                    Event::PointerButton { button, pressed: false, .. } if scratch.pointer.button_double_clicked(*button) => 2,
                    Event::PointerButton { button, pressed: false, .. } if scratch.pointer.button_clicked(*button) => 1,
                    _ => 0,
                };
                // Modifiers attached to a button retain their event-local value.
                let modifiers = match event {
                    Event::PointerButton { modifiers, .. } => *modifiers,
                    _ => scratch.modifiers,
                };
                samples.push(Sample {
                    ordinal: *ordinal,
                    event: event.clone(),
                    modifiers,
                    space: scratch.key_down(Key::Space),
                    zoom_key: scratch.key_down(Key::Z),
                    any_down: scratch.pointer.any_down(),
                    clicks,
                });
            }
        }
        let zoom = next.zoom_delta();
        let scroll = next.smooth_scroll_delta;
        if !zoom.is_finite() || zoom <= 0.0 || !scroll.is_finite() {
            return Err(Error::NonFinite);
        }
        let navigation = ((zoom - 1.0).abs() > 1e-4 || scroll != Vec2::ZERO).then(|| Navigation {
            ordinal: values
                .iter()
                .find_map(|(ordinal, event)| matches!(event, Event::MouseWheel { .. } | Event::Zoom(_) | Event::Rotate(_)).then_some(*ordinal)),
            position: next.pointer.hover_pos(),
            scroll,
            zoom,
            modifiers: next.modifiers,
        });
        let position = next.pointer.hover_pos();
        self.state = next;
        self.frame = Some(number);
        Ok(PhysicalFrame { position, samples, navigation, reset: false })
    }
}

#[derive(Clone, Copy, Default)]
enum State {
    #[default]
    Idle,
    Edit,
    Pan {
        button: egui::PointerButton,
        from: Pos2,
        cx: f64,
        cy: f64,
    },
    Zoom {
        from: Pos2,
    },
    Secondary,
    Panel,
    Suppressed,
}
/// Explicit physical pointer owner. The caller dispatches Sample values in
/// ordinal order and cancels before external edits; this API does not install
/// the native consumer or consume egui panel events.
#[derive(Default)]
pub struct Mouse {
    state: State,
    any_down: bool,
}
fn disabled(message: &str) -> figcraft_engine::EngineError {
    figcraft_engine::EngineError::Disabled("pointer.mouse".into(), message.into())
}
impl Mouse {
    pub fn any_down(&self) -> bool {
        self.any_down
    }
    pub fn active(&self) -> bool {
        matches!(self.state, State::Edit | State::Pan { .. } | State::Zoom { .. } | State::Secondary)
    }
    pub fn interrupt(&mut self, app: &mut FigApp) {
        app.cancel_canvas_pointer();
        app.canvas.value_navigation(false, None, None);
        self.state = if self.any_down { State::Suppressed } else { State::Idle };
    }
    pub fn reset(&mut self, app: &mut FigApp) {
        self.interrupt(app);
        *self = Self::default();
    }
    /// `hit` classifies a fresh press/hover. A held pointer retains ownership
    /// when crossing a panel. `current` validates source/layout before dispatch.
    pub fn sample(&mut self, app: &mut FigApp, ctx: &Context, sample: &Sample, hit: Owner, current: bool) -> figcraft_engine::Result<bool> {
        self.any_down = sample.any_down;
        if !finite(&sample.event) {
            self.interrupt(app);
            return Err(disabled("nonfinite physical pointer value"));
        }
        let owned = matches!(self.state, State::Edit | State::Pan { .. } | State::Zoom { .. });
        app.canvas_pointer_preflight(ctx);
        if !current || (owned && app.canvas_pointer.is_none()) {
            self.interrupt(app);
            return Ok(false);
        }
        let result = self.dispatch(app, ctx, sample, hit);
        if result.is_err() {
            self.interrupt(app);
        }
        result
    }
    fn dispatch(&mut self, app: &mut FigApp, ctx: &Context, s: &Sample, hit: Owner) -> figcraft_engine::Result<bool> {
        if matches!(self.state, State::Suppressed) {
            if !s.any_down {
                self.state = State::Idle;
            }
            return Ok(false);
        }
        let on_canvas = matches!(hit, Owner::Canvas { .. });
        match &s.event {
            Event::PointerButton { pos, button, pressed: true, .. } => {
                if !matches!(self.state, State::Idle) {
                    // Two independently pressed mouse buttons cannot transfer
                    // a held edit into a different tool or navigation owner.
                    self.interrupt(app);
                    return Ok(false);
                }
                if !on_canvas {
                    self.state = State::Panel;
                    return Ok(false);
                }
                if *button == egui::PointerButton::Secondary {
                    self.state = State::Secondary;
                    return Ok(true);
                }
                if !matches!(button, egui::PointerButton::Primary | egui::PointerButton::Middle) {
                    self.state = State::Panel;
                    return Ok(false);
                }
                let typing = app.session.ds().is_ok_and(|ds| ds.text_edit.is_some()) || crate::a11y::field_focused(ctx);
                app.begin_canvas_pointer(ctx)?;
                app.take_canvas_keyboard(ctx);
                if *button == egui::PointerButton::Middle || (s.space && !typing) {
                    let view = &app.session.ds()?.view;
                    self.state = State::Pan { button: *button, from: *pos, cx: view.cx, cy: view.cy };
                    app.canvas.value_navigation(true, None, None);
                } else if s.zoom_key && !s.modifiers.command && !typing {
                    self.state = State::Zoom { from: *pos };
                    app.canvas.value_navigation(false, Some(s.modifiers.alt), Some(*pos));
                } else {
                    self.state = State::Edit;
                    crate::canvas::pointer_value(app, ctx, PointerKind::Down, *pos, s.modifiers, None)?;
                }
                Ok(true)
            }
            Event::PointerMoved(pos) => match self.state {
                State::Edit => {
                    crate::canvas::pointer_value(app, ctx, PointerKind::Move, *pos, s.modifiers, None)?;
                    Ok(true)
                }
                State::Pan { from, cx, cy, .. } => {
                    Self::pan(app, *pos, from, cx, cy)?;
                    Ok(true)
                }
                State::Zoom { from } => {
                    app.canvas.value_navigation(false, Some(s.modifiers.alt), Some(from));
                    Ok(true)
                }
                State::Idle if on_canvas && !s.any_down => {
                    crate::canvas::pointer_value(app, ctx, PointerKind::Hover, *pos, s.modifiers, None)?;
                    Ok(true)
                }
                _ => Ok(false),
            },
            Event::PointerButton { pos, button, pressed: false, .. } => {
                let state = self.state;
                match state {
                    State::Edit if *button == egui::PointerButton::Primary => {
                        let kind = if s.clicks == 2 && on_canvas { PointerKind::DoubleClick } else { PointerKind::Up };
                        crate::canvas::pointer_value(app, ctx, kind, *pos, s.modifiers, None)?;
                        if s.clicks == 3 && on_canvas {
                            crate::shortcuts::triple_click_value(app, *pos);
                        }
                        if app.canvas_pointer.is_none() {
                            return Err(disabled("physical release lost its enrolled owner"));
                        }
                        app.finish_canvas_pointer()?;
                    }
                    State::Pan { button: held, from, cx, cy } if *button == held => {
                        Self::pan(app, *pos, from, cx, cy)?;
                        app.finish_canvas_pointer()?;
                    }
                    State::Zoom { from } if *button == egui::PointerButton::Primary => {
                        let area = egui::Rect::from_two_pos(from, *pos);
                        let (a, b) = (app.to_page(area.min), app.to_page(area.max));
                        let params = if area.width() > 4.0 && area.height() > 4.0 {
                            serde_json::json!({"x":a.x,"y":a.y,"width":b.x-a.x,"height":b.y-a.y})
                        } else {
                            let p = app.to_page(*pos);
                            serde_json::json!({"x":p.x,"y":p.y,"out":s.modifiers.alt})
                        };
                        app.pointer_session(|session| session.execute("view.zoomAt", &params))?;
                        app.finish_canvas_pointer()?;
                    }
                    State::Secondary if *button == egui::PointerButton::Secondary && s.clicks > 0 && on_canvas => {
                        crate::canvas::open_context_menu(app, *pos, s.modifiers.command);
                    }
                    _ => {}
                }
                if !s.any_down {
                    self.state = State::Idle;
                    app.canvas.value_navigation(false, None, None);
                }
                Ok(matches!(state, State::Edit | State::Pan { .. } | State::Zoom { .. } | State::Secondary))
            }
            Event::PointerGone => {
                if !s.any_down && matches!(self.state, State::Idle) {
                    app.session.vector_pointer_left();
                }
                Ok(self.active())
            }
            Event::MouseMoved(_) => Ok(self.active()),
            _ => Ok(false),
        }
    }
    fn pan(app: &mut FigApp, pos: Pos2, from: Pos2, cx: f64, cy: f64) -> figcraft_engine::Result<()> {
        app.pointer_session(|session| {
            let ds = session.ds_mut()?;
            let z = ds.view.zoom.max(1e-6);
            ds.view.cx = cx - f64::from(pos.x - from.x) / z;
            ds.view.cy = cy - f64::from(pos.y - from.y) / z;
            ds.revision = ds.revision.saturating_add(1);
            Ok(())
        })
    }
    /// Apply the pinned whole-frame physical wheel/trackpad result exactly once.
    /// The host cancels both editing sources before invoking this method.
    pub fn navigate(app: &mut FigApp, navigation: Navigation, on_canvas: bool) -> figcraft_engine::Result<()> {
        if app.canvas_pointer.is_some() {
            app.cancel_canvas_pointer();
            return Err(disabled("physical navigation requires cancellation before dispatch"));
        }
        let Some(pos) = navigation.position.filter(|_| on_canvas) else { return Ok(()) };
        if !pos.x.is_finite() || !pos.y.is_finite() || !navigation.zoom.is_finite() || navigation.zoom <= 0.0 || !navigation.scroll.is_finite() {
            return Err(disabled("nonfinite physical navigation value"));
        }
        if (navigation.zoom - 1.0).abs() > 1e-4 {
            let anchor = app.to_page(pos);
            app.pointer_session(|session| {
                let ds = session.ds_mut()?;
                ds.view.set_zoom_at(ds.view.zoom * f64::from(navigation.zoom), (anchor.x, anchor.y));
                ds.revision = ds.revision.saturating_add(1);
                Ok(())
            })
        } else if navigation.scroll != Vec2::ZERO {
            app.pointer_session(|session| {
                let ds = session.ds_mut()?;
                ds.view.scroll(f64::from(navigation.scroll.x), f64::from(navigation.scroll.y), navigation.modifiers.shift);
                ds.revision = ds.revision.saturating_add(1);
                Ok(())
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "touch_mouse_tests.rs"]
mod tests;
