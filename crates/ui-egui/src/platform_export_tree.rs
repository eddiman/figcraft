//! Immutable names and shared ownership for a batch export's staged directory.
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::platform_requests::{RejectReason, StagingLease};

#[derive(Debug)]
pub struct ExportTree {
    lease: Arc<StagingLease>,
    paths: Vec<String>,
}

impl ExportTree {
    /// Names use `/` on every platform. No normalization can hide invalid segments.
    pub fn new(lease: Arc<StagingLease>, paths: Vec<String>) -> Result<Self, RejectReason> {
        let invalid = || RejectReason::InvalidRequest("export needs 1–1000 unique relative file paths, at most 32 components deep".into());
        if paths.is_empty() || paths.len() > 1000 {
            return Err(invalid());
        }
        let mut names = BTreeSet::new();
        for path in &paths {
            let parts: Vec<_> = path.split('/').collect();
            if path.len() > 32 * 1024
                || parts.len() > 32
                || parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.chars().any(|c| c.is_control() || "\\:*?\"<>|".contains(c)))
                || !names.insert(path.to_lowercase())
            {
                return Err(invalid());
            }
        }
        for name in &names {
            for (at, _) in name.match_indices('/') {
                if names.contains(&name[..at]) {
                    return Err(invalid());
                }
            }
        }
        Ok(Self { lease, paths })
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Only declared names can yield a source path. The caller writes captured bytes here.
    pub fn source_path(&self, relative: &str) -> Option<PathBuf> {
        self.paths.iter().any(|p| p == relative).then(|| self.lease.root().join(relative))
    }

    pub(crate) fn validate_sources(&self) -> io::Result<()> {
        for path in &self.paths {
            check_path(self.lease.root(), path, false)?;
        }
        Ok(())
    }

    /// Preflight before writing, then check again before each copy. This rejects observed
    /// symlinks; it does not lock ancestors against concurrent filesystem mutation.
    pub(crate) fn copy_to(&self, destination: &Path) -> io::Result<()> {
        self.validate_sources()?;
        let destination = destination.canonicalize()?;
        let source_root = self.lease.root().canonicalize()?;
        if destination.starts_with(&source_root) {
            return Err(io::Error::other("export destination is inside its staging directory"));
        }
        for path in &self.paths {
            check_path(&destination, path, true)?;
            if destination.join(path).canonicalize().ok() == Some(source_root.join(path)) {
                return Err(io::Error::other("export destination is its staged source"));
            }
        }
        for path in &self.paths {
            check_path(self.lease.root(), path, false)?;
            check_path(&destination, path, true)?;
            let target = destination.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(self.lease.root().join(path), target)?;
        }
        Ok(())
    }
}

fn check_path(root: &Path, relative: &str, allow_missing: bool) -> io::Result<()> {
    if !std::fs::symlink_metadata(root)?.is_dir() {
        return Err(io::Error::other("export root must be a directory, not a symlink"));
    }
    let mut current = root.to_path_buf();
    let mut parts = relative.split('/').peekable();
    while let Some(part) = parts.next() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                let expected = if parts.peek().is_some() { metadata.is_dir() } else { metadata.is_file() };
                if !expected || metadata.file_type().is_symlink() {
                    return Err(io::Error::other(format!("export path is a symlink or has the wrong file type: {}", current.display())));
                }
            }
            Err(e) if allow_missing && e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
