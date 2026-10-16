//! Reviewed main-thread UIKit copy-picker boundary. See execution decision D07.
#![deny(unsafe_code)]
#[cfg(any(target_os = "ios", test))]
mod callback;
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod native;
#[cfg(target_os = "ios")]
pub use native::{Completion, Host, Operation, Outcome, PresentationError};
