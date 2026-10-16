//! UI-thread request queue. Native objects remain inside figcraft-apple-ipad.
use crate::documents::{ExportCopy, ImportCopy};
use figcraft_apple_ipad::{Host, Operation as NativeOperation, Outcome, PresentationError};
use figcraft_ui_egui::platform_requests::{
    Completion, Operation, Payload, PlatformRequest, PlatformResult, RejectReason, RequestId, SelectedCopy, Submission,
};
use figcraft_ui_egui::{Services, platform::PlatformCapabilities};
use std::cell::Cell;
use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
struct Prepared {
    request: PlatformRequest,
    native: NativeOperation,
    import: Option<Arc<ImportCopy>>,
    _export: Option<Arc<ExportCopy>>,
    cancelled: Cell<bool>,
}
pub struct IPadServices {
    host: Host,
    recovery: crate::recovery::Shared,
    imports: PathBuf,
    context: Option<egui::Context>,
    queued: VecDeque<Rc<Prepared>>,
    active: Option<Rc<Prepared>>,
    completed: VecDeque<Completion>,
}
impl IPadServices {
    pub fn new(recovery: crate::recovery::Shared) -> Result<Self, Box<dyn std::error::Error>> {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| std::io::Error::other("application home is unavailable"))?;
        Ok(Self {
            recovery,
            host: Host::new().map_err(std::io::Error::other)?,
            imports: home.join("Library/Application Support/FigCraft/Imports"),
            context: None,
            queued: VecDeque::new(),
            active: None,
            completed: VecDeque::new(),
        })
    }
    fn prepare(&self, request: PlatformRequest) -> Result<Rc<Prepared>, String> {
        let mut import = None;
        let mut export = None;
        let native = match &request.operation {
            Operation::OpenCopy(picker) | Operation::ImportCopy(picker) => {
                let owner = ImportCopy::create(&self.imports).map_err(|e| e.to_string())?;
                let destination = owner.directory().to_path_buf();
                import = Some(owner);
                NativeOperation::Import { extensions: picker.extensions.clone(), destination }
            }
            Operation::ExportFiles { files, .. } => {
                let mut names = BTreeSet::new();
                if files.is_empty() || files.len() > 1000 {
                    return Err("export needs 1–1000 files".into());
                }
                let mut paths = Vec::new();
                for file in files {
                    if !names.insert(file.display_name.to_lowercase())
                        || file.lease.file(file.display_name.clone()).map_err(|e| format!("{e:?}"))?.path != file.path
                    {
                        return Err("export has invalid or duplicate file names".into());
                    }
                    let metadata = std::fs::symlink_metadata(&file.path).map_err(|e| e.to_string())?;
                    if !metadata.is_file() || metadata.file_type().is_symlink() {
                        return Err("export is not a staged regular file".into());
                    }
                    paths.push(file.path.clone());
                }
                NativeOperation::Export { paths }
            }
            Operation::ExportTree { tree, .. } => {
                let files = tree
                    .paths()
                    .iter()
                    .map(|name| tree.source_path(name).map(|path| (name.clone(), path)).ok_or_else(|| "missing declared export source".to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let owner = ExportCopy::create(&std::env::temp_dir(), &files).map_err(|e| e.to_string())?;
                let paths = vec![owner.folder().to_path_buf()];
                export = Some(owner);
                NativeOperation::Export { paths }
            }
            Operation::SaveAs(_) => return Err("native durable document saving is not connected".into()),
        };
        Ok(Rc::new(Prepared { request, native, import, _export: export, cancelled: Cell::new(false) }))
    }
    fn advance(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(prepared) = self.queued.pop_front() else { return };
        let context = self.context.clone();
        let wake = Arc::new(move || {
            if let Some(context) = &context {
                context.request_repaint();
            }
        });
        match self.host.present(prepared.request.id.0, prepared.native.clone(), wake, None, Box::new(prepared.clone())) {
            Ok(()) => self.active = Some(prepared),
            Err(PresentationError::Busy | PresentationError::NoForegroundPresenter) => {
                self.queued.push_front(prepared);
                if let Some(context) = &self.context {
                    context.request_repaint_after(std::time::Duration::from_millis(250));
                }
            }
            Err(e) => self.completed.push_back(Completion { id: prepared.request.id, result: PlatformResult::Failed(e.to_string()) }),
        }
    }
    fn drain(&mut self) {
        for _ in 0..8 {
            let Some(done) = self.host.poll() else { break };
            if !self.active.as_ref().is_some_and(|p| p.request.id.0 == done.id) {
                continue;
            }
            let Some(prepared) = self.active.take() else { continue };
            let result = if prepared.cancelled.get() {
                PlatformResult::Cancelled
            } else {
                match done.result {
                    Ok(Outcome::Cancelled) => PlatformResult::Cancelled,
                    Ok(Outcome::Exported) => PlatformResult::Completed(Payload::Exported),
                    Ok(Outcome::Imported { path, name }) => match prepared
                        .import
                        .as_ref()
                        .ok_or_else(|| "import has no owned directory".to_string())
                        .and_then(|owner| owner.promote(&path).map_err(|e| e.to_string()))
                    {
                        Ok(()) => PlatformResult::Completed(Payload::Selected(SelectedCopy { path, display_name: name, lease: None })),
                        Err(e) => PlatformResult::Failed(e),
                    },
                    Err(e) => PlatformResult::Failed(e),
                }
            };
            self.completed.push_back(Completion { id: prepared.request.id, result });
        }
        self.advance();
    }
}
impl Services for IPadServices {
    fn emergency_save(&mut self, session: &figcraft_engine::Session) {
        crate::lifecycle::emergency(&self.recovery, session);
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::ipad()
    }
    fn set_request_repaint(&mut self, ctx: egui::Context) {
        self.context = Some(ctx);
    }
    fn submit_request(&mut self, request: PlatformRequest) -> Submission {
        if matches!(request.operation, Operation::SaveAs(_)) {
            return Submission::Rejected(RejectReason::Unsupported);
        }
        if self.queued.len() + usize::from(self.active.is_some()) + self.completed.len() >= 8 {
            return Submission::Rejected(RejectReason::Busy);
        }
        match self.prepare(request) {
            Ok(prepared) => {
                self.queued.push_back(prepared);
                self.advance();
                Submission::Pending
            }
            Err(e) => Submission::Completed(PlatformResult::Failed(e)),
        }
    }
    fn take_completion(&mut self) -> Option<Completion> {
        self.drain();
        self.completed.pop_front()
    }
    fn cancel_request(&mut self, id: RequestId) {
        if let Some(position) = self.queued.iter().position(|p| p.request.id == id) {
            self.queued.remove(position);
        }
        self.completed.retain(|p| p.id != id);
        if let Some(active) = &self.active
            && active.request.id == id
        {
            active.cancelled.set(true);
            self.host.cancel(id.0);
        }
    }
}
