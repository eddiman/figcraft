//! Complete-file checkpoints shared by desktop and iPad hosts. Normal ticks never
//! wait for an unfinished worker. Emergency desktop writes may explicitly drain it.
mod storage;
use figcraft_engine::{
    Session,
    persistence::{RecoverySnapshot, capture_recovery},
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use storage::{CheckpointRef, Hooks, LOCK};

pub const INTERVAL: f64 = 10.0;
pub type Shared = Arc<Mutex<Option<Recovery>>>;
pub fn lock(recovery: &Shared) -> MutexGuard<'_, Option<Recovery>> {
    recovery.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Filesystem assurance selected by the host. Neither policy qualifies physical
/// power loss or application suspension; those require host/device evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    /// Preserve desktop atomic replacement without adding a filesystem sync claim.
    Atomic,
    /// Sync members and directories before reporting a publication as complete.
    Synced,
}
#[derive(Clone)]
struct Payload {
    snapshot: RecoverySnapshot,
    online: Option<String>,
}
impl Payload {
    fn same(&self, other: &Self) -> bool {
        self.online == other.online && self.snapshot.same_capture(&other.snapshot)
    }
}
#[derive(Clone)]
enum Intent {
    Keep(Payload),
    Delete,
}
impl Intent {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Delete, Self::Delete) => true,
            (Self::Keep(a), Self::Keep(b)) => a.same(b),
            _ => false,
        }
    }
}
struct Flight {
    payload: Payload,
    handle: JoinHandle<Result<(), String>>,
}
struct Copy {
    desired: Intent,
    done: Option<Payload>,
    flight: Option<Flight>,
    error: Option<String>,
}
impl Copy {
    fn reap(&mut self, wait: bool) {
        if self.flight.as_ref().is_some_and(|f| wait || f.handle.is_finished())
            && let Some(f) = self.flight.take()
        {
            match f.handle.join().unwrap_or_else(|_| Err("recovery worker panicked".into())) {
                Ok(()) => {
                    self.done = Some(f.payload);
                    self.error = None;
                }
                Err(e) => {
                    self.done = None;
                    // Failure belongs to this flight. A newer queued capture
                    // still needs its own publication attempt.
                    self.error = match &self.desired {
                        Intent::Keep(wanted) if wanted.same(&f.payload) => Some(e),
                        _ => None,
                    };
                }
            }
        }
    }
    fn complete(&self, payload: &Payload) -> bool {
        self.flight.is_none() && self.error.is_none() && self.done.as_ref().is_some_and(|d| d.same(payload))
    }
}
/// A snapshot requirement, independent of later UI edits and Session borrows.
pub struct FlushTicket {
    targets: BTreeMap<u64, Intent>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum FlushStatus {
    Pending,
    Complete,
    Failed(String),
}
/// A source from a previous run stays owned until its replacement is checkpointed.
pub struct Waiting {
    session: Option<u64>,
    source: CheckpointRef,
    run: Option<PathBuf>,
    run_lock: Option<Arc<File>>,
    retain: bool,
}
#[derive(Default)]
pub struct Recovered {
    pub copies: usize,
    pub waiting: Vec<Waiting>,
}
pub struct Recovery {
    dir: PathBuf,
    run_lock: Arc<File>,
    written: BTreeMap<u64, Copy>,
    next: f64,
    waiting: Vec<Waiting>,
    policy: Durability,
    hooks: Hooks,
    capture_errors: BTreeMap<u64, String>,
}
impl Recovery {
    pub fn start(root: &Path) -> Option<Self> {
        Self::start_with(root, Durability::Atomic).ok()
    }
    pub fn start_with(root: &Path, policy: Durability) -> std::io::Result<Self> {
        storage::root(root, policy)?;
        let dir = storage::unique(root, "run")?;
        let file = File::options().write(true).create_new(true).open(dir.join(LOCK))?;
        file.try_lock().map_err(std::io::Error::other)?;
        if policy == Durability::Synced {
            File::open(root)?.sync_all()?;
        }
        Ok(Self {
            dir,
            run_lock: Arc::new(file),
            written: BTreeMap::new(),
            next: 0.0,
            waiting: Vec::new(),
            policy,
            hooks: Hooks::default(),
            capture_errors: BTreeMap::new(),
        })
    }
    pub fn directory(&self) -> &Path {
        &self.dir
    }
    pub fn adopt(&mut self, waiting: Vec<Waiting>) {
        self.waiting.extend(waiting);
    }
    fn refresh(&mut self, s: &Session) {
        let mut needed = BTreeSet::new();
        self.capture_errors.clear();
        for d in s.docs.iter().filter(|d| d.branch_of.is_none() && s.file_needs_recovery(d.uid)) {
            needed.insert(d.uid);
            match capture_recovery(s, d.uid) {
                Ok(snapshot) => {
                    let online = s.online_of(d.uid).map(|(server, file, at)| format!("{server}\n{file}\n{} {} {}", at.session, at.client_id, at.seq));
                    let payload = Payload { snapshot, online };
                    let slot = self.written.entry(d.uid).or_insert_with(|| Copy { desired: Intent::Delete, done: None, flight: None, error: None });
                    if !slot.desired.same(&Intent::Keep(payload.clone())) {
                        slot.error = None;
                    }
                    slot.desired = Intent::Keep(payload);
                }
                Err(e) => {
                    self.capture_errors.insert(d.uid, e.to_string());
                }
            }
        }
        for (uid, slot) in &mut self.written {
            if !needed.contains(uid) {
                slot.desired = Intent::Delete;
            }
        }
    }
    fn drive(&mut self, wait: bool, start: bool, retry: bool) {
        let mut removed = Vec::new();
        for (&uid, slot) in &mut self.written {
            slot.reap(wait);
            if slot.flight.is_some() {
                continue;
            }
            match slot.desired.clone() {
                Intent::Delete => match storage::delete(&self.dir, uid) {
                    Ok(()) => removed.push(uid),
                    Err(e) => {
                        // Cleanup can fail after removing some checkpoint files.
                        slot.done = None;
                        slot.error = Some(e.to_string());
                    }
                },
                Intent::Keep(payload)
                    if !self.capture_errors.contains_key(&uid) && start && !slot.complete(&payload) && (retry || slot.error.is_none()) =>
                {
                    slot.error = None;
                    if wait {
                        match storage::publish(&self.dir, uid, &payload, self.policy, &self.hooks) {
                            Ok(()) => slot.done = Some(payload),
                            Err(e) => {
                                slot.done = None;
                                slot.error = Some(e);
                            }
                        }
                    } else {
                        let (run, owner, policy, hooks, job_payload) =
                            (self.dir.clone(), self.run_lock.clone(), self.policy, self.hooks.clone(), payload.clone());
                        match std::thread::Builder::new().name("recovery".into()).spawn(move || {
                            let _owner = owner;
                            storage::publish(&run, uid, &job_payload, policy, &hooks)
                        }) {
                            Ok(handle) => slot.flight = Some(Flight { payload, handle }),
                            Err(e) => slot.error = Some(e.to_string()),
                        }
                    }
                }
                _ => {}
            }
        }
        for uid in removed {
            self.written.remove(&uid);
        }
    }
    fn covered(&self) -> bool {
        self.capture_errors.is_empty()
            && self.written.values().all(|slot| match &slot.desired {
                Intent::Keep(p) => slot.complete(p),
                Intent::Delete => false,
            })
    }
    fn settle(&mut self, s: &Session) {
        if !self.covered() {
            return;
        }
        let mut retained = Vec::new();
        for w in std::mem::take(&mut self.waiting) {
            if w.retain || w.session.is_some_and(|id| s.live.recovering(id)) || w.source.remove().is_err() {
                retained.push(w);
                continue;
            }
            if let (Some(run), Some(owner)) = (w.run, w.run_lock) {
                storage::remove_empty_run(&run, owner);
            }
        }
        self.waiting = retained;
    }
    /// Emergency compatibility path: `wait=true` drains an earlier job before
    /// publishing the newest requested state. UI/lifecycle paths use flush tickets.
    pub fn write(&mut self, s: &Session, wait: bool) {
        self.refresh(s);
        self.drive(wait, true, true);
        self.settle(s);
    }
    pub fn tick(&mut self, s: &Session, now: f64) -> Option<f64> {
        let now = if now.is_finite() { now } else { self.next };
        self.refresh(s);
        let due = now >= self.next;
        self.drive(false, due, due);
        if due {
            self.next = now + INTERVAL;
        }
        self.settle(s);
        if self.written.values().any(|slot| slot.flight.is_some() || matches!(slot.desired, Intent::Delete)) {
            Some(0.05)
        } else if !self.waiting.is_empty() {
            Some(0.5)
        } else if !self.covered() {
            Some((self.next - now).max(0.05))
        } else {
            None
        }
    }
    /// Capture current requirements and start their workers. The host owns the
    /// deadline and may drop the ticket while workers retain their resources.
    pub fn begin_flush(&mut self, s: &Session) -> Result<FlushTicket, String> {
        self.refresh(s);
        if let Some(e) = self.capture_errors.values().next() {
            return Err(e.clone());
        }
        let ticket = FlushTicket { targets: self.written.iter().map(|(&uid, slot)| (uid, slot.desired.clone())).collect() };
        self.drive(false, true, true);
        Ok(ticket)
    }
    /// Reap finished jobs without waiting for unfinished ones. A later request
    /// that supersedes a required capture fails this ticket rather than certifying
    /// a different checkpoint. Starting another flush can retry failed writes.
    pub fn poll_flush(&mut self, ticket: &FlushTicket) -> FlushStatus {
        self.drive(false, true, false);
        let mut pending = false;
        for (&uid, wanted) in &ticket.targets {
            match (wanted, self.written.get(&uid)) {
                (Intent::Delete, None) => {}
                (Intent::Keep(p), Some(slot)) if wanted.same(&slot.desired) && slot.complete(p) => {}
                (_, Some(slot)) if !wanted.same(&slot.desired) => return FlushStatus::Failed("checkpoint was superseded".into()),
                (_, Some(slot)) if slot.error.is_some() => return FlushStatus::Failed(slot.error.clone().unwrap_or_default()),
                (_, Some(_)) => pending = true,
                _ => return FlushStatus::Failed("checkpoint is no longer requested".into()),
            }
        }
        if pending { FlushStatus::Pending } else { FlushStatus::Complete }
    }
    /// Normal intentional quit discards this run only after every worker ends.
    /// Dropping Recovery without finish leaves checkpoints and worker-held locks.
    pub fn finish(mut self) {
        for slot in self.written.values_mut() {
            slot.reap(true);
        }
        let dir = self.dir.clone();
        drop(self.run_lock);
        let _ = std::fs::remove_dir_all(dir);
    }
}
fn rejoin(source: &CheckpointRef, s: &mut Session) -> Option<u64> {
    let text = source.online.as_ref()?;
    let mut lines = text.lines();
    let (server, file, at) = (lines.next()?, lines.next()?, lines.next()?);
    let nums = at.split_whitespace().map(str::parse::<u64>).collect::<Result<Vec<_>, _>>().ok()?;
    let [session, client, seq] = nums.as_slice() else { return None };
    let params = json!({"path":source.archive.to_string_lossy(),"server":server,"file":file,"session":session,"client":client,"seq":seq});
    s.execute("collab.recoverFile", &params).ok()?.get("session")?.as_u64()
}
fn open_source(source: CheckpointRef, run: Option<PathBuf>, owner: Option<Arc<File>>, s: &mut Session, out: &mut Recovered) {
    // A partially repaired copy may be opened, but its original remains available.
    let retain = source.retain
        || std::fs::read(&source.archive)
            .ok()
            .and_then(|b| figcraft_format::load_bytes_with_versions(&b).ok())
            .is_some_and(|loaded| !loaded.issues.is_empty());
    if let Some(id) = rejoin(&source, s) {
        out.waiting.push(Waiting { session: Some(id), source, run, run_lock: owner, retain });
    } else if s.execute("file.recover", &json!({"path":source.archive.to_string_lossy()})).is_ok() {
        out.copies += 1;
        out.waiting.push(Waiting { session: None, source, run, run_lock: owner, retain });
    }
}
/// Load committed generations and legacy flat copies. Every opened source stays
/// on disk until the next Recovery adopts and checkpoints its replacement.
pub fn recover_all(root: &Path, s: &mut Session) -> Recovered {
    let mut out = Recovered::default();
    let Ok(entries) = std::fs::read_dir(root) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if storage::regular(&path) && path.extension().is_some_and(|s| s == "figcraft") {
            if let Ok(source) = CheckpointRef::legacy(&path) {
                open_source(source, None, None, s, &mut out);
            }
        } else if storage::directory(&path) {
            let lock_path = path.join(LOCK);
            // Existing run locks are never replaced or truncated. A directory
            // without one is an incomplete fresh run and is left for inspection.
            if !storage::regular(&lock_path) {
                continue;
            }
            let Ok(file) = File::options().write(true).open(lock_path) else { continue };
            if file.try_lock().is_err() {
                continue;
            }
            let owner = Arc::new(file);
            for source in storage::candidates(&path) {
                open_source(source, Some(path.clone()), Some(owner.clone()), s, &mut out);
            }
            storage::remove_empty_run(&path, owner);
        }
    }
    out
}

#[cfg(test)]
mod tests;
