//! Safe host coordination. Native callbacks never join unfinished checkpoint workers.
use figcraft_apple_ipad::lifecycle::{End, Event, Progress};
use figcraft_engine::Session;
use figcraft_recovery::{Durability, FlushStatus, FlushTicket, Recovery, recover_all};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, TryLockError};

pub type Shared = Arc<Mutex<Coordinator>>;
#[cfg(target_os = "ios")]
pub fn support_dir() -> std::io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(|path| PathBuf::from(path).join("Library/Application Support/FigCraft"))
        .ok_or_else(|| std::io::Error::other("application home is unavailable"))
}
pub fn try_with<T>(shared: &Shared, f: impl FnOnce(&mut Coordinator) -> T) -> Option<T> {
    let mut owner = match shared.try_lock() {
        Ok(owner) => owner,
        Err(TryLockError::WouldBlock) => return None,
        Err(TryLockError::Poisoned(error)) => {
            shared.clear_poison();
            error.into_inner()
        }
    };
    Some(f(&mut owner))
}
pub struct Coordinator {
    recovery: Recovery,
    cycle: Option<u64>,
    ticket: Option<FlushTicket>,
    notice: Option<String>,
    restored: usize,
}
impl Coordinator {
    pub fn open(root: &Path, session: &mut Session) -> std::io::Result<Self> {
        let recovered = recover_all(root, session);
        let mut recovery = Recovery::start_with(root, Durability::Synced)?;
        recovery.adopt(recovered.waiting);
        recovery.write(session, false);
        Ok(Self { recovery, cycle: None, ticket: None, notice: None, restored: recovered.copies })
    }
    pub fn restored(&self) -> usize {
        self.restored
    }
    pub fn tick(&mut self, session: &Session, now: f64) -> Option<f64> {
        self.recovery.tick(session, now)
    }
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }
    pub fn needs_session(&self, event: &Event) -> bool {
        match event {
            Event::Checkpoint { cycle } => self.cycle != Some(*cycle) || self.ticket.is_none(),
            Event::Terminating => true,
            _ => false,
        }
    }
    pub fn handle(&mut self, event: Event, session: Option<&Session>) -> Progress {
        match event {
            Event::Checkpoint { cycle } => {
                if self.cycle != Some(cycle) {
                    self.cycle = Some(cycle);
                    self.ticket = None;
                }
                if self.ticket.is_none() {
                    let Some(session) = session else { return Progress::Pending };
                    match self.recovery.begin_flush(session) {
                        Ok(ticket) => self.ticket = Some(ticket),
                        Err(error) => return Progress::Failed(error),
                    }
                }
                match self.ticket.as_ref().map(|t| self.recovery.poll_flush(t)) {
                    Some(FlushStatus::Complete) => Progress::Complete,
                    Some(FlushStatus::Failed(error)) => Progress::Failed(error),
                    _ => Progress::Pending,
                }
            }
            Event::Finished { cycle, outcome } => {
                if self.cycle == Some(cycle) {
                    self.ticket = None;
                    self.cycle = None;
                }
                let notice = match outcome {
                    End::Deadline | End::Expired => {
                        Some("Recovery could not finish while the app was inactive. Existing recovery copies were kept.".to_string())
                    }
                    End::Failed(error) => Some(format!("Recovery copy could not be updated: {error}")),
                    End::Complete | End::Resumed | End::Closed => None,
                };
                if let Some(notice) = notice {
                    log::warn!("{notice}");
                    self.notice = Some(notice);
                }
                Progress::Pending
            }
            Event::Resumed => Progress::Pending,
            Event::Terminating => session.map_or(Progress::Pending, |session| self.emergency(session)),
        }
    }
    pub fn emergency(&mut self, session: &Session) -> Progress {
        self.ticket = None;
        self.cycle = None;
        match self.recovery.begin_flush(session) {
            Ok(ticket) => match self.recovery.poll_flush(&ticket) {
                FlushStatus::Complete => Progress::Complete,
                FlushStatus::Failed(error) => Progress::Failed(error),
                FlushStatus::Pending => Progress::Pending,
            },
            Err(error) => Progress::Failed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, Instant};
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("figcraft-ipad-lifecycle-{}-{id}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            Self(root)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn edited() -> Session {
        let mut s = Session::new();
        s.execute("shape.rectangle", &json!({"x":0,"y":0,"width":10,"height":10})).unwrap();
        s
    }
    fn flush(c: &mut Coordinator, s: &Session, cycle: u64) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match c.handle(Event::Checkpoint { cycle }, Some(s)) {
                Progress::Complete => return,
                Progress::Failed(error) => panic!("{error}"),
                Progress::Pending => {}
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn ipad_lifecycle_retries_capture_without_a_ui_borrow() {
        let root = Root::new();
        let s = edited();
        let mut empty = Session::empty();
        let mut c = Coordinator::open(&root.0, &mut empty).unwrap();
        assert_eq!(c.handle(Event::Checkpoint { cycle: 1 }, None), Progress::Pending);
        assert!(c.needs_session(&Event::Checkpoint { cycle: 1 }));
        flush(&mut c, &s, 1);
        assert!(!c.needs_session(&Event::Checkpoint { cycle: 1 }));
    }
    #[test]
    fn ipad_startup_adopts_the_complete_checkpoint_and_drop_keeps_it() {
        let root = Root::new();
        let mut s = edited();
        s.execute("file.saveVersion", &json!({"name":"Before suspension"})).unwrap();
        let mut c = Coordinator::open(&root.0, &mut Session::empty()).unwrap();
        flush(&mut c, &s, 1);
        drop(c);
        let mut next = Session::empty();
        let mut c = Coordinator::open(&root.0, &mut next).unwrap();
        assert_eq!(c.restored(), 1);
        assert_eq!(next.docs[0].versions[0].name, "Before suspension");
        assert!(next.docs[0].is_dirty() && next.docs[0].path.is_none());
        flush(&mut c, &next, 2);
        c.tick(&next, 0.0);
        assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1, "successful replacement releases the previous run");
    }
    #[test]
    fn ipad_expiration_preserves_previous_checkpoint_and_resumes_new_work() {
        let root = Root::new();
        let mut s = edited();
        let mut c = Coordinator::open(&root.0, &mut Session::empty()).unwrap();
        flush(&mut c, &s, 1);
        c.handle(Event::Finished { cycle: 1, outcome: End::Complete }, None);
        s.execute("file.rename", &json!({"name":"After resume"})).unwrap();
        c.handle(Event::Checkpoint { cycle: 2 }, Some(&s));
        c.handle(Event::Finished { cycle: 2, outcome: End::Expired }, None);
        assert!(c.take_notice().is_some());
        c.handle(Event::Resumed, None);
        flush(&mut c, &s, 3);
        drop(c);
        let mut next = Session::empty();
        let recovered = recover_all(&root.0, &mut next);
        assert_eq!(recovered.copies, 1);
        assert_eq!(next.docs[0].doc.name, "After resume");
    }
    #[test]
    fn ipad_termination_starts_a_copy_and_host_drop_keeps_worker_ownership() {
        let root = Root::new();
        let s = edited();
        let mut c = Coordinator::open(&root.0, &mut Session::empty()).unwrap();
        assert!(!matches!(c.handle(Event::Terminating, Some(&s)), Progress::Failed(_)));
        drop(c);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut next = Session::empty();
        loop {
            let recovered = recover_all(&root.0, &mut next);
            if recovered.copies > 0 {
                assert_eq!(recovered.copies, 1);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(next.doc().unwrap().node(next.doc().unwrap().pages[0]).unwrap().children.len(), 1);
    }
    #[test]
    fn ipad_busy_recovery_lock_never_waits_on_a_reentrant_callback() {
        let root = Root::new();
        let shared = Arc::new(Mutex::new(Coordinator::open(&root.0, &mut Session::empty()).unwrap()));
        let _guard = shared.lock().unwrap();
        assert!(try_with(&shared, |_| ()).is_none());
    }
    #[test]
    fn ipad_checkpoint_failure_is_reported_and_the_prior_generation_stays() {
        let root = Root::new();
        let mut s = edited();
        let mut c = Coordinator::open(&root.0, &mut Session::empty()).unwrap();
        flush(&mut c, &s, 1);
        let run = c.recovery.directory().to_path_buf();
        let slot = run.join(s.docs[0].uid.to_string());
        let current = std::fs::read(slot.join("current")).unwrap();
        s.execute("file.saveVersion", &json!({"name":"Invalid metadata"})).unwrap();
        s.ds_mut().unwrap().versions[0].id = "../invalid".into();
        c.handle(Event::Finished { cycle: 1, outcome: End::Complete }, None);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let progress = c.handle(Event::Checkpoint { cycle: 2 }, Some(&s));
            if let Progress::Failed(error) = progress {
                c.handle(Event::Finished { cycle: 2, outcome: End::Failed(error) }, None);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(std::fs::read(slot.join("current")).unwrap(), current);
        assert!(c.take_notice().is_some());
    }
}
