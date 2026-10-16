//! UI ownership for the opt-in local engine checkpoint. The native observer does
//! not enroll contacts; a complete consumer must serialize input before using it.
use egui::{Context, LayerId, Rect};
use figcraft_doc::NodeId;
use figcraft_engine::{PointerGestureEnd, PointerGestureToken, Result, Session};

use crate::FigApp;

pub(crate) struct Owner {
    token: PointerGestureToken,
    uid: u64,
    page: NodeId,
    rect: Rect,
    modal: Option<LayerId>,
    context: Context,
    comments: crate::comments::PointerUi,
    proto_editor: Option<(NodeId, usize)>,
}

fn clear_owners(ctx: &Context, canvas: &mut crate::canvas::CanvasState, comments: &mut crate::comments::CommentUi) {
    crate::handles::cancel_pointer(ctx);
    crate::guides::cancel_pointer(ctx);
    crate::proto_canvas::cancel_pointer(ctx);
    comments.cancel_pointer();
    canvas.cancel_pointer();
    ctx.stop_dragging();
}

fn cancel_live_owner(
    session: &mut Session,
    owner: &mut Option<Owner>,
    interruption: &mut Option<u64>,
    canvas: &mut crate::canvas::CanvasState,
    comments: &mut crate::comments::CommentUi,
) -> Option<figcraft_engine::EngineError> {
    *interruption = interruption.and_then(|value| value.checked_add(1));
    let owner = owner.take()?;
    clear_owners(&owner.context, canvas, comments);
    match session.cancel_pointer_gesture(&owner.token) {
        Ok(PointerGestureEnd::Cancelled) if session.ds().is_ok_and(|ds| ds.uid == owner.uid && ds.page == owner.page) => {
            comments.restore_pointer_ui(owner.comments);
            crate::proto_panel::open_editor(&owner.context, owner.proto_editor);
            None
        }
        Err(error) => Some(error),
        _ => None,
    }
}

impl FigApp {
    pub(crate) fn poll_live_with_pointer_barrier(&mut self) -> bool {
        let mut error = None;
        let changed = self.session.live_poll_with_barrier(|session| {
            error = cancel_live_owner(session, &mut self.canvas_pointer, &mut self.canvas_interruption, &mut self.canvas, &mut self.ui.comments);
        });
        if let Some(error) = error {
            self.failed("pointer.cancel", &error);
        }
        changed
    }

    pub(crate) fn sync_live_with_pointer_barrier(&mut self) {
        let mut error = None;
        let windows = &self.windows;
        self.session.live_sync_in_with_barrier(
            |uid| uid.map_or(0, |u| windows.of(u)) == windows.current,
            |session| {
                error = cancel_live_owner(session, &mut self.canvas_pointer, &mut self.canvas_interruption, &mut self.canvas, &mut self.ui.comments);
            },
        );
        if let Some(error) = error {
            self.failed("pointer.cancel", &error);
        }
    }

    /// Begin immediately before a local pointer Down. The caller must dispatch
    /// every preview through `pointer_session` and cancel before external input.
    pub fn begin_canvas_pointer(&mut self, ctx: &Context) -> Result<()> {
        self.cancel_canvas_pointer();
        if self.canvas_interruption.is_none() {
            return Err(figcraft_engine::EngineError::Disabled("pointer.gesture".into(), "UI interruption generation exhausted".into()));
        }
        let ds = self.session.ds()?;
        let (uid, page) = (ds.uid, ds.page);
        let token = self.session.begin_pointer_gesture()?;
        self.canvas_pointer = Some(Owner {
            token,
            uid,
            page,
            rect: self.canvas.rect,
            modal: ctx.memory(|m| m.top_modal_layer()),
            context: ctx.clone(),
            comments: self.ui.comments.pointer_ui(),
            proto_editor: crate::proto_panel::editor(ctx),
        });
        Ok(())
    }

    /// Run only synchronous pointer tools and local previews. External commands,
    /// file operations and callbacks must use their ordinary barrier instead.
    pub fn pointer_session<T>(&mut self, operation: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        let result = match self.canvas_pointer.as_ref() {
            Some(owner) => self.session.with_pointer_gesture(&owner.token, operation),
            None => operation(&mut self.session),
        };
        if result.is_err() {
            self.cancel_canvas_pointer();
        }
        result
    }

    /// Remove application owners before an inserted reset Up enters egui. This
    /// also clears panel-started owners when no canvas checkpoint was enrolled.
    pub fn cancel_pointer_owners(&mut self, ctx: &Context) {
        clear_owners(ctx, &mut self.canvas, &mut self.ui.comments);
    }

    /// Restore the initiating editing state without Escape, ending composition,
    /// changing field focus or selecting the source tab. Unenrolled routes do
    /// nothing. A stale checkpoint reports its error and keeps independent data.
    pub fn cancel_canvas_pointer(&mut self) -> bool {
        self.canvas_interruption = self.canvas_interruption.and_then(|value| value.checked_add(1));
        let Some(owner) = self.canvas_pointer.take() else { return false };
        self.cancel_pointer_owners(&owner.context);
        match self.session.cancel_pointer_gesture(&owner.token) {
            Ok(PointerGestureEnd::Cancelled) if self.session.ds().is_ok_and(|ds| ds.uid == owner.uid && ds.page == owner.page) => {
                self.ui.comments.restore_pointer_ui(owner.comments);
                crate::proto_panel::open_editor(&owner.context, owner.proto_editor);
            }
            Err(error) => self.failed("pointer.cancel", &error),
            _ => {}
        }
        true
    }

    /// Finish only after the accepted Up. Deferred engine effects are then safe
    /// to route to panels and platform services.
    pub fn finish_canvas_pointer(&mut self) -> Result<PointerGestureEnd> {
        let Some(owner) = self.canvas_pointer.take() else { return Ok(PointerGestureEnd::Inactive) };
        let result = self.session.finish_pointer_gesture(&owner.token);
        self.cancel_pointer_owners(&owner.context);
        if matches!(result, Ok(PointerGestureEnd::Finished)) {
            self.handle_ui_requests();
        }
        result
    }

    /// Queue a lifecycle cancellation when the editor is already borrowed.
    pub fn request_canvas_pointer_cancel(ctx: &Context) {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("canvas_pointer_cancel_requested"), true));
        ctx.request_repaint();
    }

    /// Validate source/layout and ownership before logic or any release handler.
    pub fn canvas_pointer_preflight(&mut self, ctx: &Context) {
        let requested = ctx.data_mut(|data| data.remove_temp::<bool>(egui::Id::new("canvas_pointer_cancel_requested"))).unwrap_or(false);
        let invalid = self.canvas_pointer.as_ref().is_some_and(|owner| {
            !ctx.input(|i| i.focused)
                || ctx.memory(|m| m.top_modal_layer()) != owner.modal
                || self.canvas.rect != owner.rect
                || !self.session.ds().is_ok_and(|ds| ds.uid == owner.uid && ds.page == owner.page)
        });
        if requested || invalid {
            self.cancel_canvas_pointer();
        }
        if self.canvas_pointer.is_some()
            && let Err(error) = self.pointer_session(|_| Ok(()))
        {
            self.failed("pointer.preflight", &error);
        }
    }

    pub(crate) fn pointer_command(&mut self, id: &str, params: serde_json::Value) -> Option<serde_json::Value> {
        if self.canvas_pointer.is_none() {
            return self.run(id, params);
        }
        let result = self.pointer_session(|session| session.execute(id, &params));
        result.map_err(|error| self.failed(id, &error)).ok()
    }
}

#[cfg(test)]
#[path = "canvas_pointer_tests.rs"]
pub(crate) mod tests;
