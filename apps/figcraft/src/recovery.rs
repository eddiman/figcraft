//! Desktop adapter for the host-neutral recovery implementation.
pub use figcraft_recovery::{Recovery, Shared, lock, recover_all};
#[cfg(test)]
pub fn recover(root: &std::path::Path, session: &mut figcraft_engine::Session) -> usize {
    recover_all(root, session).copies
}
