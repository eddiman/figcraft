//! One revocable native owner shared by fields, canvas and explicit Paste requests.
use figcraft_apple_ipad_clipboard::{Error, NamedHost, snapshot};
use std::sync::Arc;

pub struct Clipboard {
    _owner: NamedHost,
}
fn write_error(error: Error) -> egui::TextClipboardError {
    match error {
        Error::TooLarge => egui::TextClipboardError::TooLarge,
        Error::Panicked => egui::TextClipboardError::Panicked,
        Error::InvalidImage | Error::InvalidToken => egui::TextClipboardError::InvalidContent,
        _ => egui::TextClipboardError::Unavailable,
    }
}
impl Clipboard {
    pub fn install(ctx: &egui::Context) -> Result<Self, String> {
        let owner = NamedHost::general().map_err(|error| format!("Clipboard setup failed: {error:?}"))?;
        let writer = owner.writer();
        ctx.set_receipt_clipboard_writer(Some(Arc::new(move |text| {
            let receipt = writer.write_text(text).map_err(write_error)?;
            Ok(Some(egui::PasteIdentity {
                token: receipt.token,
                generation: u64::try_from(receipt.change_count).map_err(|_| egui::TextClipboardError::Unavailable)?,
            }))
        })));
        let writer = owner.writer();
        ctx.set_image_clipboard_writer(Some(Arc::new(move |image| {
            writer.write_image(snapshot::RgbaImage { width: image.width, height: image.height, bytes: image.rgba }).map(|_| ()).map_err(write_error)
        })));
        let reader = owner.writer();
        ctx.set_paste_reader(Some(Arc::new(move || match reader.read_owned() {
            snapshot::Outcome::Empty => egui::PasteOutcome::Empty,
            snapshot::Outcome::Unsupported => egui::PasteOutcome::Unsupported,
            snapshot::Outcome::Content(content) => egui::PasteOutcome::Content(egui::PasteContent {
                text: content.text,
                image: content.image.map(|image| egui::PasteImage { width: image.width, height: image.height, rgba: image.bytes }),
                identity: content.identity.map(|identity| egui::PasteIdentity { token: identity.token, generation: identity.generation }),
            }),
            snapshot::Outcome::Failed(error) => egui::PasteOutcome::Failed(match error {
                snapshot::SnapshotError::Denied => egui::PasteError::Denied,
                snapshot::SnapshotError::Changed => egui::PasteError::Changed,
                snapshot::SnapshotError::TooLarge => egui::PasteError::TooLarge,
                snapshot::SnapshotError::Panicked => egui::PasteError::Panicked,
                snapshot::SnapshotError::Reentrant => egui::PasteError::Reentrant,
                snapshot::SnapshotError::Unavailable => egui::PasteError::Unavailable,
                _ => egui::PasteError::InvalidContent,
            }),
        })));
        Ok(Self { _owner: owner })
    }
}
