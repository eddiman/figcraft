//! UIKit implementation of the D07 boundary; no native object escapes this module.
use crate::callback::contain;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSArray, NSFileCoordinator, NSFileCoordinatorReadingOptions, NSObject, NSObjectProtocol, NSString, NSURL};
use objc2_ui_kit::{
    UIAdaptivePresentationControllerDelegate, UIApplication, UIApplicationState, UIDocumentPickerDelegate, UIDocumentPickerViewController,
    UISceneActivationState, UIWindowScene,
};
use objc2_uniform_type_identifiers::UTType;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

#[derive(Debug)]
pub enum Outcome {
    Imported { path: PathBuf, name: String },
    Exported,
    Cancelled,
}
pub struct Completion {
    pub id: u64,
    pub result: Result<Outcome, String>,
}
#[derive(Clone)]
pub enum Operation {
    Import { extensions: Vec<String>, destination: PathBuf },
    Export { paths: Vec<PathBuf> },
}
struct State {
    id: u64,
    operation: Operation,
    terminal: Cell<bool>,
    presented: Cell<bool>,
    cancel_requested: Cell<bool>,
    sender: mpsc::Sender<Completion>,
    wake: Arc<dyn Fn() + Send + Sync>,
    _keep_alive: Box<dyn std::any::Any>,
}

define_class!(
    // SAFETY: NSObject has no subclassing invariants. Rust ivars use main-thread-only ownership.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "FigCraftCopyPickerDelegate"]
    #[ivars = State]
    struct Delegate;
    // SAFETY: NSObjectProtocol adds no invariants.
    unsafe impl NSObjectProtocol for Delegate {}
    // SAFETY: The delegate is main-thread-only and both method signatures match UIKit's protocol.
    unsafe impl UIDocumentPickerDelegate for Delegate {
        #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
        fn picked(&self, controller: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
            // Coordinated provider access can reenter the native run loop. Retain both
            // callback owners before any such call, independently of the Rust host.
            let _callback_owner: Retained<Self> = Retained::from(self);
            let _picker_owner: Retained<UIDocumentPickerViewController> = Retained::from(controller);
            if self.ivars().terminal.get() {
                return;
            }
            self.ivars().presented.set(true);
            if self.ivars().cancel_requested.get() {
                self.request_cancel(controller);
                return;
            }
            let result = contain(|| {
                let url = urls.firstObject().ok_or("picker returned no file")?;
                match &self.ivars().operation {
                    Operation::Import { extensions, destination } => copy_import(&url, extensions, destination),
                    Operation::Export { .. } => Ok(Outcome::Exported),
                }
            });
            self.finish(controller, result);
        }
        #[unsafe(method(documentPickerWasCancelled:))]
        fn cancelled(&self, controller: &UIDocumentPickerViewController) {
            self.ivars().presented.set(true);
            self.request_cancel(controller);
        }
    }
    // SAFETY: UIKit invokes this main-thread-only protocol after interactive dismissal.
    unsafe impl UIAdaptivePresentationControllerDelegate for Delegate {
        #[unsafe(method(presentationControllerDidDismiss:))]
        fn dismissed(&self, controller: &objc2_ui_kit::UIPresentationController) {
            if let Ok(picker) = controller.presentedViewController().downcast::<UIDocumentPickerViewController>() {
                self.ivars().presented.set(true);
                self.request_cancel(&picker);
            }
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, state: State) -> Retained<Self> {
        let object = Self::alloc(mtm).set_ivars(state);
        // SAFETY: NSObject's designated initializer returns the allocated subclass with its Rust ivars intact.
        unsafe { msg_send![super(object), init] }
    }
    fn request_cancel(&self, picker: &UIDocumentPickerViewController) {
        self.ivars().cancel_requested.set(true);
        if self.ivars().presented.get() {
            self.finish(picker, Ok(Outcome::Cancelled));
        }
    }
    fn finish(&self, picker: &UIDocumentPickerViewController, result: Result<Outcome, String>) {
        if self.ivars().terminal.replace(true) {
            return;
        }
        let state = self.ivars();
        let sender = state.sender.clone();
        let wake = state.wake.clone();
        let id = state.id;
        let result = RefCell::new(Some(result));
        // Strong native references outlive Host::drop until native dismissal completes.
        let hold_delegate: Retained<Self> = Retained::from(self);
        let hold_picker: Retained<UIDocumentPickerViewController> = Retained::from(picker);
        let done = block2::RcBlock::new(move || {
            let _hold = (&hold_delegate, &hold_picker);
            let result = result.try_borrow_mut().ok().and_then(|mut r| r.take());
            if let Some(result) = result {
                let _ = sender.send(Completion { id, result });
                let _ = contain(|| {
                    wake();
                    Ok(())
                });
            }
        });
        if picker.presentingViewController().is_some() {
            picker.dismissViewControllerAnimated_completion(false, Some(&done));
        } else {
            done.call(());
        }
    }
}

struct Scope<'a> {
    url: &'a NSURL,
    started: bool,
}
impl Drop for Scope<'_> {
    fn drop(&mut self) {
        if self.started {
            // SAFETY: Balance exactly the successful start call while the retained callback URL is alive.
            unsafe {
                self.url.stopAccessingSecurityScopedResource();
            }
        }
    }
}

fn copy_import(url: &NSURL, extensions: &[String], destination: &Path) -> Result<Outcome, String> {
    if !url.isFileURL() {
        return Err("provider did not return a file URL".into());
    }
    // SAFETY: This URL came directly from the picker. The scoped guard balances a successful start.
    let _scope = Scope { url, started: unsafe { url.startAccessingSecurityScopedResource() } };
    let result = RefCell::new(None);
    let block = block2::RcBlock::new(|source: std::ptr::NonNull<NSURL>| {
        // SAFETY: NSFileCoordinator supplies a live, non-null URL for the duration of this synchronous accessor.
        let source = unsafe { source.as_ref() };
        let copied = contain(|| {
            let path = PathBuf::from(source.path().ok_or("provider URL has no path")?.to_string());
            let name = path.file_name().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).ok_or("invalid provider file name")?.to_string();
            if !extensions.is_empty()
                && !path.extension().and_then(|s| s.to_str()).is_some_and(|ext| extensions.iter().any(|v| v.eq_ignore_ascii_case(ext)))
            {
                return Err("unsupported selected file type".into());
            }
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err("selected item is not a regular file".into());
            }
            let target = destination.join(&name);
            let mut input = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
            Ok(Outcome::Imported { path: target, name })
        });
        if let Ok(mut result) = result.try_borrow_mut() {
            *result = Some(copied);
        }
    });
    let coordinator = NSFileCoordinator::initWithFilePresenter(NSFileCoordinator::alloc(), None);
    let mut error = None;
    coordinator.coordinateReadingItemAtURL_options_error_byAccessor(url, NSFileCoordinatorReadingOptions::empty(), Some(&mut error), &block);
    if let Some(error) = error {
        return Err(error.localizedDescription().to_string());
    }
    drop(block);
    result.into_inner().ok_or("file provider did not grant coordinated access")?
}

#[derive(Debug)]
pub enum PresentationError {
    Busy,
    NoForegroundPresenter,
    InvalidRequest(String),
}
impl std::fmt::Display for PresentationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => write!(f, "another modal is visible"),
            Self::NoForegroundPresenter => write!(f, "no foreground presenter"),
            Self::InvalidRequest(s) => write!(f, "{s}"),
        }
    }
}
impl From<&str> for PresentationError {
    fn from(s: &str) -> Self {
        Self::InvalidRequest(s.into())
    }
}

struct Active {
    picker: Retained<UIDocumentPickerViewController>,
    delegate: Retained<Delegate>,
    id: u64,
}
pub struct Host {
    active: Option<Active>,
    sender: mpsc::Sender<Completion>,
    receiver: mpsc::Receiver<Completion>,
    mtm: MainThreadMarker,
}
impl Host {
    pub fn new() -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("picker host must be created on the main thread")?;
        let (sender, receiver) = mpsc::channel();
        Ok(Self { active: None, sender, receiver, mtm })
    }
    pub fn present(
        &mut self,
        id: u64,
        operation: Operation,
        wake: Arc<dyn Fn() + Send + Sync>,
        start: Option<&Path>,
        keep_alive: Box<dyn std::any::Any>,
    ) -> Result<(), PresentationError> {
        if self.active.is_some() {
            return Err(PresentationError::Busy);
        }
        let app = UIApplication::sharedApplication(self.mtm);
        if app.applicationState() != UIApplicationState::Active {
            return Err(PresentationError::NoForegroundPresenter);
        }
        let mut presenter = None;
        for scene in app.connectedScenes().iter() {
            if scene.activationState() != UISceneActivationState::ForegroundActive {
                continue;
            }
            if let Ok(scene) = scene.downcast::<UIWindowScene>() {
                for window in scene.windows().iter() {
                    if window.isKeyWindow() {
                        presenter = window.rootViewController();
                        break;
                    }
                }
            }
        }
        if presenter.is_none() {
            // The pinned winit host also supports the pre-scene single-window lifecycle.
            #[allow(deprecated)]
            for window in app.windows().iter() {
                if window.isKeyWindow() {
                    presenter = window.rootViewController();
                    break;
                }
            }
        }
        let presenter = presenter.ok_or(PresentationError::NoForegroundPresenter)?;
        if presenter.presentedViewController().is_some() {
            return Err(PresentationError::Busy);
        }
        let picker = match &operation {
            Operation::Import { extensions, .. } => {
                let mut types = Vec::new();
                let mut fallback = extensions.is_empty();
                for extension in extensions {
                    if let Some(kind) = UTType::typeWithFilenameExtension(&NSString::from_str(extension)) {
                        types.push(kind);
                    } else {
                        fallback = true;
                    }
                }
                if fallback {
                    types.push(UTType::typeWithIdentifier(&NSString::from_str("public.data")).ok_or("public.data is unavailable")?);
                }
                let types = NSArray::from_retained_slice(&types);
                UIDocumentPickerViewController::initForOpeningContentTypes_asCopy(UIDocumentPickerViewController::alloc(self.mtm), &types, true)
            }
            Operation::Export { paths } => {
                if paths.is_empty() {
                    return Err("export has no files".into());
                }
                let urls = paths.iter().map(|path| NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))).collect::<Vec<_>>();
                UIDocumentPickerViewController::initForExportingURLs_asCopy(
                    UIDocumentPickerViewController::alloc(self.mtm),
                    &NSArray::from_retained_slice(&urls),
                    true,
                )
            }
        };
        if let Some(start) = start {
            picker.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(&start.to_string_lossy()))));
        }
        picker.setAllowsMultipleSelection(false);
        let delegate = Delegate::new(
            self.mtm,
            State {
                id,
                operation,
                terminal: Cell::new(false),
                presented: Cell::new(false),
                cancel_requested: Cell::new(false),
                sender: self.sender.clone(),
                wake,
                _keep_alive: keep_alive,
            },
        );
        picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let shown_delegate = delegate.clone();
        let shown_picker = picker.clone();
        let shown = block2::RcBlock::new(move || {
            if let Some(presentation) = shown_picker.presentationController() {
                // SAFETY: The strong delegate owner implements this main-thread protocol; UIKit holds only a weak reference.
                unsafe {
                    presentation.setDelegate(Some(ProtocolObject::from_ref(&*shown_delegate)));
                }
            }
            shown_delegate.ivars().presented.set(true);
            if shown_delegate.ivars().cancel_requested.get() {
                shown_delegate.request_cancel(&shown_picker);
            }
        });
        presenter.presentViewController_animated_completion(&picker, true, Some(&shown));
        self.active = Some(Active { picker, delegate, id });
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Completion> {
        let result = self.receiver.try_recv().ok()?;
        if self.active.as_ref().is_some_and(|a| a.id == result.id) {
            self.active = None;
        }
        Some(result)
    }
    pub fn cancel(&mut self, id: u64) {
        if let Some(active) = &self.active
            && active.id == id
        {
            active.delegate.request_cancel(&active.picker);
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        if let Some(active) = &self.active {
            self.cancel(active.id);
        }
    }
}
