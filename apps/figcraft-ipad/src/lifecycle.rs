//! Main-thread adapter; native code retains no Session or editor reference.
use crate::recovery::{Shared, try_with};
use figcraft_apple_ipad::lifecycle::{Callback, Event, Progress};
use figcraft_engine::Session;

pub fn callback(shared: Shared, context: egui::Context) -> Callback {
    std::sync::Arc::new(move |event| {
        if matches!(event, Event::Checkpoint { .. } | Event::Terminating) {
            // If the editor is already borrowed by a frame, its next preflight
            // consumes this value before it dispatches another edit.
            figcraft_ui_egui::FigApp::request_canvas_pointer_cancel(&context);
        }
        let progress = try_with(&shared, |coordinator| {
            if coordinator.needs_session(&event) {
                figcraft_ui_egui::windows::with_app(|fig| {
                    fig.cancel_canvas_pointer();
                    coordinator.handle(event.clone(), Some(&fig.session))
                })
                .unwrap_or_else(|| coordinator.handle(event.clone(), None))
            } else {
                coordinator.handle(event.clone(), None)
            }
        })
        .unwrap_or(Progress::Pending);
        if matches!(event, Event::Resumed | Event::Finished { .. }) {
            context.request_repaint();
        }
        progress
    })
}
pub fn emergency(shared: &Shared, session: &Session) {
    if let Some(Progress::Failed(error)) = try_with(shared, |coordinator| coordinator.emergency(session)) {
        log::warn!("Emergency recovery could not start: {error}");
    }
}
