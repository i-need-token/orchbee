//! The optional position cache: platform storage identity, load, and the single background writer.
//! The TUI is the record's only writer: a load keeps an earlier record migrated in memory, and the
//! writer backs it up and persists the migration under its lock before its first replacement.
use super::{MAXIMUM_BYTES, Position, Record, Schema, decode, decode_record};
use crate::background_thread::BackgroundThread;
use orchbee_local_runtime::{OpenedRecord, VersionedFile};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix::storage;
#[cfg(windows)]
use windows::storage;

pub(crate) struct Preferences {
    record: Option<VersionedFile>,
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
        match storage(runtime) {
            Ok(record) => Self::load_from(record),
            Err(_) => Self {
                record: None,
                position: None,
                unreadable: false,
                failed: true,
                unusable: false,
            },
        }
    }
    fn load_from(record: VersionedFile) -> Self {
        let mut unusable = false;
        let (position, unreadable, failed) = match record.read(MAXIMUM_BYTES, admits) {
            Ok(OpenedRecord::Absent) => (None, false, false),
            Ok(OpenedRecord::Unusable(_)) => {
                unusable = true;
                (None, false, false)
            }
            Ok(OpenedRecord::Readable(bytes)) => match decode(&bytes) {
                Ok(position) => (Some(position), false, false),
                Err(_) => (None, true, false),
            },
            Err(error) if error.kind() == std::io::ErrorKind::FileTooLarge => (None, true, false),
            Err(_) => (None, false, true),
        };
        Self {
            record: Some(record),
            position,
            unreadable,
            failed,
            unusable,
        }
    }
    pub(crate) fn writer(self) -> std::io::Result<Option<Writer>> {
        match self.record {
            Some(record) if !self.failed && !self.unusable => Writer::start(record).map(Some),
            _ => Ok(None),
        }
    }
}
fn admits(bytes: &[u8]) -> bool {
    decode(bytes).is_ok()
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
/// What one write did with the record.
enum Written {
    Replaced,
    /// The record's version is one this build cannot use, saved by a newer TUI since the load or
    /// an earlier one whose migration could not be saved, so it stays as it is and this writer
    /// writes no more.
    Kept,
}
pub(crate) struct Writer {
    shared: std::sync::Arc<Shared>,
    task: BackgroundThread,
    last: Option<Position>,
}
impl Writer {
    fn start(record: VersionedFile) -> std::io::Result<Self> {
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
                match write(&record, position) {
                    Ok(Written::Replaced) => {}
                    Ok(Written::Kept) => return,
                    Err(_) => worker
                        .failed
                        .store(true, std::sync::atomic::Ordering::Release),
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
/// Opens the record under its writer lock, which backs up and persists an earlier version first,
/// and replaces it unless its version is one this build cannot use. The replacement keeps the
/// current record's `extensions`.
fn write(record: &VersionedFile, position: Position) -> std::io::Result<Written> {
    let encode = |extensions, position| {
        serde_json::to_vec(&Record {
            schema: Schema::Position,
            version: 1,
            extensions,
            position,
        })
        .map_err(std::io::Error::other)
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match record.acquire(MAXIMUM_BYTES, admits) {
            Ok((_, Ok(OpenedRecord::Unusable(_)))) => return Ok(Written::Kept),
            // A malformed record counts as unsaved and is replaced without extensions.
            Ok((mut writer, Ok(opened))) => {
                let extensions = match opened {
                    OpenedRecord::Readable(bytes) => decode_record(&bytes)
                        .map(|current| current.extensions)
                        .unwrap_or_default(),
                    OpenedRecord::Absent | OpenedRecord::Unusable(_) => serde_json::Map::new(),
                };
                let bytes = encode(extensions, position)?;
                return writer.replace(&bytes).map(|()| Written::Replaced);
            }
            // A record too large to read counts as unsaved, like a malformed one, and is replaced.
            Ok((mut writer, Err(error))) if error.kind() == std::io::ErrorKind::FileTooLarge => {
                let bytes = encode(serde_json::Map::new(), position)?;
                return writer.replace(&bytes).map(|()| Written::Replaced);
            }
            Ok((_, Err(error))) => return Err(error),
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
