//! Bounded owned-document saves. No Session, native object, or UI borrow crosses the worker boundary.
use crate::documents::{ImportCopy, OwnedRecents};
use crate::writes::{WriteCoordinator, WriteError, WriteTicket};
use figcraft_engine::persistence::{PreparedSave, WriteOrder, encode_save};
use figcraft_ui_egui::platform_requests::{OwnedSaveCompletion, OwnedSaveDestination, OwnedSaveOutcome, OwnedSaveRequest};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

const MAX_OUTSTANDING: usize = 8;

struct Job {
    prepared: PreparedSave,
    ticket: WriteTicket<WriteOrder>,
    path: PathBuf,
    directory: PathBuf,
    owner: Option<Arc<ImportCopy>>,
    wake: egui::Context,
}

#[derive(Default)]
struct State {
    queued: VecDeque<Job>,
    completed: VecDeque<OwnedSaveCompletion>,
    highest: BTreeMap<PathBuf, WriteOrder>,
    closing: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}

pub struct SaveWorker {
    store: OwnedRecents,
    coordinator: WriteCoordinator<WriteOrder>,
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
    outstanding: BTreeSet<WriteOrder>,
}

impl SaveWorker {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        Self::start(root, write)
    }

    fn start(root: &Path, writer: impl FnMut(Job, &Path) -> Result<OwnedSaveOutcome, String> + Send + 'static) -> std::io::Result<Self> {
        let store = OwnedRecents::open(root)?;
        let coordinator = WriteCoordinator::new(store.directory())?;
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        let root = store.directory().to_path_buf();
        let thread = std::thread::Builder::new().name("owned-document-saves".into()).spawn(move || run(worker, root, writer))?;
        Ok(Self { store, coordinator, shared, thread, outstanding: BTreeSet::new() })
    }

    pub fn submit(&mut self, request: OwnedSaveRequest, wake: egui::Context) -> Result<(), String> {
        if self.thread.is_finished() {
            return Err("The save worker stopped; restart the app before saving again".into());
        }
        if self.outstanding.len() >= MAX_OUTSTANDING {
            return Err("Too many saves are pending; wait for a save to finish".into());
        }
        let (path, owner) = match request.destination {
            OwnedSaveDestination::Existing(path) => {
                let path = self.store.validate_document(Path::new(&path)).map_err(|error| error.to_string())?;
                if path.extension().is_none_or(|ext| ext != "figcraft") {
                    return Err("Existing saves require an owned .figcraft document".into());
                }
                (path, None)
            }
            OwnedSaveDestination::NewCopy(name) => {
                let name = name.trim().trim_end_matches(".figcraft");
                if name.is_empty()
                    || name.len() > 240
                    || name.starts_with('.')
                    || name.contains(['/', '\\', ':'])
                    || name.chars().any(char::is_control)
                {
                    return Err("Enter a document name without path separators or control characters".into());
                }
                let owner = ImportCopy::create(self.store.directory()).map_err(|error| error.to_string())?;
                (owner.directory().join(format!("{name}.figcraft")), Some(owner))
            }
        };
        let key = self.coordinator.key(&path).map_err(|error| error.to_string())?;
        let directory = path.parent().ok_or("The document has no owned directory")?.to_path_buf();
        let order = request.prepared.identity().write_order();
        let mut state = self.shared.state.lock().map_err(|_| "The save queue lock is poisoned")?;
        // Each exclusive import/copy directory owns one document. Keying admission by
        // that identity also serializes case/Unicode aliases of its file name on APFS.
        if state.highest.get(&directory).is_some_and(|highest| *highest >= order) {
            return Err("A newer save for this owned document was already accepted".into());
        }
        let ticket = self.coordinator.admit(key, order).map_err(|error| error.to_string())?;
        state.highest.insert(directory.clone(), order);
        if let Some(index) = state.queued.iter().position(|job| job.directory == directory)
            && let Some(old) = state.queued.remove(index)
        {
            state.completed.push_back(OwnedSaveCompletion { order: old.prepared.identity().write_order(), result: Ok(OwnedSaveOutcome::Superseded) });
        }
        state.queued.push_back(Job { prepared: request.prepared, ticket, path, directory, owner, wake });
        self.outstanding.insert(order);
        self.shared.ready.notify_one();
        Ok(())
    }

    pub fn poll(&mut self) -> Option<OwnedSaveCompletion> {
        match self.shared.state.lock() {
            Ok(mut state) => {
                if let Some(completion) = state.completed.pop_front() {
                    self.outstanding.remove(&completion.order);
                    return Some(completion);
                }
                if !self.thread.is_finished() {
                    return None;
                }
            }
            Err(error) => log::error!("Save queue is unavailable: {error}"),
        }
        self.outstanding.pop_first().map(|order| OwnedSaveCompletion {
            order,
            result: Err("The save worker stopped before confirming this write; the document remains unsaved".into()),
        })
    }
}

impl Drop for SaveWorker {
    fn drop(&mut self) {
        match self.shared.state.lock() {
            Ok(mut state) => {
                state.closing = true;
                self.shared.ready.notify_one();
            }
            Err(error) => log::error!("Could not drain accepted saves: {error}"),
        }
    }
}

fn run(shared: Arc<Shared>, root: PathBuf, mut writer: impl FnMut(Job, &Path) -> Result<OwnedSaveOutcome, String>) {
    loop {
        let job = {
            let Ok(mut state) = shared.state.lock() else {
                log::error!("Save worker stopped: queue lock is poisoned");
                return;
            };
            while state.queued.is_empty() && !state.closing {
                let Ok(next) = shared.ready.wait(state) else {
                    log::error!("Save worker stopped: queue wait is poisoned");
                    return;
                };
                state = next;
            }
            let Some(job) = state.queued.pop_front() else { return };
            job
        };
        let order = job.ticket.order();
        let wake = job.wake.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| writer(job, &root)))
            .unwrap_or_else(|_| Err("The document save worker panicked; this write was not confirmed".into()));
        if let Err(error) = &result {
            log::error!("Owned document save failed: {error}");
        }
        let Ok(mut state) = shared.state.lock() else {
            log::error!("Save worker cannot deliver completion: queue lock is poisoned");
            wake.request_repaint();
            return;
        };
        state.completed.push_back(OwnedSaveCompletion { order, result });
        drop(state);
        wake.request_repaint();
    }
}

fn write(job: Job, root: &Path) -> Result<OwnedSaveOutcome, String> {
    let bytes = encode_save(&job.prepared).map_err(|error| error.to_string())?;
    match job.ticket.run(&bytes) {
        Ok(()) => {}
        Err(WriteError::Superseded) => return Ok(OwnedSaveOutcome::Superseded),
        Err(error) => return Err(error.to_string()),
    }
    if let Some(owner) = job.owner {
        std::fs::File::open(root).and_then(|file| file.sync_all()).map_err(|error| error.to_string())?;
        owner.promote(&job.path).map_err(|error| error.to_string())?;
    }
    let path = job.path.into_os_string().into_string().map_err(|_| "The saved document path is not UTF-8")?;
    Ok(OwnedSaveOutcome::Saved(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_engine::Session;
    use figcraft_engine::persistence::{SaveAssurance, SaveDestination, SaveOutcome, acknowledge_save, prepare_save};
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn capture(session: &mut Session) -> PreparedSave {
        let uid = session.ds().unwrap().uid;
        prepare_save(session, uid).unwrap()
    }

    fn edit(session: &mut Session) {
        session.execute("shape.rectangle", &json!({"x": 3, "y": 4, "width": 50, "height": 60})).unwrap();
    }

    fn completion(worker: &mut SaveWorker) -> OwnedSaveCompletion {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(done) = worker.poll() {
                return done;
            }
            assert!(Instant::now() < deadline, "save completion timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn saved(completion: OwnedSaveCompletion) -> String {
        match completion.result {
            Ok(OwnedSaveOutcome::Saved(path)) => path,
            Ok(OwnedSaveOutcome::Superseded) => panic!("unexpected superseded save"),
            Err(error) => panic!("save failed: {error}"),
        }
    }

    #[test]
    fn ipad_local_save_reopen() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let mut worker = SaveWorker::new(root.directory()).unwrap();
        let mut session = Session::new();
        edit(&mut session);
        let expected = session.doc().unwrap().nodes().count();
        let prepared = capture(&mut session);
        worker
            .submit(
                OwnedSaveRequest { prepared: prepared.clone(), destination: OwnedSaveDestination::NewCopy("Local copy".into()) },
                egui::Context::default(),
            )
            .unwrap();
        let path = saved(completion(&mut worker));
        assert!(session.ds().unwrap().is_dirty(), "only acknowledgement may mark the capture saved");
        assert_eq!(
            acknowledge_save(&mut session, &prepared, SaveDestination::SaveAs(path.clone()), SaveAssurance::SyncedReplacement).unwrap(),
            SaveOutcome::CurrentSaved
        );
        assert!(!session.ds().unwrap().is_dirty());
        let recents = OwnedRecents::open(root.directory()).unwrap();
        recents.save(&session.prefs.recent_files).unwrap();
        let loaded = recents.load().unwrap();
        assert_eq!(loaded, vec![path.clone()]);
        let mut reopened = Session::new();
        reopened.execute("file.open", &json!({"path": loaded[0]})).unwrap();
        assert_eq!(reopened.doc().unwrap().nodes().count(), expected);
        assert_eq!(reopened.ds().unwrap().path.as_deref(), Some(path.as_str()));
    }

    #[test]
    fn ipad_save_coalesces_and_bounds_outstanding_work() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let (started, ready) = std::sync::mpsc::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let mut first = true;
        let mut worker = SaveWorker::start(root.directory(), move |job, root| {
            let result = write(job, root);
            if first {
                first = false;
                let path = match &result {
                    Ok(OwnedSaveOutcome::Saved(path)) => path.clone(),
                    _ => panic!("first write failed"),
                };
                started.send(path).unwrap();
                gate.recv_timeout(Duration::from_secs(10)).unwrap();
            }
            result
        })
        .unwrap();
        let mut session = Session::new();
        edit(&mut session);
        let first_capture = capture(&mut session);
        worker
            .submit(
                OwnedSaveRequest { prepared: first_capture.clone(), destination: OwnedSaveDestination::NewCopy("Typing".into()) },
                egui::Context::default(),
            )
            .unwrap();
        let path = ready.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut newest = first_capture.clone();
        for _ in 1..MAX_OUTSTANDING {
            edit(&mut session);
            newest = capture(&mut session);
            worker
                .submit(
                    OwnedSaveRequest { prepared: newest.clone(), destination: OwnedSaveDestination::Existing(path.clone()) },
                    egui::Context::default(),
                )
                .unwrap();
        }
        {
            let state = worker.shared.state.lock().unwrap();
            assert_eq!(state.queued.len(), 1);
            assert_eq!(state.completed.len(), MAX_OUTSTANDING - 2);
        }
        let excess = capture(&mut session);
        assert!(
            worker
                .submit(OwnedSaveRequest { prepared: excess, destination: OwnedSaveDestination::Existing(path.clone()) }, egui::Context::default())
                .unwrap_err()
                .contains("Too many")
        );
        release.send(()).unwrap();
        let mut written = Vec::new();
        for _ in 0..MAX_OUTSTANDING {
            let done = completion(&mut worker);
            if matches!(done.result, Ok(OwnedSaveOutcome::Saved(_))) {
                written.push(done.order);
            } else {
                assert!(matches!(done.result, Ok(OwnedSaveOutcome::Superseded)));
            }
        }
        assert_eq!(written, vec![first_capture.identity().write_order(), newest.identity().write_order()]);
        assert_eq!(
            acknowledge_save(&mut session, &first_capture, SaveDestination::SaveAs(path.clone()), SaveAssurance::SyncedReplacement).unwrap(),
            SaveOutcome::OlderSnapshotSaved
        );
        assert!(session.ds().unwrap().is_dirty());
        assert_eq!(
            acknowledge_save(&mut session, &newest, SaveDestination::Existing(path.clone()), SaveAssurance::SyncedReplacement).unwrap(),
            SaveOutcome::CurrentSaved
        );
        let expected = session.doc().unwrap().nodes().count();
        let mut reopened = Session::new();
        reopened.execute("file.open", &json!({"path": path})).unwrap();
        assert_eq!(reopened.doc().unwrap().nodes().count(), expected);
    }

    #[test]
    fn ipad_save_while_typing() {
        use figcraft_engine::textedit;
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let (started, ready) = std::sync::mpsc::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let mut worker = SaveWorker::start(root.directory(), move |job, root| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(10)).unwrap();
            write(job, root)
        })
        .unwrap();
        let mut session = Session::new();
        let created = session.execute("text.create", &json!({"text": "Before", "x": 20, "y": 20})).unwrap();
        let id = figcraft_engine::params::id_of(&created["id"]).unwrap();
        textedit::begin(&mut session, id, Some(6), false).unwrap();
        textedit::insert(&mut session, "A").unwrap();
        let prepared = capture(&mut session);
        worker
            .submit(
                OwnedSaveRequest { prepared: prepared.clone(), destination: OwnedSaveDestination::NewCopy("Typing".into()) },
                egui::Context::default(),
            )
            .unwrap();
        ready.recv_timeout(Duration::from_secs(10)).unwrap();
        textedit::insert(&mut session, "B").unwrap();
        release.send(()).unwrap();
        let path = saved(completion(&mut worker));
        assert_eq!(
            acknowledge_save(&mut session, &prepared, SaveDestination::SaveAs(path.clone()), SaveAssurance::SyncedReplacement).unwrap(),
            SaveOutcome::OlderSnapshotSaved
        );
        assert!(session.ds().unwrap().is_dirty());
        assert_eq!(session.ds().unwrap().text_edit.as_ref().unwrap().caret, 8);
        assert_eq!(session.doc().unwrap().node(id).unwrap().kind.text().unwrap().characters, "BeforeAB");
        let mut reopened = Session::new();
        reopened.execute("file.open", &json!({"path": path})).unwrap();
        assert_eq!(reopened.doc().unwrap().node(id).unwrap().kind.text().unwrap().characters, "BeforeA");
        textedit::insert(&mut session, "C").unwrap();
        assert_eq!(session.doc().unwrap().node(id).unwrap().kind.text().unwrap().characters, "BeforeABC");
    }

    #[test]
    fn ipad_save_rejects_delayed_older_admission_and_external_paths() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let mut worker = SaveWorker::new(root.directory()).unwrap();
        let mut session = Session::new();
        edit(&mut session);
        let old = capture(&mut session);
        edit(&mut session);
        let new = capture(&mut session);
        worker
            .submit(OwnedSaveRequest { prepared: new, destination: OwnedSaveDestination::NewCopy("Latest".into()) }, egui::Context::default())
            .unwrap();
        let path = saved(completion(&mut worker));
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            worker
                .submit(OwnedSaveRequest { prepared: old, destination: OwnedSaveDestination::Existing(path.clone()) }, egui::Context::default())
                .unwrap_err()
                .contains("newer save")
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        let outside = root.directory().join("outside.figcraft");
        std::fs::write(&outside, b"external original").unwrap();
        assert!(
            worker
                .submit(
                    OwnedSaveRequest {
                        prepared: capture(&mut session),
                        destination: OwnedSaveDestination::Existing(outside.to_str().unwrap().into())
                    },
                    egui::Context::default()
                )
                .is_err()
        );
        assert_eq!(std::fs::read(outside).unwrap(), b"external original");
    }

    #[test]
    fn ipad_save_failure_and_panic_never_acknowledge_or_leave_new_copies() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let mut worker = SaveWorker::start(root.directory(), |_, _| Err("synthetic disk full".into())).unwrap();
        let mut session = Session::new();
        edit(&mut session);
        worker
            .submit(
                OwnedSaveRequest { prepared: capture(&mut session), destination: OwnedSaveDestination::NewCopy("Failed".into()) },
                egui::Context::default(),
            )
            .unwrap();
        assert!(completion(&mut worker).result.err().unwrap().contains("disk full"));
        assert!(session.ds().unwrap().path.is_none());
        assert!(session.ds().unwrap().is_dirty());
        assert_eq!(std::fs::read_dir(root.directory()).unwrap().count(), 0);
        let mut worker = SaveWorker::start(root.directory(), |_, _| panic!("synthetic encoder panic")).unwrap();
        worker
            .submit(
                OwnedSaveRequest { prepared: capture(&mut session), destination: OwnedSaveDestination::NewCopy("Panicked".into()) },
                egui::Context::default(),
            )
            .unwrap();
        assert!(completion(&mut worker).result.err().unwrap().contains("panicked"));
        assert_eq!(std::fs::read_dir(root.directory()).unwrap().count(), 0);
    }

    #[test]
    fn ipad_save_existing_failure_preserves_previous_file() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let mut first = true;
        let mut worker = SaveWorker::start(root.directory(), move |job, root| {
            if first {
                first = false;
                write(job, root)
            } else {
                Err("synthetic disk full before replacement".into())
            }
        })
        .unwrap();
        let mut session = Session::new();
        edit(&mut session);
        let old = capture(&mut session);
        worker
            .submit(
                OwnedSaveRequest { prepared: old.clone(), destination: OwnedSaveDestination::NewCopy("Previous".into()) },
                egui::Context::default(),
            )
            .unwrap();
        let path = saved(completion(&mut worker));
        let bytes = std::fs::read(&path).unwrap();
        acknowledge_save(&mut session, &old, SaveDestination::SaveAs(path.clone()), SaveAssurance::SyncedReplacement).unwrap();
        edit(&mut session);
        worker
            .submit(
                OwnedSaveRequest { prepared: capture(&mut session), destination: OwnedSaveDestination::Existing(path.clone()) },
                egui::Context::default(),
            )
            .unwrap();
        assert!(completion(&mut worker).result.err().unwrap().contains("disk full"));
        assert!(session.ds().unwrap().is_dirty());
        assert_eq!(session.ds().unwrap().path.as_deref(), Some(path.as_str()));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn ipad_save_worker_drains_accepted_jobs_after_host_drop() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let (finished, result) = std::sync::mpsc::channel();
        let mut worker = SaveWorker::start(root.directory(), move |job, root| {
            let outcome = write(job, root);
            if let Ok(OwnedSaveOutcome::Saved(path)) = &outcome {
                finished.send(path.clone()).unwrap();
            }
            outcome
        })
        .unwrap();
        let mut session = Session::new();
        for index in 0..3 {
            edit(&mut session);
            worker
                .submit(
                    OwnedSaveRequest { prepared: capture(&mut session), destination: OwnedSaveDestination::NewCopy(format!("Copy {index}")) },
                    egui::Context::default(),
                )
                .unwrap();
        }
        let shared = worker.shared.clone();
        drop(worker);
        for _ in 0..3 {
            let path = result.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(Path::new(&path).is_file());
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while shared.state.lock().unwrap().completed.len() != 3 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
