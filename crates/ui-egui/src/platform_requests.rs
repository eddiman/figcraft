//! Owned platform requests. Native objects and callbacks stay in the app adapter.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{FigApp, Services};

const MAX_PENDING: usize = 8;
const MAX_COMPLETIONS_PER_FRAME: usize = 32;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct RequestId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentRef {
    pub uid: u64,
}
#[derive(Clone, Debug)]
pub struct PickerSpec {
    pub title: String,
    pub suggested_name: Option<String>,
    pub extensions: Vec<String>,
}
#[derive(Debug)]
pub struct PlatformRequest {
    pub id: RequestId,
    pub initiating_document: Option<DocumentRef>,
    pub operation: Operation,
}
#[derive(Debug)]
pub enum Operation {
    OpenCopy(PickerSpec),
    ImportCopy(PickerSpec),
    SaveAs(PickerSpec),
    ExportFiles { picker: PickerSpec, files: Vec<StagedFile> },
    ExportTree { picker: PickerSpec, tree: crate::platform_export_tree::ExportTree },
}
#[derive(Debug)]
pub struct StagedFile {
    pub display_name: String,
    pub path: PathBuf,
    pub lease: Arc<StagingLease>,
}

/// Owns only a newly allocated staging directory, never an arbitrary supplied path.
#[derive(Debug)]
pub struct StagingLease {
    root: PathBuf,
}
impl StagingLease {
    pub fn create(parent: &Path) -> std::io::Result<Arc<Self>> {
        for _ in 0..32 {
            let id = allocate_id(&NEXT_ID).map_err(|_| std::io::Error::other("platform IDs exhausted"))?;
            let root = parent.join(format!("figcraft-stage-{}-{}", std::process::id(), id.0));
            match std::fs::create_dir(&root) {
                Ok(()) => return Ok(Arc::new(Self { root })),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("cannot allocate a unique staging directory"))
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub fn file(self: &Arc<Self>, display_name: String) -> Result<StagedFile, RejectReason> {
        if !valid_name(&display_name) {
            return Err(RejectReason::InvalidRequest("a staged file needs a single file name".into()));
        }
        Ok(StagedFile { path: self.root.join(&display_name), display_name, lease: self.clone() })
    }
}
impl Drop for StagingLease {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.root) {
            log::warn!("staging cleanup {}: {e}", self.root.display());
        }
    }
}
fn valid_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    !name.is_empty() && !name.contains(['/', '\\']) && matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}
#[derive(Debug)]
pub struct SelectedCopy {
    pub path: PathBuf,
    pub display_name: String,
    pub lease: Option<Arc<StagingLease>>,
}
#[derive(Debug)]
pub enum Payload {
    Selected(SelectedCopy),
    Destination(PathBuf),
    Exported,
}
#[derive(Debug)]
pub enum PlatformResult {
    Completed(Payload),
    Cancelled,
    Failed(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectReason {
    Unsupported,
    Busy,
    IdExhausted,
    InvalidRequest(String),
}
#[derive(Debug)]
pub enum Submission {
    Completed(PlatformResult),
    Pending,
    Rejected(RejectReason),
}
#[derive(Debug)]
pub struct Completion {
    pub id: RequestId,
    pub result: PlatformResult,
}

pub type PlatformDone = Box<dyn FnOnce(&mut FigApp, Payload)>;
pub(crate) type ImportDone = Box<dyn FnOnce(&mut FigApp, &Path)>;
pub(crate) type ExportDone = Box<dyn FnOnce(&mut FigApp, serde_json::Value)>;
#[derive(Clone, Copy)]
enum Expected {
    Selected,
    Destination,
    Exported,
}
struct Pending {
    document: Option<DocumentRef>,
    expected: Expected,
    done: PlatformDone,
}
#[derive(Default)]
pub(crate) struct PendingRequests {
    entries: BTreeMap<RequestId, Pending>,
}

fn allocate_id(counter: &AtomicU64) -> Result<RequestId, RejectReason> {
    counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map(RequestId).map_err(|_| RejectReason::IdExhausted)
}

impl FigApp {
    pub(crate) fn platform_command_allowed(&mut self, command: &str, params: &serde_json::Value) -> bool {
        let caps = self.services.capabilities();
        let denied = match command {
            "plugin.directory.install" => !caps.plugin_downloads,
            "plugin.import" => caps.document_mode != crate::platform::DocumentMode::DesktopPaths && params.get("path").is_some(),
            "collab.host" => !caps.local_session_hosting,
            "collab.share" => !caps.local_session_hosting && params["via"].as_str().map(str::trim) == Some("local"),
            "file.save" | "file.saveAs" => caps.document_mode != crate::platform::DocumentMode::DesktopPaths,
            _ => false,
        };
        if denied {
            self.platform_error(&format!("{command} is unsupported on this platform"));
        }
        !denied
    }

    pub(crate) fn picker(title: &str, suggested_name: Option<String>, extensions: &[&str]) -> PickerSpec {
        PickerSpec { title: title.into(), suggested_name, extensions: extensions.iter().map(|s| (*s).into()).collect() }
    }

    pub(crate) fn platform_document(&self) -> Option<DocumentRef> {
        self.session.ds().ok().map(|d| DocumentRef { uid: d.uid })
    }

    pub(crate) fn platform_error(&mut self, message: &str) {
        self.toast(message);
        self.ui.last_error = Some(message.into());
    }

    pub(crate) fn desktop_document_write(&mut self) -> bool {
        if self.services.capabilities().document_mode == crate::platform::DocumentMode::DesktopPaths {
            true
        } else {
            self.platform_error("Saving documents is unsupported on this platform until durable saving is available");
            false
        }
    }

    pub(crate) fn request_platform(&mut self, document: Option<DocumentRef>, operation: Operation, done: PlatformDone) {
        let ctx = self.platform_context.clone();
        let _ = self.submit_platform_request(&ctx, document, operation, done);
    }

    pub(crate) fn import_copy(&mut self, title: &str, extensions: &[&str], done: ImportDone) {
        let picker = Self::picker(title, None, extensions);
        self.request_platform(
            self.platform_document(),
            Operation::ImportCopy(picker),
            Box::new(move |app, payload| {
                if let Payload::Selected(copy) = payload {
                    // Retain the imported lease through synchronous consumption. Native adapters must
                    // promote inputs whose paths the engine retains before delivering SelectedCopy.
                    done(app, &copy.path);
                }
            }),
        );
    }

    /// Encode at activation, before a delayed picker can observe later edits or selection.
    pub(crate) fn export_snapshot(&mut self, title: &str, command: &str, mut params: serde_json::Value, name: &str, ext: &str, done: ExportDone) {
        let stage = match StagingLease::create(&std::env::temp_dir()) {
            Ok(stage) => stage,
            Err(e) => {
                self.platform_error(&e.to_string());
                return;
            }
        };
        // A single-file picker suggests only a basename, as the existing export UI does.
        let name = name.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or("Untitled");
        let file = match stage.file(name.into()) {
            Ok(file) => file,
            Err(reason) => {
                self.platform_rejected(&reason);
                return;
            }
        };
        params["path"] = serde_json::json!(file.path.to_string_lossy());
        let Some(result) = self.run(command, params) else { return };
        let picker = Self::picker(title, Some(name.into()), &[ext]);
        self.request_platform(
            self.platform_document(),
            Operation::ExportFiles { picker, files: vec![file] },
            Box::new(move |app, _| done(app, result)),
        );
    }

    /// Submit owned work. The success closure runs only for a valid, live target.
    pub fn submit_platform_request(
        &mut self,
        ctx: &egui::Context,
        document: Option<DocumentRef>,
        operation: Operation,
        done: PlatformDone,
    ) -> Result<RequestId, RejectReason> {
        self.cancel_closed_platform_requests();
        let expected = match &operation {
            Operation::OpenCopy(_) => Expected::Selected,
            Operation::ImportCopy(_) => Expected::Selected,
            Operation::SaveAs(_) => Expected::Destination,
            Operation::ExportFiles { .. } | Operation::ExportTree { .. } => Expected::Exported,
        };
        let invalid = (!matches!(operation, Operation::OpenCopy(_)) && document.is_none())
            || document.is_some_and(|d| !self.session.docs.iter().any(|tab| tab.uid == d.uid));
        let rejection = if invalid {
            Some(RejectReason::InvalidRequest("request target is not an open document".into()))
        } else if self.platform_requests.entries.len() >= MAX_PENDING {
            Some(RejectReason::Busy)
        } else {
            None
        };
        if let Some(reason) = rejection {
            self.platform_rejected(&reason);
            return Err(reason);
        }
        let id = allocate_id(&NEXT_ID).inspect_err(|reason| self.platform_rejected(reason))?;
        self.platform_requests.entries.insert(id, Pending { document, expected, done });
        self.services.set_request_repaint(ctx.clone());
        match self.services.submit_request(PlatformRequest { id, initiating_document: document, operation }) {
            Submission::Pending => {}
            Submission::Completed(result) => self.complete_platform_request(Completion { id, result }),
            Submission::Rejected(reason) => {
                self.platform_requests.entries.remove(&id);
                self.platform_rejected(&reason);
                return Err(reason);
            }
        }
        Ok(id)
    }

    fn platform_rejected(&mut self, reason: &RejectReason) {
        self.toast(format!("Platform request rejected: {reason:?}"));
        self.ui.last_error = Some(format!("Platform request rejected: {reason:?}"));
    }

    pub fn cancel_platform_request(&mut self, id: RequestId) {
        if self.platform_requests.entries.remove(&id).is_some() {
            self.services.cancel_request(id);
        }
    }

    pub fn cancel_platform_requests(&mut self) {
        for (id, _) in std::mem::take(&mut self.platform_requests.entries) {
            self.services.cancel_request(id);
        }
    }

    pub(crate) fn cancel_closed_platform_requests(&mut self) {
        let closed: Vec<_> = self
            .platform_requests
            .entries
            .iter()
            .filter(|(_, p)| p.document.is_some_and(|d| !self.session.docs.iter().any(|tab| tab.uid == d.uid)))
            .map(|(id, _)| *id)
            .collect();
        for id in closed {
            self.cancel_platform_request(id);
        }
    }

    pub fn poll_platform_requests(&mut self, ctx: &egui::Context) {
        self.cancel_closed_platform_requests();
        for _ in 0..MAX_COMPLETIONS_PER_FRAME {
            let Some(completion) = self.services.take_completion() else { return };
            self.complete_platform_request(completion);
            // A callback can close other pending documents.
            self.cancel_closed_platform_requests();
        }
        ctx.request_repaint();
    }

    fn complete_platform_request(&mut self, completion: Completion) {
        let Some(pending) = self.platform_requests.entries.remove(&completion.id) else { return };
        let payload = match completion.result {
            PlatformResult::Cancelled => return,
            PlatformResult::Failed(e) => {
                self.toast(e.clone());
                self.ui.last_error = Some(e);
                return;
            }
            PlatformResult::Completed(p) => p,
        };
        if !matches!(
            (pending.expected, &payload),
            (Expected::Selected, Payload::Selected(_)) | (Expected::Destination, Payload::Destination(_)) | (Expected::Exported, Payload::Exported)
        ) {
            self.platform_rejected(&RejectReason::InvalidRequest("completion payload does not match operation".into()));
            return;
        }
        let previous = self.session.ds().ok().map(|d| d.uid);
        if let Some(document) = pending.document {
            let Some(index) = self.session.docs.iter().position(|d| d.uid == document.uid) else { return };
            self.session.active = index;
        }
        (pending.done)(self, payload);
        // OpenCopy intentionally selects its newly opened document.
        if pending.document.is_some()
            && let Some(index) = previous.and_then(|uid| self.session.docs.iter().position(|d| d.uid == uid))
        {
            self.session.active = index;
        }
    }
}

impl Drop for FigApp {
    fn drop(&mut self) {
        self.cancel_platform_requests();
    }
}

/// Opt-in adapter for providers that implement the historical synchronous dialogs.
/// Inherited Services defaults never invoke this, so unavailable differs from cancellation.
pub fn submit_desktop(services: &mut (impl Services + ?Sized), request: PlatformRequest) -> Submission {
    let selected = |path: PathBuf| {
        let display_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        Payload::Selected(SelectedCopy { path, display_name, lease: None })
    };
    let picked = match request.operation {
        Operation::OpenCopy(p) | Operation::ImportCopy(p) => {
            services.pick_open(&p.title, &p.extensions.iter().map(String::as_str).collect::<Vec<_>>()).map(selected)
        }
        Operation::SaveAs(p) => services
            .pick_save(&p.title, p.suggested_name.as_deref().unwrap_or_default(), &p.extensions.iter().map(String::as_str).collect::<Vec<_>>())
            .map(Payload::Destination),
        Operation::ExportTree { picker, tree } => {
            if let Err(e) = tree.validate_sources() {
                return Submission::Completed(PlatformResult::Failed(e.to_string()));
            }
            let Some(dir) = services.pick_folder(&picker.title) else {
                return Submission::Completed(PlatformResult::Cancelled);
            };
            if let Err(e) = tree.copy_to(&dir) {
                return Submission::Completed(PlatformResult::Failed(e.to_string()));
            }
            Some(Payload::Exported)
        }
        Operation::ExportFiles { picker, files } => {
            let mut names = BTreeSet::new();
            if files.is_empty() || files.iter().any(|f| !valid_name(&f.display_name) || !names.insert(f.display_name.clone())) {
                return Submission::Rejected(RejectReason::InvalidRequest("export needs unique, single-component file names".into()));
            }
            let destinations = if let [file] = files.as_slice() {
                services
                    .pick_save(&picker.title, &file.display_name, &picker.extensions.iter().map(String::as_str).collect::<Vec<_>>())
                    .map(|path| vec![path])
            } else {
                services.pick_folder(&picker.title).map(|dir| files.iter().map(|f| dir.join(&f.display_name)).collect())
            };
            let Some(destinations) = destinations else { return Submission::Completed(PlatformResult::Cancelled) };
            for (file, destination) in files.iter().zip(destinations) {
                if let Err(e) = std::fs::copy(&file.path, destination) {
                    return Submission::Completed(PlatformResult::Failed(e.to_string()));
                }
            }
            Some(Payload::Exported)
        }
    };
    Submission::Completed(picked.map_or(PlatformResult::Cancelled, PlatformResult::Completed))
}

#[cfg(test)]
pub(crate) fn exhausted_id_for_test() -> Result<RequestId, RejectReason> {
    allocate_id(&AtomicU64::new(u64::MAX))
}
