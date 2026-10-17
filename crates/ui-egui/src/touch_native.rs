//! Completed layout ownership for the native consumer. Capturing these values
//! does not activate input; the app still uses its passive T34 observer.
use egui::{Context, LayerId, Pos2, Rect};

use figcraft_engine::tools::{PointerKind, ToolId};

use crate::{
    FigApp,
    ordered_input::{CanvasConsumer, Interruption, Target},
    touch::{Gesture, Navigation, Output, Phase, Policy},
    touch_events::{Clock, Contact, Owner},
};

#[derive(Clone)]
pub struct Layout {
    pub target: Target,
    canvas: Rect,
    layer: LayerId,
    toolbar: Option<Rect>,
    edge_grab: f32,
}
impl Layout {
    /// Call only after the frame has drawn its canvas and panels. Counter
    /// exhaustion refuses a map instead of reusing a held contact's layout ID.
    pub fn capture(app: &FigApp, ctx: &Context, previous: Option<&Self>, generation: &mut Option<u64>) -> Option<Self> {
        let source = app.session.ds().ok().map(|ds| (ds.uid, ds.page));
        if app.canvas.input_source.is_none() || source != app.canvas.input_source {
            return None;
        }
        let target = Target::capture(app, ctx, 0)?;
        let layer = app.canvas.input_layer?;
        let regions = app.ui.nav.input_regions();
        if regions.iter().flatten().any(|r| !r.is_finite()) {
            return None;
        }
        let same = previous.is_some_and(|old| old.layer == layer && target.same_geometry(&old.target));
        let epoch = if same {
            let Owner::Canvas { layout, .. } = previous?.target.owner() else { return None };
            if *generation != Some(layout) {
                return None;
            }
            layout
        } else {
            let next = generation.and_then(|n| n.checked_add(1));
            *generation = next;
            next?
        };
        let hidden = app.session.ds().is_ok_and(|ds| ds.view.ui_hidden);
        let edge_grab = if hidden { 0.0 } else { ctx.style_of(ctx.theme()).interaction.resize_grab_radius_side };
        Some(Self { target: target.with_layout(epoch), canvas: app.canvas.rect, layer, toolbar: regions[0], edge_grab })
    }
    pub fn classify(&self, ctx: &Context, pos: Pos2) -> Owner {
        if !pos.x.is_finite() || !pos.y.is_finite() {
            return Owner::Suppressed;
        }
        if !self.canvas.contains(pos)
            || self.toolbar.is_some_and(|r| r.contains(pos))
            || pos.x - self.canvas.min.x < self.edge_grab
            || self.canvas.max.x - pos.x < self.edge_grab
            || ctx.layer_id_at(pos).is_some_and(|layer| layer != self.layer)
        {
            Owner::Panel
        } else {
            self.target.owner()
        }
    }
}

/// Finger-only host consumer. The native shell must provide physical input and
/// focus-reset policy before enabling this consumer in its raw input hook.
#[derive(Default)]
pub struct Finger {
    gesture: Gesture,
    navigation: Option<Navigation>,
}
impl Finger {
    pub fn phase(&self) -> Phase {
        self.gesture.phase()
    }
    /// The producer resets its identities on focus loss, so the consumer must
    /// reset too. Ordinary layout/keyboard interruption retains held identities.
    pub fn reset(&mut self, app: &mut FigApp, ctx: &Context) {
        let outputs = self.gesture.reset();
        self.apply(app, ctx, outputs);
        app.cancel_canvas_pointer();
        self.navigation = None;
    }
    fn policy(app: &FigApp, position: Pos2) -> Policy {
        match app.session.tool.tool {
            ToolId::Eyedropper => Policy::TapOnly,
            ToolId::Comment if !crate::comments::pointer_hit(app, position) => Policy::TapOnly,
            _ => Policy::Drag,
        }
    }
    fn baseline(app: &FigApp, a: Pos2, b: Pos2) -> Option<Navigation> {
        Navigation::begin(a, b, &app.session.ds().ok()?.view, app.canvas.rect.min)
    }
    fn apply(&mut self, app: &mut FigApp, ctx: &Context, outputs: Vec<Output>) {
        for output in outputs {
            let result = match output {
                Output::Begin(contact) => app
                    .begin_canvas_pointer(ctx)
                    .and_then(|()| crate::canvas::pointer_value(app, ctx, PointerKind::Down, contact.position, contact.modifiers, contact.pressure)),
                Output::Move(contact) => {
                    crate::canvas::pointer_value(app, ctx, PointerKind::Move, contact.position, contact.modifiers, contact.pressure)
                }
                Output::Up(contact) => crate::canvas::pointer_value(app, ctx, PointerKind::Up, contact.position, contact.modifiers, contact.pressure),
                Output::Finish => app.finish_canvas_pointer().map(|_| ()),
                Output::Cancel => {
                    app.cancel_canvas_pointer();
                    Ok(())
                }
                Output::LongPress(contact) => {
                    crate::canvas::open_context_menu(app, contact.position, contact.modifiers.command);
                    Ok(())
                }
                Output::NavigationStart { a, b } => {
                    // Cancel restores the pre-Down view, including Hand-tool pan,
                    // before the pair establishes its page anchor.
                    app.cancel_canvas_pointer();
                    self.navigation = Self::baseline(app, a, b);
                    Ok(())
                }
                Output::Navigate { a, b } => {
                    if let Some(baseline) = self.navigation {
                        if let Some((cx, cy, zoom)) = baseline.transform(a, b) {
                            app.pointer_session(|session| {
                                let ds = session.ds_mut()?;
                                ds.view.cx = cx;
                                ds.view.cy = cy;
                                ds.view.zoom = zoom;
                                ds.revision = ds.revision.saturating_add(1);
                                Ok(())
                            })
                        } else {
                            Err(figcraft_engine::EngineError::Disabled("pointer.navigation".into(), "nonfinite navigation transform".into()))
                        }
                    } else {
                        self.navigation = Self::baseline(app, a, b);
                        Ok(())
                    }
                }
                Output::NavigationEnd => {
                    self.navigation = None;
                    Ok(())
                }
                Output::Unsupported(error) => {
                    app.cancel_canvas_pointer();
                    self.navigation = None;
                    Err(figcraft_engine::EngineError::Disabled("pointer.gesture".into(), format!("unsupported finger input: {error:?}")))
                }
            };
            if let Err(error) = result {
                // Never dispatch a later Move/Up after a failed Begin or helper.
                app.cancel_canvas_pointer();
                self.gesture.interrupt();
                self.navigation = None;
                app.toast(error.to_string());
                break;
            }
        }
    }
}
impl CanvasConsumer for Finger {
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

    fn contact(&mut self, app: &mut FigApp, ctx: &Context, contact: &Contact) {
        let outputs = self.gesture.event(contact, Self::policy(app, contact.position));
        self.apply(app, ctx, outputs);
    }
    fn interrupt(&mut self, app: &mut FigApp, ctx: &Context, _: Interruption) {
        let outputs = self.gesture.interrupt();
        self.apply(app, ctx, outputs);
        self.navigation = None;
    }
    fn producer_reset(&mut self, app: &mut FigApp, ctx: &Context) {
        self.reset(app, ctx);
    }
    fn finish_frame(&mut self, app: &mut FigApp, ctx: &Context, timestamp: Option<(f64, Clock, u64)>) {
        if let Some((time, _, frame)) = timestamp {
            let outputs = self.gesture.tick(time, frame);
            self.apply(app, ctx, outputs);
            if let Some(deadline) = self.gesture.deadline() {
                // Stay in the producer's clock domain. egui's context still has
                // the preceding frame's time during raw_input_hook.
                ctx.request_repaint_after(std::time::Duration::from_secs_f64((deadline - time).clamp(0.0, crate::touch::LONG_PRESS_SECS)));
            }
        }
    }
}

/// Ordered finger and physical values, still opt-in until the native Shell
/// installs this policy. Context pointer aggregates cannot edit this canvas.
#[derive(Default)]
pub struct Native {
    boundary: crate::ordered_input::Boundary,
    consumer: Combined,
    generation: Option<u64>,
    initialized: bool,
}
impl Native {
    /// Capture only after FigApp has completed drawing this window.
    pub fn complete_frame(&mut self, app: &FigApp, ctx: &Context) {
        if !self.initialized {
            self.generation = Some(0);
            self.initialized = true;
        }
        self.consumer.layout = Layout::capture(app, ctx, self.consumer.layout.as_ref(), &mut self.generation);
    }
    pub fn process(&mut self, app: &mut FigApp, ctx: &Context, raw: &mut egui::RawInput) -> crate::touch_events::Frame {
        let layout = self.consumer.layout.clone();
        self.boundary.process(
            app,
            ctx,
            raw,
            layout.as_ref().map(|layout| &layout.target),
            |p| layout.as_ref().map_or(Owner::Panel, |layout| layout.classify(ctx, p)),
            &mut self.consumer,
        )
    }
    pub fn phase(&self) -> Phase {
        self.consumer.finger.phase()
    }
}
#[derive(Default)]
struct Combined {
    finger: Finger,
    mouse: crate::touch_mouse::Mouse,
    physical: crate::touch_mouse::Physical,
    frame: Option<crate::touch_mouse::PhysicalFrame>,
    layout: Option<Layout>,
}
impl Combined {
    fn hit(&self, ctx: &Context, position: Pos2) -> Owner {
        self.layout.as_ref().map_or(Owner::Suppressed, |layout| layout.classify(ctx, position))
    }
    fn navigation(&mut self, app: &mut FigApp, ctx: &Context, navigation: crate::touch_mouse::Navigation, current: bool) -> bool {
        self.finger.interrupt(app, ctx, Interruption::ForwardedPointer);
        self.mouse.interrupt(app);
        let on_canvas = current && navigation.position.is_some_and(|pos| matches!(self.hit(ctx, pos), Owner::Canvas { .. }));
        if let Err(error) = crate::touch_mouse::Mouse::navigate(app, navigation, on_canvas) {
            app.toast(error.to_string());
        }
        on_canvas
    }
}
impl CanvasConsumer for Combined {
    fn routes_physical(&self) -> bool {
        true
    }
    fn physical_held(&self) -> bool {
        self.mouse.any_down()
    }
    fn begin_frame(
        &mut self,
        app: &mut FigApp,
        ctx: &Context,
        raw: &egui::RawInput,
        frame: &crate::touch_events::Frame,
        current: bool,
    ) -> Result<bool, String> {
        app.canvas.native_values = true;
        app.canvas.native_hover = None;
        app.canvas.native_position = None;
        self.frame = None;
        let mut physical = self.physical.prepare(ctx, raw, frame).map_err(|error| format!("Unsupported physical input: {error:?}"))?;
        let tail = physical.navigation.filter(|navigation| navigation.ordinal.is_none());
        if tail.is_some() {
            physical.navigation = None;
        }
        app.canvas.native_position = physical.position;
        app.canvas.native_hover = physical.position.filter(|pos| current && matches!(self.hit(ctx, *pos), Owner::Canvas { .. }));
        self.frame = Some(physical);
        if let Some(navigation) = tail {
            self.navigation(app, ctx, navigation, current);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn physical_input(&mut self, app: &mut FigApp, ctx: &Context, input: crate::ordered_input::PhysicalInput<'_>) -> bool {
        if matches!(input.event, egui::Event::MouseWheel { .. } | egui::Event::Zoom(_) | egui::Event::Rotate(_)) {
            let navigation = self.frame.as_mut().and_then(|frame| {
                if frame.navigation.is_some_and(|navigation| navigation.ordinal == Some(input.ordinal)) { frame.navigation.take() } else { None }
            });
            if let Some(navigation) = navigation {
                return self.navigation(app, ctx, navigation, input.current);
            }
            self.finger.interrupt(app, ctx, Interruption::ForwardedPointer);
            self.mouse.interrupt(app);
            return input.current;
        }
        // The explicit physical value interrupts finger intent first. An
        // already-owned mouse remains enrolled across its Move/Up sequence.
        self.finger.interrupt(app, ctx, Interruption::ForwardedPointer);
        let sample = self.frame.as_ref().and_then(|frame| {
            frame.samples.binary_search_by_key(&input.ordinal, |sample| sample.ordinal).ok().and_then(|index| frame.samples.get(index)).cloned()
        });
        let Some(sample) = sample else { return false };
        match self.mouse.sample(app, ctx, &sample, input.hit, input.current) {
            Ok(handled) => handled || (input.current && !sample.any_down && !matches!(sample.event, egui::Event::PointerButton { .. })),
            Err(error) => {
                app.toast(error.to_string());
                false
            }
        }
    }
    fn contact(&mut self, app: &mut FigApp, ctx: &Context, contact: &Contact) {
        self.finger.contact(app, ctx, contact);
    }
    fn interrupt(&mut self, app: &mut FigApp, ctx: &Context, why: Interruption) {
        self.finger.interrupt(app, ctx, why);
        self.mouse.interrupt(app);
    }
    fn producer_reset(&mut self, app: &mut FigApp, ctx: &Context) {
        self.finger.reset(app, ctx);
        self.mouse.reset(app);
        self.physical.reset();
        self.frame = None;
    }
    fn finish_frame(&mut self, app: &mut FigApp, ctx: &Context, timestamp: Option<(f64, Clock, u64)>) {
        self.finger.finish_frame(app, ctx, timestamp);
        self.frame = None;
    }
}

#[cfg(test)]
#[path = "touch_native_combined_tests.rs"]
mod combined_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers_drag_tests::Ui;
    use figcraft_engine::Session;
    #[test]
    fn ipad_touch_completed_layout_excludes_panels_and_toolbar() {
        let u = Ui::new(Session::new());
        let mut generation = Some(0);
        let map = Layout::capture(&u.app, &u.ctx, None, &mut generation).unwrap();
        assert!(matches!(map.classify(&u.ctx, map.canvas.center()), Owner::Canvas { .. }));
        assert_eq!(map.classify(&u.ctx, Pos2::ZERO), Owner::Panel);
        assert_eq!(map.classify(&u.ctx, map.toolbar.unwrap().center()), Owner::Panel);
    }
    #[test]
    fn ipad_touch_layout_epoch_survives_document_previews_but_not_allocation_change() {
        let mut u = Ui::new(Session::new());
        let mut generation = Some(0);
        let first = Layout::capture(&u.app, &u.ctx, None, &mut generation).unwrap();
        u.app.run("shape.rectangle", serde_json::json!({"x":10,"y":10,"width":50,"height":50})).unwrap();
        let next = Layout::capture(&u.app, &u.ctx, Some(&first), &mut generation).unwrap();
        assert_eq!(first.target.owner(), next.target.owner());
        u.app.canvas.rect.max.x -= 20.0;
        let resized = Layout::capture(&u.app, &u.ctx, Some(&next), &mut generation).unwrap();
        assert_ne!(next.target.owner(), resized.target.owner());
    }
    #[test]
    fn ipad_touch_layout_refuses_a_tab_switched_after_canvas_drawing() {
        let mut u = Ui::new(Session::new());
        let mut generation = Some(0);
        u.app.run("file.new", serde_json::json!({})).unwrap();
        assert!(Layout::capture(&u.app, &u.ctx, None, &mut generation).is_none());
    }
}

#[cfg(test)]
#[path = "touch_native_tests.rs"]
mod host_tests;
