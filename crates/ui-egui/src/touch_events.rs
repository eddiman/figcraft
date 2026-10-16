//! Ordered decoding of the pinned egui-winit native touch producer. The observer
//! does not change routing. A complete consumer must explicitly request routing.
use std::collections::BTreeMap;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, TouchPhase};

pub(crate) const MAX_CONTACTS: usize = 64;
const MAX_ITEMS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContactId {
    pub device: u64,
    pub contact: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Panel,
    Canvas { uid: u64, layout: u64 },
    Suppressed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    Frame,
    PredictedFrame,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    pub id: ContactId,
    pub source: Source,
    pub phase: TouchPhase,
    pub position: Pos2,
    pub pressure: Option<f32>,
    pub time: f64,
    pub clock: Clock,
    pub frame: u64,
    pub ordinal: usize,
    pub owner: Owner,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Pointer,
    Keyboard,
    Other,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Contact(Contact),
    /// Index in the resulting raw event vector; text remains in that vector.
    Input {
        ordinal: usize,
        index: usize,
        kind: InputKind,
    },
    /// Cancel application owners before processing the forwarded pointer reset.
    PanelCancelled {
        ordinal: usize,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    InvalidClock,
    NonFinite,
    DuplicateStart,
    MissingStart,
    NumericIdCollision,
    InvalidSuffix,
    Capacity,
    FrameExhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Routing {
    Observe,
    CanvasConsumer,
}
#[derive(Default, Debug)]
pub struct Frame {
    pub items: Vec<Item>,
    pub timestamp: Option<(f64, Clock, u64)>,
    pub issues: Vec<Issue>,
    pub interrupted: bool,
    pub physical_primary_down: bool,
    pub blocked: bool,
}
impl Frame {
    fn note(&mut self, issue: Issue) {
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
        self.interrupted = true;
        self.blocked = true;
    }
}
#[derive(Default)]
pub struct Normalizer {
    contacts: BTreeMap<ContactId, Owner>,
    backend_primary: Option<u64>,
    backend_position: Option<Pos2>,
    physical_primary: bool,
    forwarded_touch_down: bool,
    modifiers: Modifiers,
    frame: u64,
    time: Option<f64>,
    overflow: bool,
}
pub(crate) fn external_edit_event(e: &Event) -> bool {
    matches!(e, Event::Text(_) | Event::Ime(_) | Event::Key { .. } | Event::Paste(_) | Event::Copy | Event::Cut)
}
fn pointer(e: &Event) -> bool {
    matches!(e, Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone | Event::MouseMoved(_))
}
fn same_position(a: Pos2, b: Pos2) -> bool {
    a.x == b.x && a.y == b.y
}
fn primary(e: &Event, down: bool, position: Pos2) -> bool {
    matches!(e,Event::PointerButton{button:PointerButton::Primary,pressed,pos,..} if *pressed==down && same_position(*pos, position))
}
impl Normalizer {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Freeze existing contact ownership after an interruption. New ownership is
    /// decided only at Start; crossing a panel edge cannot transfer a contact.
    pub fn suppress_canvas(&mut self) {
        for owner in self.contacts.values_mut() {
            if matches!(owner, Owner::Canvas { .. }) {
                *owner = Owner::Suppressed;
            }
        }
    }
    pub(crate) fn suppress_contacts(&mut self, ids: impl Iterator<Item = ContactId>) {
        for id in ids {
            if let Some(owner) = self.contacts.get_mut(&id)
                && matches!(owner, Owner::Canvas { .. })
            {
                *owner = Owner::Suppressed;
            }
        }
    }
    pub fn physical_primary_down(&self) -> bool {
        self.physical_primary
    }

    pub fn process(&mut self, raw: &mut RawInput, routing: Routing, classify: impl Fn(Pos2) -> Owner) -> Frame {
        let mut result = Frame { blocked: self.overflow || !raw.focused, ..Default::default() };
        let Some(frame) = self.frame.checked_add(1) else {
            result.note(Issue::FrameExhausted);
            result.interrupted = true;
            return result;
        };
        self.frame = frame;
        let (time, clock) = match raw.time {
            Some(t) if t.is_finite() && self.time.is_none_or(|last| t >= last) => (t, Clock::Frame),
            None if raw.predicted_dt.is_finite() && raw.predicted_dt >= 0.0 => {
                (self.time.unwrap_or(0.0) + f64::from(raw.predicted_dt), Clock::PredictedFrame)
            }
            _ => {
                result.note(Issue::InvalidClock);
                result.interrupted = true;
                self.suppress_canvas();
                (self.time.unwrap_or(0.0), Clock::PredictedFrame)
            }
        };
        let time = if time.is_finite() {
            time
        } else {
            result.note(Issue::InvalidClock);
            self.suppress_canvas();
            self.time.unwrap_or(0.0)
        };
        self.time = Some(time);
        result.timestamp = Some((time, clock, frame));
        if !raw.focused {
            result.interrupted = true;
            self.suppress_canvas();
        }
        let mut lost_focus = !raw.focused;
        let original = std::mem::take(&mut raw.events);
        let mut output = Vec::with_capacity(original.len());
        let mut i = 0;
        while let Some(event) = original.get(i) {
            if original.len() > MAX_ITEMS && !result.issues.contains(&Issue::Capacity) {
                result.note(Issue::Capacity);
                result.interrupted = true;
                self.overflow = true;
                self.suppress_canvas();
            }
            if let Event::Touch { device_id, id, phase, pos, force } = event {
                let key = ContactId { device: device_id.0, contact: id.0 };
                let finite = pos.x.is_finite() && pos.y.is_finite() && force.is_none_or(f32::is_finite);
                if !finite {
                    result.note(Issue::NonFinite);
                    result.interrupted = true;
                    self.suppress_canvas();
                }
                let mut owner = match phase {
                    TouchPhase::Start => {
                        if self.contacts.contains_key(&key) {
                            result.note(Issue::DuplicateStart);
                            result.interrupted = true;
                            self.suppress_canvas();
                        }
                        let collision = self.contacts.keys().any(|k| k.contact == key.contact && k.device != key.device);
                        if collision {
                            result.note(Issue::NumericIdCollision);
                            result.interrupted = true;
                            self.suppress_canvas();
                        }
                        let owner = if self.overflow || result.blocked || !raw.focused || !result.issues.is_empty() || !finite || collision {
                            Owner::Suppressed
                        } else {
                            classify(*pos)
                        };
                        if self.contacts.len() < MAX_CONTACTS || self.contacts.contains_key(&key) {
                            self.contacts.insert(key, owner);
                        } else {
                            result.note(Issue::Capacity);
                            result.interrupted = true;
                            self.overflow = true;
                            self.suppress_canvas();
                        }
                        owner
                    }
                    _ => self.contacts.get(&key).copied().unwrap_or_else(|| {
                        result.note(Issue::MissingStart);
                        result.interrupted = true;
                        Owner::Suppressed
                    }),
                };
                let emulated = self.backend_primary.is_none() || self.backend_primary == Some(key.contact);
                let mut suffix = 0;
                let valid = if !emulated {
                    true
                } else {
                    match phase {
                        TouchPhase::Start => {
                            suffix = 2;
                            matches!(original.get(i+1),Some(Event::PointerMoved(p)) if same_position(*p, *pos))
                                && original.get(i + 2).is_some_and(|e| primary(e, true, *pos))
                        }
                        TouchPhase::Move => {
                            suffix = 1;
                            matches!(original.get(i+1),Some(Event::PointerMoved(p)) if same_position(*p, *pos))
                        }
                        TouchPhase::End => {
                            if let Some(p) = self.backend_position {
                                suffix = 2;
                                original.get(i + 1).is_some_and(|e| primary(e, false, p)) && matches!(original.get(i + 2), Some(Event::PointerGone))
                            } else {
                                suffix = 1;
                                matches!(original.get(i + 1), Some(Event::PointerGone))
                            }
                        }
                        TouchPhase::Cancel => {
                            suffix = 1;
                            matches!(original.get(i + 1), Some(Event::PointerGone))
                        }
                    }
                };
                if !valid {
                    result.note(Issue::InvalidSuffix);
                    result.interrupted = true;
                    self.suppress_canvas();
                    suffix = 0;
                }
                if emulated {
                    match phase {
                        TouchPhase::Start => {
                            self.backend_primary = Some(key.contact);
                            self.backend_position = Some(*pos);
                        }
                        TouchPhase::Move => self.backend_position = Some(*pos),
                        TouchPhase::End | TouchPhase::Cancel => {
                            self.backend_primary = None;
                            self.backend_position = None;
                        }
                    }
                }
                if !finite || (result.blocked && !matches!(owner, Owner::Panel)) {
                    owner = Owner::Suppressed;
                }
                let route_canvas = routing == Routing::CanvasConsumer && !matches!(owner, Owner::Panel);
                if result.items.len() < MAX_ITEMS {
                    result.items.push(Item::Contact(Contact {
                        id: key,
                        source: Source::Unknown,
                        phase: *phase,
                        position: *pos,
                        pressure: force.map(|f| f.clamp(0.0, 1.0)),
                        time,
                        clock,
                        frame,
                        ordinal: i,
                        owner,
                    }));
                }
                if !route_canvas {
                    output.push(event.clone());
                    if valid {
                        for e in original.iter().skip(i + 1).take(suffix) {
                            output.push(e.clone());
                        }
                    }
                    if emulated {
                        match phase {
                            TouchPhase::Start => self.forwarded_touch_down = true,
                            TouchPhase::End => self.forwarded_touch_down = false,
                            TouchPhase::Cancel => {
                                if routing == Routing::CanvasConsumer {
                                    if result.items.len() < MAX_ITEMS {
                                        result.items.push(Item::PanelCancelled { ordinal: i });
                                    }
                                    // Keep a separately held physical mouse down. A touch
                                    // cancellation cannot manufacture its release.
                                    if self.forwarded_touch_down && !self.physical_primary && valid && finite {
                                        // stop_dragging does not clear egui's potential
                                        // click. Mark this synthetic reset as moved before
                                        // Up so a short Cancel cannot click a panel button.
                                        // These are control events, never source matching.
                                        output.push(Event::PointerMoved(Pos2::new(-1.0e6, -1.0e6)));
                                        output.push(Event::PointerMoved(Pos2::new(1.0e6, 1.0e6)));
                                        output.push(Event::PointerButton {
                                            pos: *pos,
                                            button: PointerButton::Primary,
                                            pressed: false,
                                            modifiers: self.modifiers,
                                        });
                                        output.push(Event::PointerGone);
                                    }
                                }
                                self.forwarded_touch_down = false;
                            }
                            _ => {}
                        }
                    }
                }
                if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                    self.contacts.remove(&key);
                }
                i += 1 + suffix;
            } else {
                let finite_pointer = match event {
                    Event::PointerMoved(p) | Event::PointerButton { pos: p, .. } => p.x.is_finite() && p.y.is_finite(),
                    Event::MouseMoved(delta) => delta.x.is_finite() && delta.y.is_finite(),
                    _ => true,
                };
                if !finite_pointer {
                    result.note(Issue::NonFinite);
                    self.suppress_canvas();
                    if routing == Routing::CanvasConsumer {
                        // Unsupported values cannot enter egui's position equality
                        // or geometry. Preserve the untouched stream in Observe.
                        i += 1;
                        continue;
                    }
                }
                match event {
                    Event::ModifiersChanged(modifiers) => self.modifiers = *modifiers,
                    Event::PointerMoved(p) => self.backend_position = Some(*p),
                    Event::PointerGone => self.backend_position = None,
                    Event::PointerButton { button: PointerButton::Primary, pressed, .. } => self.physical_primary = *pressed,
                    Event::WindowFocused(false) => {
                        lost_focus = true;
                        result.blocked = true;
                        result.interrupted = true;
                        self.suppress_canvas();
                        self.physical_primary = false;
                        self.forwarded_touch_down = false;
                    }
                    _ => {}
                }
                if result.items.len() < MAX_ITEMS {
                    result.items.push(Item::Input {
                        ordinal: i,
                        index: output.len(),
                        kind: if external_edit_event(event) {
                            InputKind::Keyboard
                        } else if pointer(event) {
                            InputKind::Pointer
                        } else {
                            InputKind::Other
                        },
                    });
                }
                output.push(event.clone());
                i += 1;
            }
        }
        self.overflow |= result.blocked;
        // Overflow makes contact identity incomplete. Explicit focus loss is the
        // reset boundary; guessing that an untracked contact ended is unsafe.
        if lost_focus {
            self.contacts.clear();
            // egui-winit preserves its numeric primary and cursor on focus
            // loss. Keep that producer mirror for a late End/Cancel suffix.
            self.physical_primary = false;
            self.forwarded_touch_down = false;
            self.modifiers = Modifiers::default();
            self.time = None;
            self.overflow = false;
        }
        result.physical_primary_down = self.physical_primary;
        raw.events = output;
        result
    }
}

#[cfg(test)]
#[path = "touch_events_tests.rs"]
mod tests;
