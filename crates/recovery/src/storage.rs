use crate::{Durability, Payload};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
pub(crate) const LOCK: &str = "lock";
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    ArchiveWritten,
    NoteWritten,
    ManifestRenamed,
}
#[derive(Clone, Default)]
pub(crate) struct Hooks {
    #[cfg(test)]
    pub callback: Option<Arc<dyn Fn(Phase) -> io::Result<()> + Send + Sync>>,
}
impl Hooks {
    fn at(&self, phase: Phase) -> io::Result<()> {
        #[cfg(test)]
        if let Some(callback) = &self.callback {
            return callback(phase);
        }
        let _ = phase;
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    generation: String,
    online: bool,
}
fn error(text: &str) -> io::Error {
    io::Error::other(text)
}
pub(crate) fn regular(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}
pub(crate) fn directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}
fn sync(path: &Path, policy: Durability) -> io::Result<()> {
    if policy == Durability::Synced {
        File::open(path)?.sync_all()?;
    }
    Ok(())
}
pub(crate) fn root(path: &Path, policy: Durability) -> io::Result<()> {
    let mut ancestor = path;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| error("recovery root has no parent"))?;
    }
    std::fs::create_dir_all(path)?;
    if !directory(path) {
        return Err(error("recovery root is not an owned directory"));
    }
    let mut current = path;
    loop {
        sync(current, policy)?;
        if current == ancestor {
            break;
        }
        current = current.parent().ok_or_else(|| error("recovery root has no parent"))?;
    }
    Ok(())
}
fn child(parent: &Path, name: &str, policy: Durability) -> io::Result<PathBuf> {
    let path = parent.join(name);
    match std::fs::create_dir(&path) {
        Ok(()) => sync(parent, policy)?,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && directory(&path) => {}
        Err(e) => return Err(e),
    }
    Ok(path)
}
pub(crate) fn unique(parent: &Path, prefix: &str) -> io::Result<PathBuf> {
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    for _ in 0..32 {
        let id = NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1)).map_err(|_| error("recovery IDs exhausted"))?;
        let path = parent.join(format!("{prefix}-{}-{time}-{id}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(error("no unique recovery directory"))
}
fn write_new(path: &Path, bytes: &[u8], policy: Durability) -> io::Result<()> {
    let mut file = File::options().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    if policy == Durability::Synced {
        file.sync_all()?;
    }
    Ok(())
}
fn bounded(path: &Path, maximum: u64) -> io::Result<Vec<u8>> {
    if !regular(path) {
        return Err(error("checkpoint member is not a regular file"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(error("checkpoint metadata is too large"));
    }
    Ok(bytes)
}
fn generation_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("g-") else { return false };
    let parts = suffix.split('-').collect::<Vec<_>>();
    name.len() <= 128 && parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}
fn manifest(uid: &Path) -> io::Result<Manifest> {
    let m: Manifest = serde_json::from_slice(&bounded(&uid.join("current"), 4096)?).map_err(io::Error::other)?;
    if m.version != 1 || !generation_name(&m.generation) {
        return Err(error("invalid checkpoint generation"));
    }
    Ok(m)
}
#[derive(Clone)]
pub(crate) struct CheckpointRef {
    pub archive: PathBuf,
    pub online: Option<String>,
    pub retain: bool,
    cleanup: PathBuf,
    generated: bool,
}
impl CheckpointRef {
    pub fn generated(uid: &Path) -> io::Result<Self> {
        if !directory(uid) || !directory(&uid.join("generations")) {
            return Err(error("invalid checkpoint directory"));
        }
        let m = manifest(uid)?;
        let generation = uid.join("generations").join(m.generation);
        if !directory(&generation) || !regular(&generation.join("snapshot.figcraft")) {
            return Err(error("incomplete checkpoint generation"));
        }
        let online =
            if m.online { Some(String::from_utf8(bounded(&generation.join("snapshot.online"), 16384)?).map_err(io::Error::other)?) } else { None };
        Ok(Self { archive: generation.join("snapshot.figcraft"), online, retain: false, cleanup: uid.to_path_buf(), generated: true })
    }
    pub fn legacy(path: &Path) -> io::Result<Self> {
        if !regular(path) {
            return Err(error("invalid legacy checkpoint"));
        }
        let note = path.with_extension("online");
        let (online, retain) = if std::fs::symlink_metadata(&note).is_ok() {
            match bounded(&note, 16384).and_then(|bytes| String::from_utf8(bytes).map_err(io::Error::other)) {
                Ok(text) => (Some(text), false),
                Err(_) => (None, true),
            }
        } else {
            (None, false)
        };
        // An unreadable legacy note cannot prevent opening the complete archive
        // as an unsaved file. Keep the original pair for inspection.
        Ok(Self { archive: path.to_path_buf(), online, retain, cleanup: path.to_path_buf(), generated: false })
    }
    pub fn remove(&self) -> io::Result<()> {
        fn missing_ok(result: io::Result<()>) -> io::Result<()> {
            match result {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        }
        if self.generated {
            missing_ok(std::fs::remove_dir_all(&self.cleanup))
        } else {
            missing_ok(std::fs::remove_file(&self.archive))?;
            if self.online.is_some() {
                missing_ok(std::fs::remove_file(self.archive.with_extension("online")))?;
            }
            Ok(())
        }
    }
}
pub(crate) fn publish(run: &Path, uid: u64, payload: &Payload, policy: Durability, hooks: &Hooks) -> Result<(), String> {
    let bytes = figcraft_engine::persistence::encode_recovery(&payload.snapshot).map_err(|e| e.to_string())?;
    let result = (|| -> io::Result<()> {
        let uid = child(run, &uid.to_string(), policy)?;
        let generations = child(&uid, "generations", policy)?;
        let previous = manifest(&uid).ok().map(|m| m.generation);
        let generation = unique(&generations, "g")?;
        let name = generation.file_name().and_then(|s| s.to_str()).ok_or_else(|| error("invalid generation name"))?.to_string();
        sync(&generations, policy)?;
        write_new(&generation.join("snapshot.figcraft"), &bytes, policy)?;
        hooks.at(Phase::ArchiveWritten)?;
        if let Some(note) = &payload.online {
            write_new(&generation.join("snapshot.online"), note.as_bytes(), policy)?;
        }
        hooks.at(Phase::NoteWritten)?;
        sync(&generation, policy)?;
        let m = Manifest { version: 1, generation: name.clone(), online: payload.online.is_some() };
        let tmp = uid.join(format!("current-{name}.tmp"));
        write_new(&tmp, &serde_json::to_vec(&m).map_err(io::Error::other)?, policy)?;
        if let Err(e) = std::fs::rename(&tmp, uid.join("current")) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        hooks.at(Phase::ManifestRenamed)?;
        sync(&uid, policy)?;
        // After a successful publication retain the current and preceding generation.
        for entry in std::fs::read_dir(&generations)?.flatten() {
            let entry_name = entry.file_name();
            let text = entry_name.to_str().unwrap_or_default();
            if text != name && previous.as_deref() != Some(text) && generation_name(text) && directory(&entry.path()) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        Ok(())
    })();
    result.map_err(|e| {
        log::warn!("Recovery checkpoint for document {uid} failed: {e}");
        e.to_string()
    })
}
pub(crate) fn delete(run: &Path, uid: u64) -> io::Result<()> {
    let path = run.join(uid.to_string());
    match std::fs::symlink_metadata(&path) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => std::fs::remove_dir_all(path)?,
        Ok(_) => return Err(error("invalid recovery directory during cleanup")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(())
}
pub(crate) fn candidates(run: &Path) -> Vec<CheckpointRef> {
    let Ok(entries) = std::fs::read_dir(run) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if regular(&p) && p.extension().is_some_and(|s| s == "figcraft") {
                CheckpointRef::legacy(&p).ok()
            } else if directory(&p) && e.file_name().to_str().is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())) {
                CheckpointRef::generated(&p).ok()
            } else {
                None
            }
        })
        .collect()
}
pub(crate) fn remove_empty_run(run: &Path, lock: Arc<File>) {
    if Arc::strong_count(&lock) != 1 {
        return;
    }
    let empty = std::fs::read_dir(run).is_ok_and(|mut entries| entries.all(|e| e.is_ok_and(|e| e.file_name() == LOCK)));
    if empty {
        drop(lock);
        let _ = std::fs::remove_file(run.join(LOCK));
        let _ = std::fs::remove_dir(run);
    }
}
