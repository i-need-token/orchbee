use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, RetainedChild, WindowsSessionStore,
};
use orchbee_server::{MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, WorkbenchCheckpoint};
use std::{
    io,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

pub(super) fn load(
    site: &ModeCheckedLocalSiteRendezvous,
) -> io::Result<(WindowsSessionStore, Option<WorkbenchCheckpoint>)> {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("LOCALAPPDATA is unavailable"))?;
    let store = WindowsSessionStore::open(&local, site, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)?;
    let checkpoint = store
        .read()?
        .map(|bytes| WorkbenchCheckpoint::parse(&bytes))
        .transpose()
        .map_err(io::Error::other)?;
    Ok((store, checkpoint))
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
