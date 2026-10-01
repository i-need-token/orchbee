//! Bounded Site-owned file roots and temporary read/preparation leases; canonical topology owns only directory identities.
use orchbee_local_site_runtime::WindowsPrivateFrameDirectory;
use orchbee_workbench_protocol::{
    PaneGraphicsFailure as Failure, SemanticPane, SiteId, WorkbenchRequestId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAXIMUM_ROOTS: usize = 128;
#[derive(Default)]
struct Roots {
    directories: BTreeMap<String, Arc<WindowsPrivateFrameDirectory>>,
    preparing_or_reaping: usize,
}

/// Owns active, failed and retired roots; all filesystem operations run outside its registry lock.
pub(in crate::local_terminal_service) struct GraphicsFiles {
    runtime: PathBuf,
    roots: Mutex<Roots>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
/// Keeps one exact root alive through preparation; release wakes the existing maintenance owner.
pub(in crate::local_terminal_service) struct FileLease {
    root: Option<Arc<WindowsPrivateFrameDirectory>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl FileLease {
    /// Directory identity advertised only after successful private materialization.
    pub(in crate::local_terminal_service) fn directory(&self) -> Result<&Path, Failure> {
        self.root
            .as_ref()
            .map(|root| root.path())
            .ok_or(Failure::Unavailable)
    }
    /// Returns a complete immutable file copy, never a pathname dependency in installed pixels.
    pub(in crate::local_terminal_service) fn read(
        &self,
        path: &Path,
        bytes: usize,
    ) -> Result<Vec<u8>, Failure> {
        self.root
            .as_ref()
            .ok_or(Failure::InvalidFile)?
            .read_frame(path, bytes)
            .map_err(|error| match error.kind() {
                io::ErrorKind::FileTooLarge => Failure::ImageTooLarge,
                io::ErrorKind::OutOfMemory => Failure::MemoryLimit,
                _ => Failure::InvalidFile,
            })
    }
}
impl Drop for FileLease {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            let weak = Arc::downgrade(&root);
            drop(root);
            if weak.strong_count() == 1 {
                (self.wake)();
            }
        }
    }
}
impl GraphicsFiles {
    /// Lazy managed-runtime configuration; no directory exists until an admitted producer opens.
    pub(in crate::local_terminal_service) fn new(
        runtime: &Path,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            runtime: runtime.to_owned(),
            roots: Mutex::default(),
            wake,
        }
    }
    /// Allocates an owned private root with an explicit recovery note; failure remains registered for cleanup.
    pub(in crate::local_terminal_service) fn create(
        &self,
        site: &SiteId,
        source: &SemanticPane,
        connection: NonZeroU64,
        stream: &WorkbenchRequestId,
    ) -> Result<FileLease, Failure> {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| Failure::Unavailable)?;
        if nonce.iter().all(|b| *b == 0) {
            return Err(Failure::Unavailable);
        }
        let nonce = format!("{:032x}", u128::from_be_bytes(nonce));
        let note = serde_json::to_vec(&serde_json::json!({
            "version": 1, "siteId": site, "source": source, "connection": connection.get(),
            "streamId": stream, "processId": std::process::id(), "nonce": nonce,
            "owner": "site-graphics-producer", "release": "producer retired and last reader settled"
        }))
        .map_err(|_| Failure::Unavailable)?;
        {
            let mut roots = self.roots.lock().map_err(|_| Failure::Unavailable)?;
            if roots.directories.len() + roots.preparing_or_reaping >= MAXIMUM_ROOTS {
                return Err(Failure::Busy);
            }
            roots.preparing_or_reaping += 1;
        }
        let reserved = WindowsPrivateFrameDirectory::reserve(&self.runtime, &nonce);
        let mut root = match reserved {
            Ok(root) => root,
            Err(_) => {
                self.roots
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .preparing_or_reaping -= 1;
                return Err(Failure::Unavailable);
            }
        };
        let created = root.create(&note);
        let directory = root.path().to_string_lossy().into_owned(); // reserve has admitted UTF-8 paths.
        let root = Arc::new(root);
        {
            let mut roots = self
                .roots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            roots.preparing_or_reaping -= 1;
            if roots.directories.contains_key(&directory) {
                // A nonce collision cannot have created the existing root, and never adopts it.
                return Err(Failure::Unavailable);
            }
            roots.directories.insert(directory, Arc::clone(&root));
        }
        (self.wake)();
        created.map_err(|_| Failure::Unavailable)?;
        Ok(FileLease {
            root: Some(root),
            wake: Arc::clone(&self.wake),
        })
    }
    /// Pins an already admitted producer root; arbitrary caller paths never create or select one.
    pub(in crate::local_terminal_service) fn lease(
        &self,
        directory: &str,
    ) -> Result<FileLease, Failure> {
        let root = self
            .roots
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .directories
            .get(directory)
            .cloned()
            .ok_or(Failure::InvalidFile)?;
        Ok(FileLease {
            root: Some(root),
            wake: Arc::clone(&self.wake),
        })
    }
    /// Reaps retired roots only when no preparing or reading lease remains; failed roots retain ownership.
    pub(in crate::local_terminal_service) fn reap(
        &self,
        active: &BTreeSet<String>,
    ) -> io::Result<bool> {
        let retired = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| io::Error::other("graphics root owner poisoned"))?;
            let keys = roots
                .directories
                .iter()
                .filter(|(key, root)| !active.contains(*key) && Arc::strong_count(root) == 1)
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            let retired = keys
                .into_iter()
                .filter_map(|key| roots.directories.remove(&key).map(|root| (key, root)))
                .collect::<Vec<_>>();
            roots.preparing_or_reaping += retired.len();
            retired
        };
        let mut failure = None;
        for (key, root) in retired {
            let (retained, error) = match Arc::try_unwrap(root) {
                Ok(mut root) => match root.reap() {
                    Ok(()) => (None, None),
                    Err(error) => (Some(Arc::new(root)), Some(error)),
                },
                Err(root) => (Some(root), Some(io::ErrorKind::WouldBlock.into())),
            };
            let mut roots = self
                .roots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            roots.preparing_or_reaping -= 1;
            if let Some(root) = retained {
                roots.directories.insert(key, root);
            }
            if error.is_some() {
                failure = error;
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let roots = self
            .roots
            .lock()
            .map_err(|_| io::Error::other("graphics root owner poisoned"))?;
        Ok(!roots.directories.is_empty() || roots.preparing_or_reaping != 0)
    }
}
