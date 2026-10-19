use super::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let parent = std::env::temp_dir();
        Self(storage::unique(&parent, "figcraft-recovery-test").unwrap())
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
fn uid(s: &Session) -> u64 {
    s.docs[0].uid
}
fn payload(s: &Session, note: Option<&str>) -> Payload {
    Payload { snapshot: capture_recovery(s, uid(s)).unwrap(), online: note.map(str::to_owned) }
}
fn checkpoint(r: &Recovery, s: &Session) -> CheckpointRef {
    CheckpointRef::generated(&r.dir.join(uid(s).to_string())).unwrap()
}
fn name(source: &CheckpointRef) -> String {
    figcraft_format::load_bytes_with_versions(&std::fs::read(&source.archive).unwrap()).unwrap().doc.name
}
fn gate(r: &mut Recovery, phase: storage::Phase) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release = Mutex::new(release_rx);
    let first = AtomicBool::new(true);
    r.hooks.callback = Some(Arc::new(move |at| {
        if at == phase && first.swap(false, Ordering::SeqCst) {
            entered_tx.send(()).unwrap();
            release.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
        }
        Ok(())
    }));
    (entered_rx, release_tx)
}
fn wait_flush(r: &mut Recovery, ticket: &FlushTicket) -> FlushStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = r.poll_flush(ticket);
        if status != FlushStatus::Pending {
            return status;
        }
        assert!(Instant::now() < deadline, "flush did not finish");
        std::thread::yield_now();
    }
}
#[test]
fn ipad_recovery_survives_interrupted_write() {
    for phase in [storage::Phase::ArchiveWritten, storage::Phase::NoteWritten, storage::Phase::ManifestRenamed] {
        let root = Root::new();
        let mut s = edited();
        let mut r = Recovery::start(&root.0).unwrap();
        storage::publish(&r.dir, uid(&s), &payload(&s, Some("old note")), Durability::Atomic, &Hooks::default()).unwrap();
        let old_name = s.doc().unwrap().name.clone();
        s.execute("file.rename", &json!({"name":"New generation"})).unwrap();
        r.hooks.callback = Some(Arc::new(move |at| if at == phase { Err(std::io::Error::other("interrupted")) } else { Ok(()) }));
        assert!(storage::publish(&r.dir, uid(&s), &payload(&s, Some("new note")), Durability::Atomic, &r.hooks).is_err());
        let source = checkpoint(&r, &s);
        if phase == storage::Phase::ManifestRenamed {
            assert_eq!(name(&source), "New generation");
            assert_eq!(source.online.as_deref(), Some("new note"));
        } else {
            assert_eq!(name(&source), old_name);
            assert_eq!(source.online.as_deref(), Some("old note"));
        }
        // An interrupted generation never becomes a second recovery candidate.
        assert_eq!(storage::candidates(&r.dir).len(), 1);
        r.finish();
    }
}
#[test]
fn ipad_worker_finishes_before_deferred_checkpoint_deletion() {
    let root = Root::new();
    let mut s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    let (entered, release) = gate(&mut r, storage::Phase::ArchiveWritten);
    r.write(&s, false);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    s.ds_mut().unwrap().mark_saved();
    let ticket = r.begin_flush(&s).unwrap();
    assert_eq!(r.poll_flush(&ticket), FlushStatus::Pending);
    assert!(r.written.get(&uid(&s)).unwrap().flight.is_some());
    release.send(()).unwrap();
    assert_eq!(wait_flush(&mut r, &ticket), FlushStatus::Complete);
    assert!(!r.dir.join(uid(&s).to_string()).exists());
    r.finish();
}
#[test]
fn ipad_earlier_success_does_not_cover_a_conflicting_worker() {
    let root = Root::new();
    let mut s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    r.write(&s, true);
    let original = s.doc().unwrap().name.clone();
    s.execute("file.rename", &json!({"name":"Intermediate"})).unwrap();
    let (entered, release) = gate(&mut r, storage::Phase::ArchiveWritten);
    r.write(&s, false);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    s.execute("file.rename", &json!({"name":original})).unwrap();
    let ticket = r.begin_flush(&s).unwrap();
    assert!(!r.covered());
    assert_eq!(r.poll_flush(&ticket), FlushStatus::Pending);
    release.send(()).unwrap();
    assert_eq!(wait_flush(&mut r, &ticket), FlushStatus::Complete);
    assert_eq!(name(&checkpoint(&r, &s)), original);
    r.finish();
}
#[test]
fn ipad_metadata_changes_schedule_without_document_revision_changes() {
    let root = Root::new();
    let mut s = edited();
    s.execute("file.saveVersion", &json!({"name":"Before"})).unwrap();
    let mut r = Recovery::start(&root.0).unwrap();
    r.write(&s, true);
    let old = checkpoint(&r, &s).archive;
    let revision = s.ds().unwrap().revision;
    s.ds_mut().unwrap().versions[0].description = "Metadata-only change".into();
    assert_eq!(s.ds().unwrap().revision, revision);
    r.write(&s, true);
    let current = checkpoint(&r, &s);
    assert_ne!(old, current.archive);
    let loaded = figcraft_format::load_bytes_with_versions(&std::fs::read(current.archive).unwrap()).unwrap();
    assert_eq!(loaded.versions[0].description, "Metadata-only change");
    r.finish();
}
#[test]
fn ipad_open_branch_overlay_and_history_round_trip() {
    let root = Root::new();
    let mut s = edited();
    let main = uid(&s);
    s.execute("branch.create", &json!({"name":"Explore"})).unwrap();
    s.execute("file.saveVersion", &json!({"name":"Branch version"})).unwrap();
    s.ds_mut().unwrap().doc_mut().name = "Unsynchronized branch".into();
    let mut r = Recovery::start(&root.0).unwrap();
    r.write(&s, true);
    let source = CheckpointRef::generated(&r.dir.join(main.to_string())).unwrap();
    let loaded = figcraft_format::load_bytes_with_versions(&std::fs::read(source.archive).unwrap()).unwrap();
    assert_eq!(loaded.branches[0].head.name, "Unsynchronized branch");
    assert_eq!(loaded.branches[0].versions[0].name, "Branch version");
    r.finish();
}
#[test]
fn ipad_failed_replacement_retains_previous_run_until_success() {
    let root = Root::new();
    let s = edited();
    let mut old = Recovery::start(&root.0).unwrap();
    old.write(&s, true);
    let old_dir = old.dir.clone();
    drop(old);
    let mut next = Session::empty();
    let recovered = recover_all(&root.0, &mut next);
    assert_eq!(recovered.copies, 1);
    assert!(old_dir.exists());
    let mut r = Recovery::start(&root.0).unwrap();
    r.adopt(recovered.waiting);
    r.hooks.callback = Some(Arc::new(|_| Err(std::io::Error::other("failed replacement"))));
    r.write(&next, true);
    assert!(old_dir.exists());
    r.hooks = Hooks::default();
    r.write(&next, true);
    assert!(!old_dir.exists());
    assert_eq!(storage::candidates(&r.dir).len(), 1);
    r.finish();
}
#[test]
fn ipad_dropped_host_worker_retains_its_run_lock() {
    let root = Root::new();
    let s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    let run = r.dir.clone();
    let (entered, release) = gate(&mut r, storage::Phase::ArchiveWritten);
    r.write(&s, false);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(r);
    assert_eq!(recover_all(&root.0, &mut Session::empty()).copies, 0);
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let file = File::options().write(true).open(run.join(LOCK)).unwrap();
        if file.try_lock().is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let mut next = Session::empty();
    let recovered = recover_all(&root.0, &mut next);
    assert_eq!(recovered.copies, 1);
    assert!(run.exists());
    let mut r = Recovery::start(&root.0).unwrap();
    r.adopt(recovered.waiting);
    r.write(&next, true);
    r.finish();
}
#[test]
fn ipad_flush_reports_publication_failure_and_a_new_attempt_can_retry() {
    let root = Root::new();
    let s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    r.hooks.callback = Some(Arc::new(|_| Err(std::io::Error::other("disk failure"))));
    let ticket = r.begin_flush(&s).unwrap();
    assert!(matches!(wait_flush(&mut r, &ticket), FlushStatus::Failed(_)));
    r.hooks = Hooks::default();
    let ticket = r.begin_flush(&s).unwrap();
    assert_eq!(wait_flush(&mut r, &ticket), FlushStatus::Complete);
    r.finish();
}
#[test]
fn ipad_legacy_flat_copy_is_retained_until_replacement_is_written() {
    let root = Root::new();
    let s = edited();
    let path = root.0.join("legacy.figcraft");
    std::fs::write(&path, figcraft_engine::persistence::encode_recovery(&payload(&s, None).snapshot).unwrap()).unwrap();
    let mut next = Session::empty();
    let recovered = recover_all(&root.0, &mut next);
    assert_eq!(recovered.copies, 1);
    assert!(path.exists());
    let mut r = Recovery::start(&root.0).unwrap();
    r.adopt(recovered.waiting);
    r.write(&next, true);
    assert!(!path.exists());
    r.finish();
}
#[test]
fn ipad_synced_generation_can_be_reopened() {
    let root = Root::new();
    let s = edited();
    let mut r = Recovery::start_with(&root.0, Durability::Synced).unwrap();
    r.write(&s, true);
    assert!(r.covered());
    assert_eq!(storage::candidates(&r.dir).len(), 1);
    r.finish();
}

#[test]
fn ipad_loader_rejects_invalid_manifests_and_missing_online_notes() {
    let root = Root::new();
    let s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    r.write(&s, true);
    let slot = r.dir.join(uid(&s).to_string());
    let current = slot.join("current");
    let valid = std::fs::read(&current).unwrap();
    for generation in ["../escape", "/tmp/escape", "g--", "g-1-2-", "g-1-2-3/escape"] {
        std::fs::write(&current, serde_json::to_vec(&json!({"version":1,"generation":generation,"online":false})).unwrap()).unwrap();
        assert!(CheckpointRef::generated(&slot).is_err());
        assert!(storage::candidates(&r.dir).is_empty());
    }
    std::fs::write(&current, vec![b' '; 4097]).unwrap();
    assert!(CheckpointRef::generated(&slot).is_err());
    let mut note_missing: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    note_missing["online"] = json!(true);
    std::fs::write(&current, serde_json::to_vec(&note_missing).unwrap()).unwrap();
    assert!(CheckpointRef::generated(&slot).is_err());
    std::fs::write(&current, valid).unwrap();
    assert_eq!(storage::candidates(&r.dir).len(), 1);
    r.finish();
}

#[test]
fn ipad_uncertain_publication_requires_a_successful_retry() {
    let root = Root::new();
    let mut s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    r.write(&s, true);
    s.execute("file.rename", &json!({"name":"After rename"})).unwrap();
    r.hooks.callback =
        Some(Arc::new(|phase| if phase == storage::Phase::ManifestRenamed { Err(std::io::Error::other("directory sync failed")) } else { Ok(()) }));
    r.write(&s, true);
    assert!(!r.covered());
    let uncertain = checkpoint(&r, &s);
    assert_eq!(name(&uncertain), "After rename");
    assert_eq!(std::fs::read_dir(r.dir.join(uid(&s).to_string()).join("generations")).unwrap().count(), 2);
    r.hooks = Hooks::default();
    r.write(&s, true);
    assert!(r.covered());
    r.finish();
}

#[test]
fn ipad_newer_queued_capture_runs_after_an_older_worker_fails() {
    let root = Root::new();
    let mut s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    let (entered, release) = gate(&mut r, storage::Phase::ArchiveWritten);
    let callback = r.hooks.callback.take().unwrap();
    let fail = AtomicBool::new(true);
    r.hooks.callback = Some(Arc::new(move |phase| {
        callback(phase)?;
        if phase == storage::Phase::ArchiveWritten && fail.swap(false, Ordering::SeqCst) {
            return Err(std::io::Error::other("older capture failed"));
        }
        Ok(())
    }));
    r.write(&s, false);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    s.execute("file.rename", &json!({"name":"Newer queued capture"})).unwrap();
    let ticket = r.begin_flush(&s).unwrap();
    release.send(()).unwrap();
    assert_eq!(wait_flush(&mut r, &ticket), FlushStatus::Complete);
    assert_eq!(name(&checkpoint(&r, &s)), "Newer queued capture");
    r.finish();
}

#[test]
fn ipad_failed_delete_invalidates_the_remembered_checkpoint() {
    let root = Root::new();
    let s = edited();
    let mut r = Recovery::start(&root.0).unwrap();
    r.write(&s, true);
    let slot = r.written.get_mut(&uid(&s)).unwrap();
    slot.desired = Intent::Delete;
    // A failed filesystem cleanup may already have removed some members. Force
    // an observed invalid cleanup target; successful prior bytes are no proof now.
    let path = r.dir.join(uid(&s).to_string());
    std::fs::remove_dir_all(&path).unwrap();
    std::fs::write(&path, b"block cleanup").unwrap();
    r.drive(false, true, true);
    assert!(r.written.get(&uid(&s)).unwrap().error.is_some());
    std::fs::remove_file(&path).unwrap();
    r.write(&s, true);
    assert!(r.covered());
    assert!(checkpoint(&r, &s).archive.is_file());
    r.finish();
}

#[test]
fn ipad_legacy_unreadable_note_does_not_hide_the_archive() {
    let root = Root::new();
    let s = edited();
    let archive = root.0.join("legacy.figcraft");
    let note = archive.with_extension("online");
    std::fs::write(&archive, figcraft_engine::persistence::encode_recovery(&payload(&s, None).snapshot).unwrap()).unwrap();
    std::fs::write(&note, [0xff, 0xfe]).unwrap();
    let mut next = Session::empty();
    let recovered = recover_all(&root.0, &mut next);
    assert_eq!(recovered.copies, 1);
    assert!(next.docs[0].is_dirty() && next.docs[0].path.is_none());
    let mut r = Recovery::start(&root.0).unwrap();
    r.adopt(recovered.waiting);
    r.write(&next, true);
    assert!(r.covered());
    assert!(archive.is_file() && note.is_file(), "unreadable legacy metadata stays available for inspection");
    r.finish();
}
