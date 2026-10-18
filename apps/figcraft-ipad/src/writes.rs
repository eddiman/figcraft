//! App-owned destination coordinator: one writer per destination, ordered admission.
//! Holds no engine state; the order type is the engine's checked `WriteOrder` on device.
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};

static STAGING: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum WriteError {
    /// A newer order was admitted for this destination; nothing was written.
    Superseded,
    /// The path is outside the owned root, aliases a symlink, or has an invalid leaf.
    InvalidDestination(&'static str),
    Io(io::Error),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Superseded => f.write_str("a newer save was already accepted"),
            Self::InvalidDestination(reason) => write!(f, "cannot save there: {reason}"),
            Self::Io(error) => write!(f, "save failed: {error}"),
        }
    }
}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Canonical existing parent plus a validated leaf name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DestinationKey {
    parent: PathBuf,
    leaf: String,
}

impl DestinationKey {
    pub fn path(&self) -> PathBuf {
        self.parent.join(&self.leaf)
    }
}

fn valid_leaf(leaf: &str) -> bool {
    !leaf.is_empty()
        && leaf.len() <= 255
        && leaf != "."
        && leaf != ".."
        && !leaf.starts_with('.')
        && !leaf.contains(['/', '\\', ':'])
        && !leaf.chars().any(char::is_control)
}

struct Destination<O> {
    highest: Mutex<Option<O>>,
    writer: Mutex<()>,
}

pub struct WriteCoordinator<O> {
    root: PathBuf,
    destinations: Mutex<HashMap<DestinationKey, Arc<Destination<O>>>>,
}

/// Proof that an order was admitted. Running it re-checks currentness under the writer lock.
pub struct WriteTicket<O> {
    key: DestinationKey,
    order: O,
    destination: Arc<Destination<O>>,
}

fn lock<T>(mutex: &Mutex<T>) -> io::Result<MutexGuard<'_, T>> {
    mutex.lock().map_err(|_| io::Error::other("save coordinator lock is poisoned"))
}

fn validate_target(path: &Path) -> Result<(), WriteError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(WriteError::InvalidDestination("destination is not a regular file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(WriteError::Io(error)),
    }
}

fn validate_parent(parent: &Path) -> Result<(), WriteError> {
    let metadata = std::fs::symlink_metadata(parent)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || std::fs::canonicalize(parent)? != parent {
        return Err(WriteError::InvalidDestination("owned document directory was replaced"));
    }
    Ok(())
}

impl<O: Ord + Copy> WriteCoordinator<O> {
    pub fn new(owned_root: &Path) -> io::Result<Self> {
        let root = std::fs::canonicalize(owned_root)?;
        if !root.is_dir() {
            return Err(io::Error::other("owned document root is not a directory"));
        }
        Ok(Self { root, destinations: Mutex::new(HashMap::new()) })
    }

    /// Resolve aliases of the owned parent to one key. The leaf must be absent or a regular file.
    pub fn key(&self, path: &Path) -> Result<DestinationKey, WriteError> {
        let leaf = path.file_name().and_then(|name| name.to_str()).ok_or(WriteError::InvalidDestination("leaf is not valid UTF-8"))?;
        if !valid_leaf(leaf) {
            return Err(WriteError::InvalidDestination("invalid file name"));
        }
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or(WriteError::InvalidDestination("missing parent"))?;
        let parent = std::fs::canonicalize(parent)?;
        if !parent.starts_with(&self.root) {
            return Err(WriteError::InvalidDestination("outside the owned directory"));
        }
        validate_target(&parent.join(leaf))?;
        Ok(DestinationKey { parent, leaf: leaf.to_owned() })
    }

    /// Record the order even if the write later fails, so a delayed older request is refused.
    pub fn admit(&self, key: DestinationKey, order: O) -> Result<WriteTicket<O>, WriteError> {
        if !key.parent.starts_with(&self.root) {
            return Err(WriteError::InvalidDestination("outside the owned directory"));
        }
        let destination = lock(&self.destinations)?
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Destination { highest: Mutex::new(None), writer: Mutex::new(()) }))
            .clone();
        {
            let mut highest = lock(&destination.highest)?;
            if highest.is_some_and(|newest| newest >= order) {
                return Err(WriteError::Superseded);
            }
            *highest = Some(order);
        }
        Ok(WriteTicket { key, order, destination })
    }
}

impl<O: Ord + Copy> WriteTicket<O> {
    pub fn order(&self) -> O {
        self.order
    }

    /// Write, sync the file, rename, then sync the parent. At most one writer runs per destination.
    pub fn run(self, bytes: &[u8]) -> Result<(), WriteError> {
        let _writer = lock(&self.destination.writer)?;
        if lock(&self.destination.highest)?.is_some_and(|newest| newest > self.order) {
            return Err(WriteError::Superseded);
        }
        let target = self.key.path();
        validate_parent(&self.key.parent)?;
        validate_target(&target)?;
        let id = STAGING
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| io::Error::other("save staging IDs exhausted"))?;
        let staging = self.key.parent.join(format!(".figcraft-save.{}.{id}.tmp", std::process::id()));
        write_staging(&staging, bytes)?;
        let result = validate_parent(&self.key.parent)
            .and_then(|()| validate_target(&target))
            .and_then(|()| std::fs::rename(&staging, &target).map_err(WriteError::Io))
            .and_then(|()| File::open(&self.key.parent)?.sync_all().map_err(WriteError::Io));
        if result.is_err() {
            remove_staging(&staging);
        }
        result
    }
}

fn write_staging(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    if result.is_err() {
        remove_staging(path);
    }
    result
}

fn remove_staging(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        log::warn!("Could not remove save staging file {}: {error}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Root(PathBuf);
    impl std::ops::Deref for Root {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }
    impl AsRef<Path> for Root {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                eprintln!("Test directory cleanup failed: {error}");
            }
        }
    }
    fn root(name: &str) -> Root {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("figcraft-writes-{name}-{}-{id}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Root(path)
    }

    #[test]
    #[cfg(unix)]
    fn ipad_write_rechecks_owned_parent_after_admission() {
        let root = root("replaced-parent");
        let owned = root.join("owned");
        let foreign = root.join("foreign");
        std::fs::create_dir(&owned).unwrap();
        std::fs::create_dir(&foreign).unwrap();
        let target = owned.join("document.figcraft");
        std::fs::write(&target, b"previous owned bytes").unwrap();
        std::fs::write(foreign.join("document.figcraft"), b"foreign original").unwrap();
        let coordinator = WriteCoordinator::new(&root).unwrap();
        let ticket = coordinator.admit(coordinator.key(&target).unwrap(), 1u64).unwrap();
        let retained = root.join("retained");
        std::fs::rename(&owned, &retained).unwrap();
        std::os::unix::fs::symlink(&foreign, &owned).unwrap();
        assert!(ticket.run(b"must not escape").is_err());
        assert_eq!(std::fs::read(foreign.join("document.figcraft")).unwrap(), b"foreign original");
        assert_eq!(std::fs::read(retained.join("document.figcraft")).unwrap(), b"previous owned bytes");
    }

    #[test]
    fn ipad_write_admits_newer_before_delayed_older_picker_reply() {
        let dir = root("picker");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let path = dir.join("doc.craft");
        let newer = coordinator.admit(coordinator.key(&path).unwrap(), 2).unwrap();
        assert!(matches!(coordinator.admit(coordinator.key(&path).unwrap(), 1), Err(WriteError::Superseded)));
        newer.run(b"newer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"newer");
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn ipad_write_two_documents_share_one_destination_and_aliases() {
        let dir = root("alias");
        std::fs::create_dir(dir.join("sub")).unwrap();
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let direct = coordinator.key(&dir.join("sub/doc.craft")).unwrap();
        let alias = coordinator.key(&dir.join("sub/../sub/doc.craft")).unwrap();
        assert_eq!(direct, alias);
        let first = coordinator.admit(direct, 5).unwrap();
        assert!(matches!(coordinator.admit(alias, 4), Err(WriteError::Superseded)));
        first.run(b"from-b").unwrap();
        assert_eq!(std::fs::read(dir.join("sub/doc.craft")).unwrap(), b"from-b");
    }

    #[test]
    fn ipad_write_concurrent_tickets_leave_the_highest_order_on_disk() {
        let dir = root("concurrent");
        let coordinator = Arc::new(WriteCoordinator::<u64>::new(&dir).unwrap());
        let path = dir.join("doc.craft");
        let tickets: Vec<_> = (1..=16u64).map(|n| coordinator.admit(coordinator.key(&path).unwrap(), n).unwrap()).collect();
        let handles: Vec<_> = tickets
            .into_iter()
            .rev()
            .map(|ticket| {
                let bytes = ticket.order().to_string().into_bytes();
                std::thread::spawn(move || ticket.run(&bytes).is_ok())
            })
            .collect();
        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(results[0], "the newest ticket always writes");
        assert_eq!(std::fs::read(&path).unwrap(), b"16");
    }

    #[test]
    fn ipad_write_running_older_write_publishes_before_newer_write() {
        let dir = root("running");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let path = dir.join("doc.craft");
        coordinator.admit(coordinator.key(&path).unwrap(), 1).unwrap().run(b"older").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"older");
        coordinator.admit(coordinator.key(&path).unwrap(), 2).unwrap().run(b"newer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"newer");
    }

    #[test]
    fn ipad_write_older_admitted_ticket_cannot_run_after_newer_started() {
        let dir = root("recheck");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let path = dir.join("doc.craft");
        let older = coordinator.admit(coordinator.key(&path).unwrap(), 1).unwrap();
        let newer = coordinator.admit(coordinator.key(&path).unwrap(), 2).unwrap();
        newer.run(b"newer").unwrap();
        assert!(matches!(older.run(b"older"), Err(WriteError::Superseded)));
        assert_eq!(std::fs::read(&path).unwrap(), b"newer");
    }

    #[test]
    fn ipad_write_failed_latest_admission_still_blocks_older() {
        let dir = root("failed");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let path = dir.join("doc.craft");
        let key = coordinator.key(&path).unwrap();
        let failing = coordinator.admit(key.clone(), 3).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(matches!(failing.run(b"x"), Err(WriteError::InvalidDestination(_))));
        assert!(path.is_dir());
        assert!(matches!(coordinator.admit(key, 2), Err(WriteError::Superseded)));
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn ipad_local_save_disk_failure_preserves_previous_bytes() {
        let dir = root("disk");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let path = dir.join("doc.craft");
        coordinator.admit(coordinator.key(&path).unwrap(), 1).unwrap().run(b"good").unwrap();
        assert!(write_staging(&path, b"lost").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"good");
    }

    #[test]
    fn ipad_write_rejects_non_file_and_foreign_destinations() {
        let dir = root("non-file");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        std::fs::create_dir(dir.join("folder")).unwrap();
        assert!(matches!(coordinator.key(&dir.join("folder")), Err(WriteError::InvalidDestination(_))));
        std::fs::write(dir.join("file"), b"existing").unwrap();
        assert!(WriteCoordinator::<u64>::new(&dir.join("file")).is_err());
        let other = root("foreign");
        let foreign = WriteCoordinator::<u64>::new(&other).unwrap().key(&other.join("doc.craft")).unwrap();
        assert!(matches!(coordinator.admit(foreign, 1), Err(WriteError::InvalidDestination(_))));
    }

    #[test]
    fn ipad_write_accepts_maximum_length_leaf_without_extending_staging_name() {
        let dir = root("long-name");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        let path = dir.join("a".repeat(255));
        coordinator.admit(coordinator.key(&path).unwrap(), 1).unwrap().run(b"saved").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"saved");
    }

    #[test]
    fn ipad_write_rejects_escape_symlink_and_bad_leaf() {
        let dir = root("escape");
        let outside = root("outside");
        let coordinator = WriteCoordinator::<u64>::new(&dir).unwrap();
        assert!(matches!(coordinator.key(&outside.join("doc.craft")), Err(WriteError::InvalidDestination(_))));
        assert!(matches!(coordinator.key(&dir.join("../doc.craft")), Err(WriteError::InvalidDestination(_))));
        for leaf in [".hidden", "a:b", ""] {
            assert!(coordinator.key(&dir.join(leaf)).is_err());
        }
        std::fs::write(outside.join("target"), b"x").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("target"), dir.join("link.craft")).unwrap();
            assert!(matches!(coordinator.key(&dir.join("link.craft")), Err(WriteError::InvalidDestination(_))));
        }
    }
}
