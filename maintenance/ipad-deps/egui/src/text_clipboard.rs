//! Optional synchronous text clipboard writes for an integration that can report failure.
use std::sync::Arc;

/// Maximum UTF-8 byte length accepted by the optional synchronous writer.
pub const TEXT_CLIPBOARD_LIMIT: usize = 8 * 1024 * 1024;

/// A synchronous clipboard write did not complete successfully.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextClipboardError {
    /// No writer is installed, or its platform/thread/lifetime is unavailable.
    Unavailable,
    /// The platform or integration denied the operation.
    Denied,
    /// The owned text exceeds the configured size bound.
    TooLarge,
    /// The callback panicked; no deletion should follow this result.
    Panicked,
    /// Another write is already executing in this context.
    Reentrant,
}
impl std::fmt::Display for TextClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Clipboard is unavailable",
            Self::Denied => "Clipboard write was denied",
            Self::TooLarge => "Clipboard text is too large",
            Self::Panicked => "Clipboard writer failed",
            Self::Reentrant => "Clipboard writer is already active",
        })
    }
}
impl std::error::Error for TextClipboardError {}

/// An owned callback invoked synchronously without the Context lock held.
///
/// Native implementations must check their UI thread and lifetime themselves.
/// They must not capture borrowed application state or native objects that may
/// be accessed from a different thread. Avoid a strong capture of the owning
/// Context, which would form a reference cycle; remove such a callback before
/// dropping its integration.
pub type TextClipboardWriter =
    Arc<dyn Fn(String) -> Result<(), TextClipboardError> + Send + Sync>;

/// The same confirmed write, optionally acknowledging a native copy identity.
#[cfg(feature = "figcraft-ipad-clipboard")]
pub type ReceiptClipboardWriter = Arc<
    dyn Fn(String) -> Result<Option<crate::PasteIdentity>, TextClipboardError> + Send + Sync,
>;
/// Owned straight-RGBA writer with synchronous acknowledgement.
#[cfg(feature = "figcraft-ipad-clipboard")]
pub type ImageClipboardWriter = Arc<dyn Fn(crate::PasteImage)->Result<(),TextClipboardError>+Send+Sync>;
