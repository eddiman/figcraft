//! Explicit, synchronous dispatch for a complete canvas consumer. The native
//! observer never calls this boundary. Text stays borrowed in RawInput and is
//! consumed only after the existing keyboard/IME interpretation has run.
use std::collections::{BTreeMap, BTreeSet};

use egui::{Context, Event, LayerId, RawInput, Rect, TouchPhase};
use figcraft_doc::NodeId;

use crate::FigApp;
use crate::touch_events::{Clock, Contact, ContactId, Frame, InputKind, Item, MAX_CONTACTS, Normalizer, Owner, Routing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interruption {
    UnsupportedInput,
    SourceOrLayout,
    ExternalKeyboard,
    ForwardedPointer,
    NativeCancel,
}

/// T16 must provide every operation. There is no production recognizer fallback.
pub trait CanvasConsumer {
    fn contact(&mut self, app: &mut FigApp, ctx: &Context, contact: &Contact);
    fn interrupt(&mut self, app: &mut FigApp, ctx: &Context, why: Interruption);
    /// Mirrors the normalizer's explicit focus reset; layout changes retain IDs.
    fn producer_reset(&mut self, app: &mut FigApp, ctx: &Context);
    fn finish_frame(&mut self, app: &mut FigApp, ctx: &Context, timestamp: Option<(f64, Clock, u64)>);
}

/// A completed layout's source identity, separate from frozen contact ownership.
/// The caller's classifier must also exclude panel/overlay hit regions from that
/// completed layout. Rotation or another layout revision requires a new target.
#[derive(Clone, PartialEq)]
pub struct Target {
    uid: u64,
    page: NodeId,
    layout: u64,
    rect: Rect,
    viewport: Rect,
    scale: f32,
    native_scale: Option<f32>,
    safe_area: egui::SafeAreaInsets,
    viewport_id: egui::ViewportId,
    window: u64,
    modal: Option<LayerId>,
}
impl Target {
    pub fn capture(app: &FigApp, ctx: &Context, layout: u64) -> Option<Self> {
        let ds = app.session.ds().ok()?;
        if app.canvas_interruption.is_none() || !app.canvas.rect.is_finite() || !app.canvas.rect.is_positive() || blocked_ui(app) {
            return None;
        }
        if !ctx.input(|i| i.viewport_rect().is_finite()) {
            return None;
        }
        Some(Self {
            uid: ds.uid,
            page: ds.page,
            layout,
            rect: app.canvas.rect,
            viewport: ctx.input(|i| i.viewport_rect()),
            scale: ctx.pixels_per_point(),
            native_scale: ctx.input(|i| i.viewport().native_pixels_per_point),
            safe_area: ctx.input(|i| i.safe_area_insets()),
            viewport_id: ctx.viewport_id(),
            window: app.windows.current,
            modal: ctx.memory(|m| m.top_modal_layer()),
        })
    }
    pub(crate) fn with_layout(mut self, layout: u64) -> Self {
        self.layout = layout;
        self
    }
    pub(crate) fn same_geometry(&self, other: &Self) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        a.layout = 0;
        b.layout = 0;
        a == b
    }
    pub fn owner(&self) -> Owner {
        Owner::Canvas { uid: self.uid, layout: self.layout }
    }
    fn current(&self, app: &FigApp, ctx: &Context, raw: &RawInput) -> bool {
        app.canvas_interruption.is_some()
            && !blocked_ui(app)
            && raw.focused
            && ctx.input(|i| i.focused)
            && app.canvas.rect.is_finite()
            && self.rect == app.canvas.rect
            && self.window == app.windows.current
            && self.scale == ctx.pixels_per_point()
            && raw.viewport_id == self.viewport_id
            && raw.screen_rect.is_none_or(|r| r.is_finite() && r == self.viewport)
            && raw.safe_area_insets.is_none_or(|area| area == self.safe_area)
            && raw.viewports.get(&raw.viewport_id).and_then(|v| v.native_pixels_per_point).is_none_or(|scale| Some(scale) == self.native_scale)
            && self.modal == ctx.memory(|m| m.top_modal_layer())
            && app.session.ds().is_ok_and(|ds| ds.uid == self.uid && ds.page == self.page)
    }
}
fn blocked_ui(app: &FigApp) -> bool {
    app.ui.home
        || app.ui.present.is_some()
        || app.ui.palette.is_some()
        || app.ui.about
        || app.ui.preferences
        || app.ui.confirm_close
        || app.ui.export_preview
        || app.ui.batch_rename.is_some()
        || app.ui.branch_review.is_some()
        || app.ui.plugin_form.is_some()
        || app.ui.live.join.is_some()
        || app.ui.context_menu.is_some()
}

#[derive(Default)]
pub struct Boundary {
    normalizer: Normalizer,
    /// Only canvas identities are retained, bounded by the producer's limit.
    /// true means a cancelled held contact cannot acquire ownership on Move/Up.
    held: BTreeMap<ContactId, bool>,
    last_interruption: Option<u64>,
}
impl Boundary {
    fn interrupt(&mut self, app: &mut FigApp, ctx: &Context, consumer: &mut impl CanvasConsumer, why: Interruption) {
        app.cancel_canvas_pointer();
        for suppressed in self.held.values_mut() {
            *suppressed = true;
        }
        consumer.interrupt(app, ctx, why);
    }

    /// Call in raw_input_hook, before egui aggregates pointer state. `target` and
    /// `classify` describe the preceding completed layout. This method does not
    /// retain the raw vector or text. The caller must use a complete consumer.
    pub fn process(
        &mut self,
        app: &mut FigApp,
        ctx: &Context,
        raw: &mut RawInput,
        target: Option<&Target>,
        classify: impl Fn(egui::Pos2) -> Owner,
        consumer: &mut impl CanvasConsumer,
    ) -> Frame {
        let physical_held = self.normalizer.physical_primary_down();
        let frame = self.normalizer.process(raw, Routing::CanvasConsumer, classify);
        let interrupted_between_frames = self.last_interruption.is_some() && self.last_interruption != app.canvas_interruption;
        let had_owner = app.canvas_pointer.is_some();
        app.canvas_pointer_preflight(ctx);
        let valid = target.is_some_and(|t| t.current(app, ctx, raw));
        let mut forwarded = !valid || physical_held || !raw.dropped_files.is_empty();
        app.canvas_input_blocked = frame.blocked;
        if frame.blocked {
            self.interrupt(app, ctx, consumer, Interruption::UnsupportedInput);
            app.cancel_pointer_owners(ctx);
            self.normalizer.suppress_canvas();
            self.held.clear();
            forwarded = true;
        } else if !valid || interrupted_between_frames || (had_owner && app.canvas_pointer.is_none()) {
            self.interrupt(app, ctx, consumer, Interruption::SourceOrLayout);
            // A barrier between frames suppresses the held identities. A new
            // Start can use the current layout when its source is still valid.
            forwarded |= !valid;
        }
        if physical_held || !raw.dropped_files.is_empty() {
            self.interrupt(app, ctx, consumer, Interruption::ForwardedPointer);
        }
        let mut keyboard = crate::shortcuts::KeyboardFrame::new(&raw.events);
        let mut consumed = BTreeSet::new();
        let now = frame.timestamp.map_or(0.0, |(time, _, _)| time);
        for item in &frame.items {
            match item {
                Item::Contact(contact) => {
                    if contact.owner == Owner::Panel {
                        self.interrupt(app, ctx, consumer, Interruption::ForwardedPointer);
                        app.cancel_pointer_owners(ctx);
                        forwarded = true;
                        consumer.contact(app, ctx, contact);
                        continue;
                    }
                    let current = target.is_some_and(|t| t.current(app, ctx, raw) && t.owner() == contact.owner);
                    if matches!(contact.owner, Owner::Canvas { .. }) && !current {
                        self.interrupt(app, ctx, consumer, Interruption::SourceOrLayout);
                        forwarded = true;
                    }
                    if contact.phase == TouchPhase::Cancel {
                        self.interrupt(app, ctx, consumer, Interruption::NativeCancel);
                    }
                    let mut contact = contact.clone();
                    let suppressed = forwarded || !current || contact.owner == Owner::Suppressed;
                    if contact.phase == TouchPhase::Start && self.held.len() < MAX_CONTACTS {
                        self.held.insert(contact.id, suppressed);
                    }
                    if suppressed || self.held.get(&contact.id).copied().unwrap_or(true) {
                        contact.owner = Owner::Suppressed;
                        if let Some(held) = self.held.get_mut(&contact.id) {
                            *held = true;
                        }
                    }
                    consumer.contact(app, ctx, &contact);
                    if matches!(contact.phase, TouchPhase::End | TouchPhase::Cancel) {
                        self.held.remove(&contact.id);
                    }
                }
                Item::PanelCancelled { .. } => {
                    self.interrupt(app, ctx, consumer, Interruption::ForwardedPointer);
                    // This executes before the reset Up enters egui, even if the
                    // application owner was started outside an engine checkpoint.
                    app.cancel_pointer_owners(ctx);
                    forwarded = true;
                }
                Item::Input { index, kind: InputKind::Keyboard, .. } => {
                    self.interrupt(app, ctx, consumer, Interruption::ExternalKeyboard);
                    let panel_keyboard = app.field_focus || crate::a11y::field_focused(ctx);
                    if !forwarded && !panel_keyboard && target.is_some_and(|t| t.current(app, ctx, raw)) {
                        if let Some(event) = raw.events.get(*index) {
                            keyboard.event(app, ctx, event, now);
                            consumed.insert(*index);
                        }
                    } else {
                        // egui owns this event; subsequent contacts cannot run
                        // before its field mutation or against its old layout.
                        forwarded = true;
                    }
                }
                Item::Input { index, kind, .. } => {
                    let navigation =
                        raw.events.get(*index).is_some_and(|event| matches!(event, Event::MouseWheel { .. } | Event::Zoom(_) | Event::Rotate(_)));
                    if *kind == InputKind::Pointer || navigation {
                        self.interrupt(app, ctx, consumer, Interruption::ForwardedPointer);
                        forwarded = true;
                    }
                }
            }
        }
        keyboard.finish(app, ctx);
        self.normalizer.suppress_contacts(self.held.iter().filter_map(|(id, suppressed)| suppressed.then_some(*id)));
        let mut index = 0;
        raw.events.retain(|_| {
            let keep = !consumed.contains(&index);
            index += 1;
            keep
        });
        if !raw.focused || raw.events.iter().any(|event| matches!(event, Event::WindowFocused(false))) {
            self.held.clear();
            consumer.producer_reset(app, ctx);
        } else {
            consumer.finish_frame(app, ctx, frame.timestamp);
        }
        self.last_interruption = app.canvas_interruption;
        frame
    }
}

impl FigApp {
    /// An accepted canvas Down can take keyboard ownership. Pending contacts do
    /// not call this, so panel typing keeps its existing egui route.
    pub fn take_canvas_keyboard(&mut self, ctx: &Context) {
        self.field_focus = false;
        self.ui.layers_focus = false;
        self.ui.key_box = None;
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
    }
}

#[cfg(test)]
#[path = "ordered_input_tests.rs"]
mod tests;
