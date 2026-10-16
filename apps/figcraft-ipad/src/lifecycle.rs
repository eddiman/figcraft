//! Main-thread adapter; native code retains no Session or editor reference.
use crate::recovery::{Shared, try_with};
use figcraft_apple_ipad::lifecycle::{Callback, Event, Progress};
use figcraft_engine::Session;

pub fn callback(shared: Shared, context: egui::Context) -> Callback {
    std::sync::Arc::new(move |event| {
        let progress = try_with(&shared, |coordinator| {
            if coordinator.needs_session(&event) {
                figcraft_ui_egui::windows::with_app(|fig| coordinator.handle(event.clone(), Some(&fig.session)))
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
