//! Owned import directories and export folders. No native handles or engine state.
use std::collections::BTreeSet;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
fn unique_directory(parent: &Path, prefix: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    let metadata = std::fs::symlink_metadata(parent)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other("copy store must be an owned directory"));
    }
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    for _ in 0..32 {
        let id = NEXT.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| io::Error::other("copy IDs exhausted"))?;
        let path = parent.join(format!("{prefix}-{time}-{}-{id}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("cannot allocate a unique copy directory"))
}
fn cleanup(path: &Path) {
    if let Err(e) = std::fs::remove_dir_all(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        log::warn!("copy cleanup {}: {e}", path.display());
    }
}
pub struct ImportCopy {
    directory: PathBuf,
    promoted: AtomicBool,
}
impl ImportCopy {
    pub fn create(parent: &Path) -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self { directory: unique_directory(parent, "import")?, promoted: AtomicBool::new(false) }))
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn promote(&self, path: &Path) -> io::Result<()> {
        if path.parent() != Some(self.directory.as_path()) {
            return Err(io::Error::other("import escaped its owned directory"));
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::other("import is not a regular file"));
        }
        self.promoted.store(true, Ordering::Release);
        Ok(())
    }
}
impl Drop for ImportCopy {
    fn drop(&mut self) {
        if !self.promoted.load(Ordering::Acquire) {
            cleanup(&self.directory);
        }
    }
}
pub struct ExportCopy {
    directory: PathBuf,
    folder: PathBuf,
}
impl ExportCopy {
    pub fn create(parent: &Path, files: &[(String, PathBuf)]) -> io::Result<Arc<Self>> {
        let mut names = BTreeSet::new();
        if files.is_empty() || files.len() > 1000 || files.iter().any(|(name, _)| !valid_relative(name) || !names.insert(name.to_lowercase())) {
            return Err(io::Error::other("export needs unique validated relative paths"));
        }
        let directory = unique_directory(parent, "export")?;
        let copy = Arc::new(Self { folder: directory.join("Export"), directory });
        std::fs::create_dir(&copy.folder)?;
        for (name, source) in files {
            check_source(source, name)?;
            let target = copy.folder.join(name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut source = std::fs::File::open(source)?;
            let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(target)?;
            std::io::copy(&mut source, &mut output)?;
            output.sync_all()?;
        }
        Ok(copy)
    }
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}
impl Drop for ExportCopy {
    fn drop(&mut self) {
        cleanup(&self.directory);
    }
}
fn valid_relative(name: &str) -> bool {
    !name.contains(['\\', ':'])
        && name.split('/').count() <= 32
        && name.split('/').all(|p| !p.is_empty() && p != "." && p != ".." && !p.chars().any(char::is_control))
}
fn check_source(source: &Path, relative: &str) -> io::Result<()> {
    if !source.ends_with(relative) {
        return Err(io::Error::other("export source does not match its declared path"));
    }
    let mut root = source.to_path_buf();
    for _ in relative.split('/') {
        root.pop();
    }
    let metadata = std::fs::symlink_metadata(&root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other("export staging root is not a directory"));
    }
    let mut parts = relative.split('/').peekable();
    while let Some(part) = parts.next() {
        root.push(part);
        let metadata = std::fs::symlink_metadata(&root)?;
        let valid = if parts.peek().is_some() { metadata.is_dir() } else { metadata.is_file() };
        if !valid || metadata.file_type().is_symlink() {
            return Err(io::Error::other("export source contains a symlink or wrong file type"));
        }
    }
    Ok(())
}

/// Persistent owned-copy identity, independent of an engine session's document UID.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DocumentId(String);

impl DocumentId {
    fn parse(value: &str) -> io::Result<Self> {
        let parts: Vec<_> = value.split('-').collect();
        if value.len() > 96
            || parts.len() != 4
            || parts.first() != Some(&"import")
            || parts.iter().skip(1).any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(io::Error::other("invalid owned document ID"));
        }
        Ok(Self(value.to_owned()))
    }
}

const MAX_RECENTS: usize = 256;
const MAX_RECENTS_BYTES: u64 = 64 * 1024;

/// The on-disk index contains a schema version and (document ID, file name) pairs,
/// never an absolute container prefix. Display paths are resolved for this launch.
pub struct OwnedRecents {
    root: PathBuf,
}

impl OwnedRecents {
    pub fn open(imports: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(imports)?;
        let metadata = std::fs::symlink_metadata(imports)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::other("owned recents root must be a directory"));
        }
        Ok(Self { root: std::fs::canonicalize(imports)? })
    }

    pub fn directory(&self) -> &Path {
        &self.root
    }

    pub fn validate_document(&self, path: &Path) -> io::Result<PathBuf> {
        let (id, name) = self.reference(path)?;
        self.resolve(&id, &name)
    }

    fn reference(&self, path: &Path) -> io::Result<(DocumentId, String)> {
        let relative = path.strip_prefix(&self.root).map_err(|_| io::Error::other("recent document is outside owned storage"))?;
        let parts: Vec<_> = relative.components().collect();
        let [std::path::Component::Normal(id), std::path::Component::Normal(name)] = parts.as_slice() else {
            return Err(io::Error::other("recent document must have an owned ID and file name"));
        };
        let id = DocumentId::parse(id.to_str().ok_or_else(|| io::Error::other("document ID is not UTF-8"))?)?;
        let name = name.to_str().ok_or_else(|| io::Error::other("document file name is not UTF-8"))?;
        Ok((id, name.to_owned()))
    }

    fn resolve(&self, id: &DocumentId, name: &str) -> io::Result<PathBuf> {
        if name.is_empty() || name.len() > 255 || name.contains(['/', '\\', ':']) || name == "." || name == ".." || name.chars().any(char::is_control)
        {
            return Err(io::Error::other("invalid owned document file name"));
        }
        let directory = self.root.join(&id.0);
        let metadata = std::fs::symlink_metadata(&directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::other("owned document directory was replaced"));
        }
        let path = directory.join(name);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::other("owned document is not a regular file"));
        }
        Ok(path)
    }

    pub fn load(&self) -> io::Result<Vec<String>> {
        let index = self.root.join(".recents.json");
        let metadata = match std::fs::symlink_metadata(&index) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_RECENTS_BYTES {
            return Err(io::Error::other("invalid or oversized owned recents index"));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(index)?.take(MAX_RECENTS_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_RECENTS_BYTES {
            return Err(io::Error::other("owned recents index exceeds its budget"));
        }
        let (version, records): (u32, Vec<(String, String)>) = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if version != 1 || records.len() > MAX_RECENTS {
            return Err(io::Error::other("unsupported owned recents index"));
        }
        let mut seen = BTreeSet::new();
        let mut paths = Vec::new();
        for (id, name) in records {
            let id = DocumentId::parse(&id)?;
            if !seen.insert((id.clone(), name.clone())) {
                return Err(io::Error::other("duplicate owned recents entry"));
            }
            match self.resolve(&id, &name) {
                Ok(path) => paths.push(path.into_os_string().into_string().map_err(|_| io::Error::other("owned document path is not UTF-8"))?),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    log::warn!("Recent owned document {} is missing: {error}", id.0);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(paths)
    }

    pub fn save(&self, paths: &[String]) -> io::Result<()> {
        if paths.len() > MAX_RECENTS {
            return Err(io::Error::other("too many owned recent documents"));
        }
        let mut records = Vec::new();
        let mut seen = BTreeSet::new();
        for path in paths {
            let (id, name) = self.reference(Path::new(path))?;
            self.resolve(&id, &name)?;
            if seen.insert((id.clone(), name.clone())) {
                records.push((id.0, name));
            }
        }
        let bytes = serde_json::to_vec(&(1u32, records)).map_err(io::Error::other)?;
        if bytes.len() as u64 > MAX_RECENTS_BYTES {
            return Err(io::Error::other("owned recents index exceeds its budget"));
        }
        // Reuse the copy guard so a failed write removes only this new staging directory.
        let staging = ImportCopy::create(&self.root)?;
        let staged = staging.directory().join("recents.json");
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&staged)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(staged, self.root.join(".recents.json"))?;
        std::fs::File::open(&self.root)?.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipad_owned_copy_names_do_not_collide() {
        let one = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let two = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let p = one.directory().join("same.svg");
        let q = two.directory().join("same.svg");
        std::fs::write(&p, b"one").unwrap();
        std::fs::write(&q, b"two").unwrap();
        assert_ne!(p, q);
        assert_eq!(std::fs::read(p).unwrap(), b"one");
        assert_eq!(std::fs::read(q).unwrap(), b"two");
    }
    #[test]
    fn ipad_import_promotion_keeps_copy_and_cancellation_cleans_up() {
        let one = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let root = one.directory().to_path_buf();
        assert!(one.promote(&root.join("absent")).is_err());
        drop(one);
        assert!(!root.exists());
        let two = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let root = two.directory().to_path_buf();
        let file = root.join("copy.svg");
        std::fs::write(&file, b"persisted").unwrap();
        two.promote(&file).unwrap();
        drop(two);
        assert_eq!(std::fs::read(file).unwrap(), b"persisted");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ipad_export_cleanup_waits_for_completion() {
        let source = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let file = source.directory().join("close.svg");
        std::fs::write(&file, b"captured").unwrap();
        let package = ExportCopy::create(&std::env::temp_dir(), &[("close.svg".into(), file)]).unwrap();
        let output = package.folder().join("close.svg");
        let native_owner = package.clone();
        drop(package);
        assert_eq!(std::fs::read(&output).unwrap(), b"captured");
        drop(native_owner);
        assert!(!output.exists());
    }
    #[test]
    fn ipad_export_folder_preserves_nested_paths_and_rejects_traversal() {
        let source = ImportCopy::create(&std::env::temp_dir()).unwrap();
        std::fs::create_dir(source.directory().join("icons")).unwrap();
        let file = source.directory().join("icons/close.svg");
        std::fs::write(&file, b"nested").unwrap();
        let copy = ExportCopy::create(&std::env::temp_dir(), &[("icons/close.svg".into(), file.clone())]).unwrap();
        assert_eq!(std::fs::read(copy.folder().join("icons/close.svg")).unwrap(), b"nested");
        for name in ["../escape", "/root", "a//b", "a/./b", "a\\b"] {
            assert!(ExportCopy::create(&std::env::temp_dir(), &[(name.into(), file.clone())]).is_err());
        }
    }

    #[test]
    fn ipad_recents_survive_container_path_change() {
        let container = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let original = container.directory().join("old-container");
        let relocated = container.directory().join("new-container");
        let store = OwnedRecents::open(&original).unwrap();
        let copy = ImportCopy::create(store.directory()).unwrap();
        let path = copy.directory().join("synthetic.svg");
        std::fs::write(&path, b"<svg/>").unwrap();
        copy.promote(&path).unwrap();
        let id = DocumentId::parse(copy.directory().file_name().unwrap().to_str().unwrap()).unwrap();
        store.save(&[path.to_str().unwrap().into()]).unwrap();
        let encoded = std::fs::read_to_string(original.join(".recents.json")).unwrap();
        assert!(!encoded.contains(original.to_str().unwrap()));
        drop(copy);
        drop(store);
        std::fs::rename(original, &relocated).unwrap();
        let reopened = OwnedRecents::open(&relocated).unwrap();
        let expected = reopened.directory().join(&id.0).join("synthetic.svg");
        assert_eq!(reopened.load().unwrap(), [expected.to_str().unwrap()]);
        assert_eq!(std::fs::read(expected).unwrap(), b"<svg/>");
    }

    #[test]
    fn ipad_recents_invalid_write_preserves_previous_index() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let store = OwnedRecents::open(root.directory()).unwrap();
        let copy = ImportCopy::create(store.directory()).unwrap();
        let path = copy.directory().join("synthetic.figcraft");
        std::fs::write(&path, b"owned").unwrap();
        store.save(&[path.to_str().unwrap().into()]).unwrap();
        let before = std::fs::read(root.directory().join(".recents.json")).unwrap();
        assert!(store.save(&["/outside/unowned.figcraft".into()]).is_err());
        assert_eq!(std::fs::read(root.directory().join(".recents.json")).unwrap(), before);
        assert_eq!(store.load().unwrap(), [path.to_str().unwrap()]);
    }

    #[test]
    fn ipad_recents_reject_corrupt_oversized_and_traversing_indexes() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let store = OwnedRecents::open(root.directory()).unwrap();
        let index = root.directory().join(".recents.json");
        for bytes in [
            b"not json".to_vec(),
            br#"[2,[]]"#.to_vec(),
            br#"[1,[["../escape","document.figcraft"]]]"#.to_vec(),
            vec![b' '; MAX_RECENTS_BYTES as usize + 1],
        ] {
            std::fs::write(&index, bytes).unwrap();
            assert!(store.load().is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn ipad_recents_reject_replaced_document_directory() {
        let root = ImportCopy::create(&std::env::temp_dir()).unwrap();
        let store = OwnedRecents::open(root.directory()).unwrap();
        let outside = ImportCopy::create(&std::env::temp_dir()).unwrap();
        std::fs::write(outside.directory().join("synthetic.svg"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.directory(), root.directory().join("import-1-2-3")).unwrap();
        std::fs::write(root.directory().join(".recents.json"), br#"[1,[["import-1-2-3","synthetic.svg"]]]"#).unwrap();
        assert!(store.load().is_err());
    }
}
