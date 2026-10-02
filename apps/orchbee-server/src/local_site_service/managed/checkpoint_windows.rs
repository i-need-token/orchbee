use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, RetainedChild, WindowsRetainedSessionRecord,
    WindowsSessionStore,
};
use orchbee_server::{MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, WorkbenchCheckpoint};
use std::{
    io,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
pub(super) mod autosave;

pub(super) fn load(
    site: &ModeCheckedLocalSiteRendezvous,
) -> io::Result<(
    WindowsSessionStore,
    Option<WorkbenchCheckpoint>,
    Option<WindowsRetainedSessionRecord>,
    bool,
)> {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("LOCALAPPDATA is unavailable"))?;
    let store = WindowsSessionStore::open(&local, site, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)?;
    let loaded = store.read_validated(|bytes| WorkbenchCheckpoint::parse(bytes).ok())?;
    let suppress_initial_empty = loaded.checkpoint.is_none()
        && loaded.notice == Some(orchbee_local_site_runtime::WindowsCheckpointRestoreNotice::Empty);
    let checkpoint = loaded.checkpoint;
    let record = WindowsRetainedSessionRecord::retain(&local, site)?;
    if let Some(record) = &record {
        store.publish_restore_notice(record, loaded.notice)?;
    } else if loaded.notice.is_some() {
        // Unmanaged development scopes have no durable generation to authenticate a warning.
        return Err(io::Error::other(
            "checkpoint restore needs a managed Site generation",
        ));
    }
    Ok((store, checkpoint, record, suppress_initial_empty))
}

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
