//! D13 owned clipboard boundary. General-board access requires an explicit host.
#![deny(unsafe_code)]
pub mod image_data;
mod image_profile;
pub mod snapshot;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    WrongThread,
    Expired,
    Inactive,
    Unavailable,
    Changed,
    TooLarge,
    ReadbackMismatch,
    InvalidToken,
    InvalidImage,
    Panicked,
}
#[derive(Debug)]
pub struct Report {
    pub passed: Vec<&'static str>,
    pub change_counts: Vec<isize>,
}
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod native;
#[cfg(target_os = "ios")]
pub use native::run_named_cases;

#[cfg(target_os = "ios")]
mod owner;
#[cfg(target_os = "ios")]
pub use owner::{NamedHost, NamedWriter, TextSnapshot, WriteReceipt};
