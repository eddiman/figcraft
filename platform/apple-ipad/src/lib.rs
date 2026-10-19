//! Reviewed main-thread UIKit copy-picker and recovery lifecycle boundaries (D07/D11).
#![deny(unsafe_code)]
#[cfg(target_os = "ios")]
mod geometry;
pub mod lifecycle;
#[cfg(target_os = "ios")]
pub use geometry::keyboard_bounds;
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod lifecycle_native;
#[cfg(target_os = "ios")]
pub use lifecycle_native::LifecycleHost;
#[cfg(any(target_os = "ios", test))]
mod callback;
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod native;
#[cfg(target_os = "ios")]
pub use native::{Completion, Host, Operation, Outcome, PresentationError};
