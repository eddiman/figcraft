//! Owned import directories and export folders. No native handles or engine state.
use std::collections::BTreeSet;
use std::io;
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
        let id = NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| io::Error::other("copy IDs exhausted"))?;
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
}
