//! Opt-in, local pointer checkpoints. This does not replace tool undo grouping or
//! transact arbitrary Session operations. The host must cancel before direct
//! state changes, tab/layout changes, callbacks or draining provisional effects.
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use figcraft_doc::{Document, NodeId};
use serde_json::Value;

use crate::tools::{Overlay, ToolState};
use crate::{DocState, EngineError, History, Interaction, Result, Session, TextEdit, UiRequest, guard};

static NEXT: AtomicU64 = AtomicU64::new(1);
const MAX_COMMANDS: usize = 5000;
const MAX_REQUESTS: usize = 1024;

/// An identity for one Session-owned checkpoint. Dropping this value does not
/// cancel: the host must explicitly finish or cancel before releasing ownership.
#[derive(Debug)]
pub struct PointerGestureToken {
    uid: u64,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerGestureEnd {
    Cancelled,
    Finished,
    Inactive,
    SourceGone,
}

struct Editing {
    doc: Arc<Document>,
    selection: Vec<NodeId>,
    page: NodeId,
    history: History,
    interaction: Option<Interaction>,
    text: Option<TextEdit>,
    vector: Option<crate::vectoredit::VectorEdit>,
    hover: Option<NodeId>,
    guide: Option<(NodeId, usize)>,
    origin: Option<crate::rotation_origin::RotationOrigin>,
    duplicate: Option<crate::cmd::edit::Duplicated>,
    text_preview: Option<Arc<Document>>,
    focus: Option<NodeId>,
    view: (f64, f64, f64),
}
impl Editing {
    fn capture(ds: &DocState) -> Self {
        Self {
            doc: ds.doc.clone(),
            selection: ds.selection.clone(),
            page: ds.page,
            history: ds.history.clone(),
            interaction: ds.interaction.clone(),
            text: ds.text_edit.clone(),
            vector: ds.vector_edit.clone(),
            hover: ds.hover,
            guide: ds.guide,
            origin: ds.rotation_origin.clone(),
            duplicate: ds.last_duplicate.clone(),
            text_preview: ds.text_preview.clone(),
            focus: ds.focus,
            view: (ds.view.cx, ds.view.cy, ds.view.zoom),
        }
    }
    fn restore(self, ds: &mut DocState) {
        ds.doc = self.doc;
        ds.selection = self.selection;
        ds.page = self.page;
        ds.history = self.history;
        ds.interaction = self.interaction;
        ds.text_edit = self.text;
        ds.vector_edit = self.vector;
        ds.hover = self.hover;
        ds.guide = self.guide;
        ds.rotation_origin = self.origin;
        ds.last_duplicate = self.duplicate;
        ds.text_preview = self.text_preview;
        ds.focus = self.focus;
        (ds.view.cx, ds.view.cy, ds.view.zoom) = self.view;
        ds.revision = ds.revision.saturating_add(1);
    }
}

struct Expected {
    doc: Arc<Document>,
    revision: u64,
    selection: Vec<NodeId>,
    page: NodeId,
    text: Option<TextEdit>,
    vector: Option<crate::vectoredit::VectorEdit>,
}
impl Expected {
    fn capture(ds: &DocState) -> Self {
        Self {
            doc: ds.doc.clone(),
            revision: ds.revision,
            selection: ds.selection.clone(),
            page: ds.page,
            text: ds.text_edit.clone(),
            vector: ds.vector_edit.clone(),
        }
    }
    fn matches(&self, ds: &DocState) -> bool {
        Arc::ptr_eq(&self.doc, &ds.doc)
            && self.revision == ds.revision
            && self.selection == ds.selection
            && self.page == ds.page
            && self.text == ds.text_edit
            && self.vector == ds.vector_edit
    }
}

pub(crate) struct Checkpoint {
    uid: u64,
    generation: u64,
    editing: Editing,
    tool: ToolState,
    expected: Expected,
    colors: Vec<figcraft_color::Color>,
    message: Option<String>,
    commands: Vec<(String, Value)>,
    last_command: Option<(String, Value)>,
    requests: Vec<UiRequest>,
    fault: Option<String>,
}
impl Checkpoint {
    fn matches(&self, token: &PointerGestureToken) -> bool {
        self.uid == token.uid && self.generation == token.generation
    }
}
fn expected_source_missing(session: &Session, uid: u64) -> bool {
    !session.docs.iter().any(|d| d.uid == uid)
}
fn disabled(why: &str) -> EngineError {
    EngineError::Disabled("pointer.gesture".into(), why.into())
}
fn generation(counter: &AtomicU64) -> Result<u64> {
    counter.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| disabled("pointer gesture generations exhausted"))
}

impl Session {
    /// Capture immediately before Down. Idle text edits and accepted pen points
    /// keep their original outer interaction. Attached live sessions are refused
    /// because a local snapshot cannot retract published operations.
    pub fn begin_pointer_gesture(&mut self) -> Result<PointerGestureToken> {
        if self.pointer_owned || self.pointer_checkpoint.is_some() {
            return Err(disabled("a pointer checkpoint is already active"));
        }
        let ds = self.ds()?;
        if self.tool.busy() {
            return Err(disabled("another tool gesture is active"));
        }
        if ds.interaction.is_some() && ds.text_edit.is_none() && self.tool.pen.is_none() {
            return Err(disabled("an unrelated interaction is active"));
        }
        if let Some(why) = self.active_read_only() {
            return Err(disabled(why));
        }
        if ds.preview.is_some() {
            return Err(disabled(crate::PREVIEW_READ_ONLY));
        }
        if self.pointer_source_live(ds.uid) {
            return Err(disabled("live pointer checkpoints require collaboration-aware cancellation"));
        }
        // At most sixteen colours are produced by the built-in tools. Refuse an
        // oversized caller-supplied list instead of cloning an unbounded list.
        if self.recent_colors.len() > MAX_REQUESTS {
            return Err(disabled("too many recent colours for a pointer checkpoint"));
        }
        let token = PointerGestureToken { uid: ds.uid, generation: generation(&NEXT)? };
        self.pointer_checkpoint = Some(Checkpoint {
            uid: token.uid,
            generation: token.generation,
            editing: Editing::capture(ds),
            tool: self.tool.clone(),
            expected: Expected::capture(ds),
            colors: self.recent_colors.clone(),
            message: self.message.clone(),
            commands: Vec::new(),
            last_command: None,
            requests: Vec::new(),
            fault: None,
        });
        Ok(token)
    }

    /// Run only the initiating pointer tool or its synchronous local preview.
    /// The closure must not perform I/O, switch tabs, invoke external services or
    /// mutate another document. The host must serialize contacts and interrupt
    /// before direct state changes or deferred native callbacks.
    pub fn with_pointer_gesture<T>(&mut self, token: &PointerGestureToken, operation: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        if self.pointer_owned {
            return Err(disabled("nested pointer dispatch is not allowed"));
        }
        self.validate_pointer(token, true)?;
        self.pointer_owned = true;
        let mut result = guard::run("pointer.gesture", || operation(self));
        self.pointer_owned = false;
        let expected = self.docs.iter().find(|d| d.uid == token.uid).map(Expected::capture);
        if let Some(cp) = self.pointer_checkpoint.as_mut().filter(|c| c.matches(token)) {
            if let Some(expected) = expected {
                cp.expected = expected;
            }
            if let Some(fault) = cp.fault.take() {
                result = Err(disabled(&fault));
            }
        }
        if self.ds().is_ok_and(|d| d.uid != token.uid) || expected_source_missing(self, token.uid) {
            result = Err(disabled("owned pointer dispatch changed or closed its source"));
        }
        if result.is_err() {
            // Mutations since validation were made inside this synchronous scope.
            // No external event may run there; roll them back before returning.
            let _ = self.cancel_pointer_gesture(token);
        }
        result
    }

    /// Restore only pointer-owned editing state; retain destination, saved roots,
    /// versions, branches and viewport dimensions. Old tokens are harmless.
    pub fn cancel_pointer_gesture(&mut self, token: &PointerGestureToken) -> Result<PointerGestureEnd> {
        self.end_pointer(token, true)
    }

    /// Release after normal Up on the initiating active tab. The tool already
    /// performed its existing commit; an inactive source must be cancelled.
    pub fn finish_pointer_gesture(&mut self, token: &PointerGestureToken) -> Result<PointerGestureEnd> {
        self.end_pointer(token, false)
    }

    fn validate_pointer(&mut self, token: &PointerGestureToken, require_active: bool) -> Result<()> {
        let Some(cp) = self.pointer_checkpoint.as_ref().filter(|c| c.matches(token)) else {
            return Err(disabled("pointer checkpoint is no longer active"));
        };
        let Some(ds) = self.docs.iter().find(|d| d.uid == token.uid) else { return Err(EngineError::NoDocument) };
        if !cp.expected.matches(ds) || self.pointer_source_live(ds.uid) {
            // A caller bypassed the mutation barrier. Never replace independent
            // data with the old snapshot. Stop its drag so a later Up cannot commit.
            let own_interaction = cp.editing.interaction.is_none();
            self.pointer_checkpoint = None;
            if own_interaction && let Some(ds) = self.docs.iter_mut().find(|d| d.uid == token.uid) {
                ds.interaction = None;
            }
            if self.ds().is_err() || self.ds().is_ok_and(|d| d.uid == token.uid) {
                self.tool.gesture = Default::default();
                self.tool.overlay = Overlay::default();
                self.tool.last_move = None;
            }
            return Err(disabled("document changed outside the owned pointer operation"));
        }
        if require_active && self.ds()?.uid != token.uid {
            return Err(disabled("pointer source is not the active document"));
        }
        Ok(())
    }

    fn end_pointer(&mut self, token: &PointerGestureToken, cancel: bool) -> Result<PointerGestureEnd> {
        if self.pointer_owned {
            return Err(disabled("cannot end a pointer checkpoint inside its dispatch"));
        }
        if !self.pointer_checkpoint.as_ref().is_some_and(|c| c.matches(token)) {
            return Ok(PointerGestureEnd::Inactive);
        }
        if !self.docs.iter().any(|d| d.uid == token.uid) {
            self.pointer_checkpoint = None;
            if self.ds().is_err() {
                self.tool = ToolState { tool: self.tool.tool, pencil_brush: self.tool.pencil_brush.clone(), ..Default::default() };
            }
            return Ok(PointerGestureEnd::SourceGone);
        }
        self.validate_pointer(token, !cancel)?;
        let Some(cp) = self.pointer_checkpoint.take() else { return Ok(PointerGestureEnd::Inactive) };
        if !cancel {
            for (id, params) in cp.commands {
                self.append_journal(id, params);
            }
            if cp.last_command.is_some() {
                self.last_command = cp.last_command;
            }
            self.ui_requests.extend(cp.requests);
            crate::branches::sync(self);
            return Ok(PointerGestureEnd::Finished);
        }
        let active = self.ds().is_ok_and(|d| d.uid == token.uid);
        // The checkpoint is already detached, so the ordinary cancellation guard
        // cannot recurse. A preceding text/pen interaction is never cancelled.
        if active && cp.editing.interaction.is_none() && self.ds()?.revision < u64::MAX {
            self.cancel_interaction()?;
        }
        if let Some(ds) = self.docs.iter_mut().find(|d| d.uid == token.uid) {
            cp.editing.restore(ds);
        }
        if active || self.ds().is_err() {
            self.tool = cp.tool;
        }
        if active {
            self.recent_colors = cp.colors;
            self.message = cp.message;
        }
        Ok(PointerGestureEnd::Cancelled)
    }

    fn pointer_source_live(&self, uid: u64) -> bool {
        let owner = self.docs.iter().find(|d| d.uid == uid).and_then(|d| d.branch_of.as_ref().map(|(main, _)| *main)).unwrap_or(uid);
        [uid, owner].iter().any(|id| {
            let live = self.live_of(*id);
            live.doc_uid.is_some() || live.tab.is_some()
        })
    }

    /// Mutable entry points call this before changing independently owned state.
    pub(crate) fn pointer_external_edit(&mut self) -> Result<()> {
        if !self.pointer_owned
            && let Some(cp) = &self.pointer_checkpoint
        {
            let token = PointerGestureToken { uid: cp.uid, generation: cp.generation };
            self.cancel_pointer_gesture(&token)?;
        }
        Ok(())
    }
    pub(crate) fn pointer_persistence_edit(&mut self) -> Result<()> {
        if self.pointer_owned {
            return Err(disabled("file persistence cannot run inside pointer dispatch"));
        }
        self.pointer_external_edit()
    }
    pub(crate) fn pointer_external_edit_or_notice(&mut self) -> bool {
        match self.pointer_external_edit() {
            Ok(()) => true,
            Err(error) => {
                self.message = Some(error.to_string());
                false
            }
        }
    }
    pub(crate) fn pointer_command_effects(&mut self, id: &str, params: &Value, history: bool) -> bool {
        if !self.pointer_owned {
            return false;
        }
        if let Some(cp) = &mut self.pointer_checkpoint {
            if cp.commands.len() >= MAX_COMMANDS {
                cp.fault = Some("pointer command queue is full".into());
            } else {
                cp.commands.push((id.into(), params.clone()));
            }
            if history && id != "edit.repeat" {
                cp.last_command = Some((id.into(), params.clone()));
            }
        }
        true
    }
    pub(crate) fn pointer_ui_request(&mut self, kind: &str, data: &Value) -> bool {
        if !self.pointer_owned {
            return false;
        }
        if let Some(cp) = &mut self.pointer_checkpoint {
            if cp.requests.len() >= MAX_REQUESTS {
                cp.fault = Some("pointer UI request queue is full".into());
            } else {
                cp.requests.push(UiRequest { kind: kind.into(), data: data.clone() });
            }
        }
        true
    }
    fn append_journal(&mut self, id: String, params: Value) {
        self.journal.push((id, params));
        if self.journal.len() > MAX_COMMANDS {
            self.journal.drain(..1000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipad_pointer_generation_exhaustion_never_wraps() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(generation(&counter).unwrap(), u64::MAX - 1);
        assert!(generation(&counter).is_err());
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
