//! The optional position cache: platform storage identity, load, and the single background writer.
use super::{FILE, MAXIMUM_BYTES, Position, RECORD, Record, Schema, decode};
use crate::background_thread::BackgroundThread;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix::storage_path;
#[cfg(windows)]
use windows::storage_path;

pub(crate) struct Preferences {
    path: std::path::PathBuf,
    pub(crate) position: Option<Position>,
    /// A record exists that this build cannot read, such as an older format. It counts as no saved
    /// position, and the next displayed position replaces it.
    pub(crate) unreadable: bool,
    /// The cache itself is unavailable, so this invocation neither reads nor writes it.
    pub(crate) failed: bool,
    /// The record's version is one this build cannot use. It counts as no saved position and is
    /// never rewritten, so this invocation remembers no position.
    pub(crate) unusable: bool,
}
impl Preferences {
    pub(crate) fn load(runtime: &std::path::Path) -> Self {
        match storage_path(runtime) {
            Ok(path) => Self::load_from(path),
            Err(_) => Self {
                path: Default::default(),
                position: None,
                unreadable: false,
                failed: true,
                unusable: false,
            },
        }
    }
    fn load_from(path: std::path::PathBuf) -> Self {
        let mut unusable = false;
        let (position, unreadable, failed) =
            match orchbee_local_runtime::PrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES) {
                Ok(None) => (None, false, false),
                Ok(Some(bytes)) => {
                    let opening = orchbee_local_runtime::open_record(
                        orchbee_local_runtime::VersionedRecord::TuiPosition,
                        &bytes,
                        |bytes| decode(bytes).is_ok(),
                    );
                    if let Some((problem, version)) = opening.problem() {
                        orchbee_local_runtime::report_record_notice(
                            &orchbee_local_runtime::RecordNotice {
                                record: orchbee_local_runtime::VersionedRecord::TuiPosition,
                                problem,
                                version,
                                path: path.join(FILE),
                            },
                        );
                        unusable = true;
                        (None, false, false)
                    } else {
                        let bytes = match opening {
                            orchbee_local_runtime::RecordOpening::Migrated { bytes, .. } => bytes,
                            _ => bytes,
                        };
                        match decode(&bytes) {
                            Ok(position) => (Some(position), false, false),
                            Err(_) => (None, true, false),
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::FileTooLarge => {
                    (None, true, false)
                }
                Err(_) => (None, false, true),
            };
        Self {
            path,
            position,
            unreadable,
            failed,
            unusable,
        }
    }
    pub(crate) fn writer(self) -> std::io::Result<Option<Writer>> {
        if self.failed || self.unusable {
            return Ok(None);
        }
        Writer::start(self.path).map(Some)
    }
}

#[derive(Default)]
struct Queue {
    pending: Option<Position>,
    closing: bool,
}
#[derive(Default)]
struct Shared {
    queue: std::sync::Mutex<Queue>,
    wake: std::sync::Condvar,
    failed: std::sync::atomic::AtomicBool,
}
pub(crate) struct Writer {
    shared: std::sync::Arc<Shared>,
    task: BackgroundThread,
    last: Option<Position>,
}
impl Writer {
    fn start(path: std::path::PathBuf) -> std::io::Result<Self> {
        let shared = std::sync::Arc::new(Shared::default());
        let worker = std::sync::Arc::clone(&shared);
        let task = BackgroundThread::spawn("orchbee-position-writer", move || {
            loop {
                let position = {
                    let Ok(mut queue) = worker.queue.lock() else {
                        return;
                    };
                    while queue.pending.is_none() && !queue.closing {
                        let Ok(next) = worker.wake.wait(queue) else {
                            return;
                        };
                        queue = next;
                    }
                    let Some(position) = queue.pending.take() else {
                        return;
                    };
                    position
                };
                if write(&path, position).is_err() {
                    worker
                        .failed
                        .store(true, std::sync::atomic::Ordering::Release);
                }
            }
        })?;
        Ok(Self {
            shared,
            task,
            last: None,
        })
    }
    /// Closes the queue without waiting; the writer still persists the pending position.
    pub(crate) fn request_stop(&mut self) {
        if let Ok(mut queue) = self.shared.queue.lock() {
            queue.closing = true;
            self.shared.wake.notify_one();
        }
    }
    /// Process exit: an unfinished last write is abandoned at the deadline.
    pub(crate) fn stop_before(&mut self, deadline: std::time::Instant) -> std::io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }
    pub(crate) fn publish(&mut self, position: Position) {
        if self.last.as_ref() == Some(&position) {
            return;
        }
        let Ok(mut queue) = self.shared.queue.lock() else {
            return;
        };
        queue.pending = Some(position.clone());
        self.last = Some(position);
        self.shared.wake.notify_one();
    }
    pub(crate) fn take_failure(&self) -> bool {
        self.shared
            .failed
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
}
fn write(path: &std::path::Path, position: Position) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(&Record {
        schema: Schema::Position,
        version: 1,
        position,
    })
    .map_err(std::io::Error::other)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match orchbee_local_runtime::PrivateCache::acquire(path, RECORD) {
            Ok(mut cache) => return cache.replace(FILE, &bytes),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                // The OS cache lock exposes contention, not a release notification. This
                // one-second persistence job samples only while another writer owns the file.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        // The writer finishes the pending position on its own; dropping never waits for it.
        self.request_stop();
    }
}

#[cfg(all(test, windows))]
mod tests;
