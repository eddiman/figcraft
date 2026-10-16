//! Pure local finger arbitration. This module owns values only, never Session,
//! egui input state or native references. T17 must review confirmed Pencil input.
use std::collections::BTreeMap;

use egui::{Pos2, TouchPhase};
use figcraft_engine::ViewState;

use crate::touch_events::{Contact, ContactId, MAX_CONTACTS, Owner};

pub const DRAG_POINTS: f64 = 6.0;
pub const LONG_PRESS_SECS: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    Drag,
    TapOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    PendingFinger,
    Editing,
    Navigating,
    SuppressedUntilAllReleased,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Clock,
    Sequence,
    Capacity,
    NonFinite,
    Owner,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// Capture before Down, then dispatch this original Start once.
    Begin(Contact),
    Move(Contact),
    Up(Contact),
    Finish,
    Cancel,
    LongPress(Contact),
    NavigationStart {
        a: Pos2,
        b: Pos2,
    },
    Navigate {
        a: Pos2,
        b: Pos2,
    },
    NavigationEnd,
    Unsupported(Error),
}
#[derive(Clone, Default)]
enum State {
    #[default]
    Idle,
    Pending {
        start: Contact,
        policy: Policy,
    },
    Editing {
        id: ContactId,
        policy: Policy,
    },
    Navigating {
        a: ContactId,
        b: ContactId,
    },
    Suppressed,
}

#[derive(Default)]
pub struct Gesture {
    state: State,
    contacts: BTreeMap<ContactId, Contact>,
    owner: Option<Owner>,
    last_time: Option<f64>,
    last_event: Option<(u64, usize)>,
    finished_frame: Option<u64>,
    blocked: bool,
}
fn distance(a: Pos2, b: Pos2) -> f64 {
    (f64::from(a.x) - f64::from(b.x)).hypot(f64::from(a.y) - f64::from(b.y))
}
impl Gesture {
    pub fn phase(&self) -> Phase {
        match self.state {
            State::Idle => Phase::Idle,
            State::Pending { .. } => Phase::PendingFinger,
            State::Editing { .. } => Phase::Editing,
            State::Navigating { .. } => Phase::Navigating,
            State::Suppressed => Phase::SuppressedUntilAllReleased,
        }
    }
    pub fn deadline(&self) -> Option<f64> {
        match &self.state {
            State::Pending { start, .. } => Some(start.time + LONG_PRESS_SECS),
            _ => None,
        }
    }
    /// External input cancels intent and editing, but never promotes a held
    /// contact after the interruption or after losing a navigation finger.
    pub fn interrupt(&mut self) -> Vec<Output> {
        let output = match self.state {
            State::Editing { policy: Policy::Drag, .. } => vec![Output::Cancel],
            State::Navigating { .. } => vec![Output::NavigationEnd],
            _ => vec![],
        };
        self.state = if self.contacts.is_empty() && !self.blocked { State::Idle } else { State::Suppressed };
        output
    }
    /// Only an explicit producer/focus reset can recover unknown contact identity.
    pub fn reset(&mut self) -> Vec<Output> {
        let output = self.interrupt();
        *self = Self::default();
        output
    }
    fn fail(&mut self, error: Error) -> Vec<Output> {
        let mut output = self.interrupt();
        self.blocked = true;
        self.state = State::Suppressed;
        output.push(Output::Unsupported(error));
        output
    }
    fn clock(&mut self, c: &Contact) -> bool {
        let valid = c.time.is_finite()
            && self.last_time.is_none_or(|t| c.time >= t)
            && self.finished_frame.is_none_or(|f| c.frame > f)
            && self.last_event.is_none_or(|(f, o)| c.frame > f || (c.frame == f && c.ordinal > o));
        if valid {
            self.last_time = Some(c.time);
            self.last_event = Some((c.frame, c.ordinal));
        }
        valid
    }
    fn long_press(&mut self) -> Vec<Output> {
        let State::Pending { start, .. } = &self.state else { return vec![] };
        let Some(current) = self.contacts.get(&start.id).cloned() else { return self.fail(Error::Sequence) };
        self.state = State::Suppressed;
        vec![Output::LongPress(current)]
    }
    fn pair(&self, a: ContactId, b: ContactId) -> Option<(Pos2, Pos2)> {
        Some((self.contacts.get(&a)?.position, self.contacts.get(&b)?.position))
    }
    pub fn event(&mut self, c: &Contact, policy: Policy) -> Vec<Output> {
        if c.owner == Owner::Panel {
            return vec![];
        }
        if !self.clock(c) {
            return self.fail(Error::Clock);
        }
        if !c.position.x.is_finite() || !c.position.y.is_finite() || c.pressure.is_some_and(|p| !p.is_finite()) {
            return self.fail(Error::NonFinite);
        }
        let mut output = vec![];
        // A deadline strictly earlier than this event fires first. Events at the
        // deadline retain raw ordinal priority over the end-of-frame timer.
        if self.deadline().is_some_and(|deadline| c.time > deadline) {
            output.extend(self.long_press());
        }
        if c.phase == TouchPhase::Start {
            if self.contacts.contains_key(&c.id) {
                output.extend(self.fail(Error::Sequence));
                return output;
            }
            if self.contacts.len() >= MAX_CONTACTS {
                output.extend(self.fail(Error::Capacity));
                return output;
            }
            self.contacts.insert(c.id, c.clone());
        } else if let Some(contact) = self.contacts.get_mut(&c.id) {
            *contact = c.clone();
        } else if c.owner == Owner::Suppressed {
            // An interrupted decoder may deliver a late terminal phase without
            // a known Start. It cannot establish a new editable identity.
            if c.phase == TouchPhase::Move && self.contacts.len() < MAX_CONTACTS {
                self.contacts.insert(c.id, c.clone());
            }
        } else {
            output.extend(self.fail(Error::Sequence));
            return output;
        }
        if c.owner == Owner::Suppressed || self.blocked || c.phase == TouchPhase::Cancel {
            output.extend(self.interrupt());
            self.state = State::Suppressed;
        } else if self.owner.is_some_and(|owner| owner != c.owner) && !matches!(self.state, State::Suppressed) {
            output.extend(self.fail(Error::Owner));
        } else {
            let state = std::mem::replace(&mut self.state, State::Idle);
            self.state = match (state, c.phase) {
                (State::Idle, TouchPhase::Start) => {
                    self.owner = Some(c.owner);
                    if c.time + LONG_PRESS_SECS <= c.time {
                        output.extend(self.fail(Error::Clock));
                        State::Suppressed
                    } else {
                        State::Pending { start: c.clone(), policy }
                    }
                }
                (State::Pending { start, policy: _ }, TouchPhase::Start) => {
                    if let Some((a, b)) = self.pair(start.id, c.id) {
                        output.push(Output::NavigationStart { a, b });
                    }
                    State::Navigating { a: start.id, b: c.id }
                }
                (State::Pending { start, policy }, TouchPhase::Move) => {
                    if distance(start.position, c.position) > DRAG_POINTS {
                        if policy == Policy::Drag {
                            output.push(Output::Begin(start));
                            output.push(Output::Move(c.clone()));
                        }
                        State::Editing { id: c.id, policy }
                    } else {
                        State::Pending { start, policy }
                    }
                }
                (State::Pending { start, policy }, TouchPhase::End) => {
                    let moved = distance(start.position, c.position) > DRAG_POINTS;
                    if !moved || policy == Policy::Drag {
                        output.push(Output::Begin(start));
                        if moved {
                            output.push(Output::Move(c.clone()));
                        }
                        output.extend([Output::Up(c.clone()), Output::Finish]);
                    }
                    State::Idle
                }
                (State::Editing { id, policy }, TouchPhase::Start) => {
                    if policy == Policy::Drag {
                        output.push(Output::Cancel);
                    }
                    if let Some((a, b)) = self.pair(id, c.id) {
                        output.push(Output::NavigationStart { a, b });
                    }
                    State::Navigating { a: id, b: c.id }
                }
                (State::Editing { id, policy }, TouchPhase::Move) => {
                    if id != c.id {
                        output.extend(self.fail(Error::Sequence));
                        State::Suppressed
                    } else {
                        if policy == Policy::Drag {
                            output.push(Output::Move(c.clone()));
                        }
                        State::Editing { id, policy }
                    }
                }
                (State::Editing { id, policy }, TouchPhase::End) => {
                    if id == c.id && policy == Policy::Drag {
                        output.extend([Output::Up(c.clone()), Output::Finish]);
                    }
                    State::Idle
                }
                (State::Navigating { .. }, TouchPhase::Start) => {
                    output.push(Output::NavigationEnd);
                    State::Suppressed
                }
                (State::Navigating { a, b }, TouchPhase::Move) => {
                    if let Some((a, b)) = self.pair(a, b) {
                        output.push(Output::Navigate { a, b });
                    }
                    State::Navigating { a, b }
                }
                (State::Navigating { .. }, TouchPhase::End) => {
                    output.push(Output::NavigationEnd);
                    State::Suppressed
                }
                (State::Suppressed, _) => State::Suppressed,
                (state, _) => state,
            };
        }
        if matches!(c.phase, TouchPhase::End | TouchPhase::Cancel) {
            self.contacts.remove(&c.id);
        }
        if self.contacts.is_empty() && !self.blocked {
            self.state = State::Idle;
            self.owner = None;
        }
        output
    }
    /// Run after the batch's earlier raw events. Repeated identical ticks are
    /// harmless; a second different clock for a completed frame is unsupported.
    pub fn tick(&mut self, time: f64, frame: u64) -> Vec<Output> {
        if self.finished_frame == Some(frame) && self.last_time == Some(time) {
            return vec![];
        }
        if !time.is_finite()
            || self.last_time.is_some_and(|t| time < t)
            || self.last_event.is_some_and(|(f, _)| frame < f)
            || self.finished_frame.is_some_and(|f| frame <= f)
        {
            return self.fail(Error::Clock);
        }
        self.last_time = Some(time);
        self.finished_frame = Some(frame);
        if self.deadline().is_some_and(|deadline| time >= deadline) { self.long_press() } else { vec![] }
    }
}

/// Baseline for view-only navigation. A zero/near-zero pair waits for a later
/// valid baseline; cancellation leaves the last accepted transform untouched.
#[derive(Clone, Copy, Debug)]
pub struct Navigation {
    distance: f64,
    zoom: f64,
    anchor: (f64, f64),
    origin: Pos2,
    size: (f64, f64),
}
impl Navigation {
    pub fn begin(a: Pos2, b: Pos2, view: &ViewState, origin: Pos2) -> Option<Self> {
        let distance = distance(a, b);
        let c = centroid(a, b)?;
        if distance < 1.0
            || view.vw <= 0.0
            || view.vh <= 0.0
            || !view.zoom.is_finite()
            || view.zoom <= 0.0
            || !origin.x.is_finite()
            || !origin.y.is_finite()
        {
            return None;
        }
        let anchor = view.screen_to_page(c.0 - f64::from(origin.x), c.1 - f64::from(origin.y));
        if ![anchor.0, anchor.1, view.vw, view.vh].iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(Self { distance, zoom: view.zoom, anchor, origin, size: (view.vw, view.vh) })
    }
    pub fn transform(&self, a: Pos2, b: Pos2) -> Option<(f64, f64, f64)> {
        let c = centroid(a, b)?;
        let d = distance(a, b);
        let zoom = (self.zoom * d / self.distance).clamp(ViewState::MIN_ZOOM, ViewState::MAX_ZOOM);
        let cx = self.anchor.0 - (c.0 - f64::from(self.origin.x) - self.size.0 / 2.0) / zoom;
        let cy = self.anchor.1 - (c.1 - f64::from(self.origin.y) - self.size.1 / 2.0) / zoom;
        [cx, cy, zoom].iter().all(|v| v.is_finite()).then_some((cx, cy, zoom))
    }
}
fn centroid(a: Pos2, b: Pos2) -> Option<(f64, f64)> {
    let x = (f64::from(a.x) + f64::from(b.x)) / 2.0;
    let y = (f64::from(a.y) + f64::from(b.y)) / 2.0;
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod tests;
