//! Loads a bounded Windows checkpoint and coordinates cleanup when startup aborts before
//! launch, retaining exact child and generation ownership. It does not interpret Workbench
//! topology or claim that a pathname or pipe disappearance proves process exit.

use orchbee_local_runtime::{
    DataRoot, ModeCheckedLocalSessionRendezvous, RetainedChild, RetainedSessionRecord, SessionStore,
};
use orchbee_server::{MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, WorkbenchCheckpoint};
use std::{
    io, thread,
    time::{Duration, Instant},
};
pub(super) mod autosave;

pub(super) fn load(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<(
    SessionStore,
    Option<WorkbenchCheckpoint>,
    RetainedSessionRecord,
    bool,
)> {
    let root = DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::other("the data root is unavailable"))?;
    load_from(&root, session)
}

fn load_from(
    root: &DataRoot,
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<(
    SessionStore,
    Option<WorkbenchCheckpoint>,
    RetainedSessionRecord,
    bool,
)> {
    // Direct Server entry must also refuse unsupported catalog versions before store effects.
    let record = RetainedSessionRecord::retain(root, session)?;
    let store = SessionStore::open(root, session, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)?;
    let loaded = store.read_validated(|bytes| WorkbenchCheckpoint::parse(bytes).ok())?;
    let suppress_initial_empty = loaded.checkpoint.is_none()
        && loaded.notice == Some(orchbee_local_runtime::CheckpointRestoreNotice::Empty);
    let checkpoint = loaded.checkpoint;
    store.publish_restore_notice(&record, loaded.notice)?;
    Ok((store, checkpoint, record, suppress_initial_empty))
}

#[cfg(test)]
mod tests;

/// No launch has been sent; this Worker cannot own a Host or terminal.
pub(super) fn abort_before_launch(
    mut worker: RetainedChild,
    carrier: Option<thread::JoinHandle<()>>,
    startup: io::Error,
) -> io::Error {
    let deadline = Instant::now() + Duration::from_secs(5);
    let cleanup = (|| -> io::Result<()> {
        worker.terminate_before(deadline)?;
        if let Some(carrier) = carrier {
            while !carrier.is_finished() {
                if Instant::now() >= deadline {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                thread::sleep(Duration::from_millis(10));
            }
            carrier
                .join()
                .map_err(|_| io::Error::other("Worker carrier panicked"))?;
        }
        Ok(())
    })();
    match cleanup {
        Ok(()) => startup,
        Err(cleanup) => {
            io::Error::other(format!("{startup}; startup cleanup unconfirmed: {cleanup}"))
        }
    }
}
