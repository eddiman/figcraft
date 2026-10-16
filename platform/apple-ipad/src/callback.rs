//! Contain Rust failures before a native callback returns through Objective-C.
pub(crate) fn contain<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| Err("native picker callback panicked".into()))
}
#[cfg(test)]
mod tests {
    use super::contain;
    #[test]
    fn ipad_native_callback_returns_success() {
        assert_eq!(contain(|| Ok(7)), Ok(7));
    }
    #[test]
    fn ipad_native_callback_preserves_failure() {
        assert_eq!(contain::<()>(|| Err("denied".into())), Err("denied".into()));
    }
    #[test]
    #[allow(clippy::panic)]
    fn ipad_native_callback_panic_unwinds_owned_guards() {
        use std::{cell::Cell, rc::Rc};
        struct Guard(Rc<Cell<bool>>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let dropped = Rc::new(Cell::new(false));
        let result = contain::<()>(|| {
            let _guard = Guard(dropped.clone());
            panic!("synthetic callback failure");
        });
        assert_eq!(result, Err("native picker callback panicked".into()));
        assert!(dropped.get());
    }
}
