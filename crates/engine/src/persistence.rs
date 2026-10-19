//! Owned file captures and acknowledgement. Destination writers own I/O ordering;
//! an acknowledgement is permitted only after their selected write contract succeeds.
use crate::{DocState, EngineError, Result, Session};
use figcraft_doc::{Document, NodeId};
use figcraft_format::{ArchiveContent, Branch, Version};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_SAVE: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WriteOrder(u64);
#[derive(Clone, Debug)]
pub struct SaveIdentity {
    uid: u64,
    incarnation: Arc<()>,
    order: WriteOrder,
}
impl SaveIdentity {
    pub fn document_uid(&self) -> u64 {
        self.uid
    }
    pub fn write_order(&self) -> WriteOrder {
        self.order
    }
}
#[derive(Clone, Debug)]
pub struct PreparedSave {
    identity: SaveIdentity,
    snapshot: FileSnapshot,
}
impl PreparedSave {
    pub fn identity(&self) -> &SaveIdentity {
        &self.identity
    }
}
#[derive(Clone, Debug)]
pub struct RecoverySnapshot {
    snapshot: FileSnapshot,
}
impl RecoverySnapshot {
    /// A sufficient cheap identity for periodic checkpoints, including the opening page.
    pub fn same_capture(&self, other: &Self) -> bool {
        self.snapshot.page == other.snapshot.page && self.snapshot.roots.same(&other.snapshot.roots)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    CurrentSaved,
    OlderSnapshotSaved,
    DocumentClosed,
    Superseded,
}
#[derive(Clone, Debug)]
pub enum SaveDestination {
    Existing(String),
    SaveAs(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveAssurance {
    AtomicReplacement,
    SyncedReplacement,
}

#[derive(Clone, Debug)]
pub(super) struct State {
    incarnation: Arc<()>,
    baseline: Option<Roots>,
    local: bool,
    started: bool,
    last_ack: Option<WriteOrder>,
    assurance: Option<SaveAssurance>,
}
impl State {
    pub(super) fn new(doc: Arc<Document>, local: bool) -> Self {
        Self {
            incarnation: Arc::new(()),
            baseline: Some(Roots { doc, versions: Vec::new(), branches: Vec::new() }),
            local,
            started: false,
            last_ack: None,
            assurance: None,
        }
    }
    pub(super) fn loaded_versions(&mut self, versions: &[Version]) {
        if !self.started
            && let Some(b) = &mut self.baseline
        {
            b.versions = versions.to_vec();
        }
    }
    pub(super) fn loaded_branches(&mut self, branches: &[Branch]) {
        if !self.started
            && let Some(b) = &mut self.baseline
        {
            b.branches = branches.to_vec();
        }
    }
}
#[derive(Clone, Debug)]
struct Roots {
    doc: Arc<Document>,
    versions: Vec<Version>,
    branches: Vec<Branch>,
}
pub(crate) fn versions_same(a: &[Version], b: &[Version]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id && a.name == b.name && a.description == b.description && a.time == b.time && a.auto == b.auto && a.same_snapshot_storage(b)
        })
}
fn branch_same(a: &Branch, b: &Branch, head: &Arc<Document>, versions: &[Version]) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.time == b.time
        && a.client == b.client
        && a.state == b.state
        && Arc::ptr_eq(&a.base, &b.base)
        && Arc::ptr_eq(&a.head, head)
        && versions_same(&a.versions, versions)
}
fn branch_tab<'a>(s: &'a Session, main: u64, id: &str) -> Option<&'a DocState> {
    s.docs.iter().find(|d| d.branch_of.as_ref().is_some_and(|(uid, b)| *uid == main && b == id))
}
impl Roots {
    fn tab(d: &DocState) -> Self {
        Self { doc: d.document_itself().clone(), versions: d.versions.clone(), branches: d.branches.clone() }
    }
    fn file(s: &Session, d: &DocState) -> Self {
        let mut roots = Self::tab(d);
        for b in &mut roots.branches {
            if let Some(tab) = branch_tab(s, d.uid, &b.id) {
                b.head = tab.document_itself().clone();
                b.versions = tab.versions.clone();
            }
        }
        roots
    }
    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc, &other.doc)
            && versions_same(&self.versions, &other.versions)
            && self.branches.len() == other.branches.len()
            && self.branches.iter().zip(&other.branches).all(|(a, b)| branch_same(a, b, &b.head, &b.versions))
    }
    fn matches(&self, d: &DocState, session: Option<&Session>) -> bool {
        Arc::ptr_eq(&self.doc, d.document_itself())
            && versions_same(&self.versions, &d.versions)
            && self.branches.len() == d.branches.len()
            && self.branches.iter().zip(&d.branches).all(|(a, b)| {
                let tab = session.and_then(|s| branch_tab(s, d.uid, &b.id));
                branch_same(a, b, tab.map_or(&b.head, DocState::document_itself), tab.map_or(b.versions.as_slice(), |t| t.versions.as_slice()))
            })
    }
    fn content(&self, page: NodeId, thumbnail: bool) -> Result<ArchiveContent> {
        let mut doc = self.doc.clone();
        if doc.current_page != Some(page) {
            Arc::make_mut(&mut doc).current_page = Some(page);
        }
        let thumb = thumbnail.then(|| crate::cmd::file::thumbnail(&doc)).flatten();
        ArchiveContent::capture(&doc, thumb.as_deref(), &self.versions, &self.branches).map_err(format_error)
    }
}
fn format_error(e: figcraft_format::FormatError) -> EngineError {
    EngineError::Other(e.to_string())
}
#[derive(Clone, Debug)]
struct BranchCapture {
    id: String,
    uid: u64,
    incarnation: Arc<()>,
    roots: Roots,
}
#[derive(Clone, Debug)]
struct FileSnapshot {
    roots: Roots,
    page: NodeId,
    tabs: Vec<BranchCapture>,
}
fn main_index(s: &Session, uid: u64) -> Result<usize> {
    let i = s.docs.iter().position(|d| d.uid == uid).ok_or(EngineError::NoDocument)?;
    match s.docs.get(i).and_then(|d| d.branch_of.as_ref()) {
        Some((main, _)) => s.docs.iter().position(|d| d.uid == *main && d.branch_of.is_none()).ok_or(EngineError::NoDocument),
        None => Ok(i),
    }
}
impl FileSnapshot {
    fn capture(s: &Session, i: usize) -> Result<Self> {
        let d = s.docs.get(i).ok_or(EngineError::NoDocument)?;
        let tabs = s
            .docs
            .iter()
            .filter(|tab| tab.branch_of.as_ref().is_some_and(|(uid, id)| *uid == d.uid && d.branches.iter().any(|b| b.id == *id)))
            .map(|tab| BranchCapture {
                id: tab.branch_of.as_ref().map(|(_, id)| id.clone()).unwrap_or_default(),
                uid: tab.uid,
                incarnation: tab.persistence.incarnation.clone(),
                roots: Roots::tab(tab),
            })
            .collect();
        Ok(Self { roots: Roots::file(s, d), page: d.preview.as_ref().map_or(d.page, |p| p.page), tabs })
    }
}
impl DocState {
    pub(crate) fn rebase_open_branch(&mut self, id: &str, before: &Arc<Document>, after: &Arc<Document>) -> Result<()> {
        // Opening reconstructs nonserialized client state. Reuse the new root only
        // after the complete serialized document (including allocator) proves equal.
        let a = ArchiveContent::capture(before, None, &[], &[]).map_err(format_error)?;
        let b = ArchiveContent::capture(after, None, &[], &[]).map_err(format_error)?;
        if a.same_persisted_content(&b, false)
            && let Some(baseline) = &mut self.persistence.baseline
            && let Some(branch) = baseline.branches.iter_mut().find(|b| b.id == id && Arc::ptr_eq(&b.head, before))
        {
            branch.head = after.clone();
        }
        Ok(())
    }
    pub(super) fn persistence_dirty(&self) -> bool {
        self.persistence.baseline.as_ref().is_none_or(|b| !b.matches(self, None))
    }
    /// Compatibility initialization for a clean online/untitled document. A local
    /// file baseline can only be acknowledged through its captured write.
    pub fn mark_saved(&mut self) {
        self.mark_online_baseline();
    }
    pub(crate) fn mark_online_baseline(&mut self) {
        if !self.persistence.local && !self.persistence.started {
            self.saved_doc = self.document_itself().clone();
            self.persistence.baseline = Some(Roots::tab(self));
            self.versions_changed = false;
        }
    }
    pub(super) fn initial_baseline_available(&self) -> bool {
        !self.persistence.local && !self.persistence.started
    }
    pub(super) fn initial_theme_baseline(&mut self) {
        if !self.persistence.local && !self.persistence.started {
            self.saved_doc = self.doc.clone();
            self.persistence.baseline = Some(Roots::tab(self));
        }
    }
    pub(super) fn invalidate_baseline(&mut self) {
        self.persistence.baseline = None;
    }
    pub fn local_save_assurance(&self) -> Option<SaveAssurance> {
        self.persistence.assurance
    }
}
impl Session {
    /// Includes branch edits that have not yet reached the command-time sync.
    pub fn file_is_dirty(&self, uid: u64) -> bool {
        let Ok(i) = main_index(self, uid) else { return false };
        let Some(d) = self.docs.get(i) else { return false };
        d.persistence.baseline.as_ref().is_none_or(|b| !b.matches(d, Some(self)))
    }
    pub fn file_needs_recovery(&self, uid: u64) -> bool {
        let Ok(i) = main_index(self, uid) else { return false };
        let Some(d) = self.docs.get(i) else { return false };
        self.file_is_dirty(d.uid) || self.registered_online_pending(d.uid)
    }
}
pub fn prepare_save(session: &mut Session, document_uid: u64) -> Result<PreparedSave> {
    session.pointer_persistence_edit()?;
    let i = main_index(session, document_uid)?;
    let order = NEXT_SAVE
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map(WriteOrder)
        .map_err(|_| EngineError::Other("save order exhausted".into()))?;
    let previous = session.ds().ok().map(|d| d.uid);
    let requested = session.docs.iter().position(|d| d.uid == document_uid).ok_or(EngineError::NoDocument)?;
    session.active = requested;
    crate::branches::sync(session);
    crate::textpreview::end(session);
    if let Ok(d) = session.ds_mut() {
        d.end_preview();
    }
    if let Some(previous) = previous.and_then(|uid| session.docs.iter().position(|d| d.uid == uid)) {
        session.active = previous;
    }
    let d = session.docs.get_mut(i).ok_or(EngineError::NoDocument)?;
    d.persistence.started = true;
    let identity = SaveIdentity { uid: d.uid, incarnation: d.persistence.incarnation.clone(), order };
    Ok(PreparedSave { identity, snapshot: FileSnapshot::capture(session, i)? })
}
pub fn encode_save(prepared: &PreparedSave) -> Result<Vec<u8>> {
    prepared.snapshot.roots.content(prepared.snapshot.page, true)?.encode().map_err(format_error)
}
pub fn capture_recovery(session: &Session, document_uid: u64) -> Result<RecoverySnapshot> {
    Ok(RecoverySnapshot { snapshot: FileSnapshot::capture(session, main_index(session, document_uid)?)? })
}
pub fn encode_recovery(snapshot: &RecoverySnapshot) -> Result<Vec<u8>> {
    snapshot.snapshot.roots.content(snapshot.snapshot.page, false)?.encode().map_err(format_error)
}
fn install(d: &mut DocState, roots: Roots, order: WriteOrder, assurance: SaveAssurance) {
    d.saved_doc = roots.doc.clone();
    d.persistence.baseline = Some(roots);
    d.persistence.local = true;
    d.persistence.last_ack = Some(order);
    d.persistence.assurance = Some(assurance);
    // versions_changed remains a synchronization hint for open branch histories.
}
pub fn acknowledge_save(
    session: &mut Session,
    prepared: &PreparedSave,
    destination: SaveDestination,
    assurance: SaveAssurance,
) -> Result<SaveOutcome> {
    session.pointer_persistence_edit()?;
    let Some(i) =
        session.docs.iter().position(|d| d.uid == prepared.identity.uid && Arc::ptr_eq(&d.persistence.incarnation, &prepared.identity.incarnation))
    else {
        return Ok(SaveOutcome::DocumentClosed);
    };
    if session.docs.get(i).is_some_and(|d| d.persistence.last_ack.is_some_and(|order| order >= prepared.identity.order)) {
        return Ok(SaveOutcome::Superseded);
    }
    let path = match destination {
        SaveDestination::Existing(path) | SaveDestination::SaveAs(path) => path,
    };
    if path.is_empty() {
        return Err(EngineError::Other("save destination is empty".into()));
    }
    // Validate the written capture even if a caller incorrectly skipped encoding.
    let written = prepared.snapshot.roots.content(prepared.snapshot.page, false)?;
    let current = FileSnapshot::capture(session, i)?;
    let same = if current.roots.same(&prepared.snapshot.roots) {
        true
    } else {
        match current.roots.content(current.page, false) {
            Ok(content) => content.same_persisted_content(&written, true),
            Err(e) => {
                session.message = Some(format!("An earlier snapshot was saved; current file content could not be compared: {e}"));
                false
            }
        }
    };
    let roots = if same { current.roots } else { prepared.snapshot.roots.clone() };
    let d = session.docs.get_mut(i).ok_or(EngineError::NoDocument)?;
    install(d, roots, prepared.identity.order, assurance);
    d.path = Some(path.clone());
    d.revision += 1;
    for capture in &prepared.snapshot.tabs {
        if let Some(tab) = session.docs.iter_mut().find(|d| {
            d.uid == capture.uid
                && Arc::ptr_eq(&d.persistence.incarnation, &capture.incarnation)
                && d.branch_of.as_ref().is_some_and(|(uid, id)| *uid == prepared.identity.uid && id == &capture.id)
        }) {
            let live = Roots::tab(tab);
            let same = live.same(&capture.roots)
                || ArchiveContent::capture(&live.doc, None, &live.versions, &live.branches)
                    .and_then(|a| {
                        ArchiveContent::capture(&capture.roots.doc, None, &capture.roots.versions, &capture.roots.branches)
                            .map(|b| a.same_persisted_content(&b, false))
                    })
                    .unwrap_or(false);
            install(tab, if same { live } else { capture.roots.clone() }, prepared.identity.order, assurance);
        }
    }
    crate::cmd::file::remember(session, &path);
    Ok(if same { SaveOutcome::CurrentSaved } else { SaveOutcome::OlderSnapshotSaved })
}
