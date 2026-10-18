use crate::{Error, native};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct State {
    name: String,
    live: AtomicBool,
}
/// Owns a revocable named or general pasteboard adapter on the creating main thread.
pub struct NamedHost {
    state: Arc<State>,
    _main_only: PhantomData<Rc<()>>,
}
/// Contains only a weak Rust lease. No UIKit or Context object is captured.
#[derive(Clone)]
pub struct NamedWriter {
    state: Weak<State>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteReceipt {
    pub change_count: isize,
    pub token: String,
}
#[derive(Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    pub change_count: isize,
    pub text: Option<String>,
    pub unsupported: bool,
    pub token: Option<String>,
    pub image: Option<crate::snapshot::RgbaImage>,
}
impl NamedHost {
    /// Own a revocable adapter to the general board without reading or clearing it.
    pub fn general() -> Result<Self, Error> {
        let _main = objc2::MainThreadMarker::new().ok_or(Error::WrongThread)?;
        Ok(Self { state: Arc::new(State { name: native::GENERAL_HOST.into(), live: AtomicBool::new(true) }), _main_only: PhantomData })
    }
    pub fn new() -> Result<Self, Error> {
        let sequence = NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| Error::Unavailable)?;
        let name = format!("org.figcraft.t13-writer-probe.{}.{}", std::process::id(), sequence);
        native::open_named(&name)?;
        Ok(Self { state: Arc::new(State { name, live: AtomicBool::new(true) }), _main_only: PhantomData })
    }
    pub fn writer(&self) -> NamedWriter {
        NamedWriter { state: Arc::downgrade(&self.state) }
    }
    pub fn snapshot(&self) -> Result<TextSnapshot, Error> {
        if !self.state.live.load(Ordering::Acquire) {
            return Err(Error::Expired);
        }
        native::read_named(&self.state.name)
    }
    pub fn revoke(&mut self) {
        if self.state.live.swap(false, Ordering::AcqRel) {
            native::remove_named(&self.state.name);
        }
    }
}
impl Drop for NamedHost {
    fn drop(&mut self) {
        self.revoke();
    }
}
impl NamedWriter {
    /// One user-initiated read, returned as owned data with explicit failure.
    pub fn read_owned(&self) -> crate::snapshot::Outcome {
        use crate::snapshot::{Content, Identity, Outcome, SnapshotError};
        match self.snapshot() {
            Ok(snapshot) => {
                if snapshot.unsupported {
                    return Outcome::Unsupported;
                }
                if snapshot.text.is_none() && snapshot.image.is_none() {
                    return Outcome::Empty;
                }
                let generation = match u64::try_from(snapshot.change_count) {
                    Ok(value) => value,
                    Err(_) => return Outcome::Failed(SnapshotError::Unavailable),
                };
                let content =
                    Content { text: snapshot.text, image: snapshot.image, identity: snapshot.token.map(|token| Identity { token, generation }) };
                match content.validate() {
                    Ok(()) => Outcome::Content(content),
                    Err(error) => Outcome::Failed(error),
                }
            }
            Err(error) => Outcome::Failed(match error {
                Error::Changed => SnapshotError::Changed,
                Error::TooLarge => SnapshotError::TooLarge,
                Error::InvalidToken => SnapshotError::InvalidToken,
                Error::InvalidImage => SnapshotError::InvalidImage,
                Error::Panicked => SnapshotError::Panicked,
                _ => SnapshotError::Unavailable,
            }),
        }
    }
    pub fn snapshot(&self) -> Result<TextSnapshot, Error> {
        let state = self.state.upgrade().ok_or(Error::Expired)?;
        if !state.live.load(Ordering::Acquire) {
            return Err(Error::Expired);
        }
        native::read_named(&state.name)
    }
    pub fn write_text(&self, text: String) -> Result<WriteReceipt, Error> {
        let state = self.state.upgrade().ok_or(Error::Expired)?;
        if !state.live.load(Ordering::Acquire) {
            return Err(Error::Expired);
        }
        native::write_named(&state.name, text)
    }
    pub fn write_image(&self, image: crate::snapshot::RgbaImage) -> Result<WriteReceipt, Error> {
        let state = self.state.upgrade().ok_or(Error::Expired)?;
        if !state.live.load(Ordering::Acquire) {
            return Err(Error::Expired);
        }
        native::write_image_named(&state.name, image)
    }
}
