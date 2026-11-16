//! Loads a bounded Session checkpoint from the data root, holds the store and saver its saves
//! publish through, and coordinates cleanup when a Windows startup aborts before launch, retaining
//! exact child and generation ownership. It does not interpret Workbench topology or claim that a
//! pathname or pipe disappearance proves process exit.

use crate::local_terminal_service::LocalTerminalService;
#[cfg(windows)]
use orchbee_local_runtime::RetainedChild;
use orchbee_local_runtime::{
    DataRoot, ModeCheckedLocalSessionRendezvous, RetainedSessionRecord, SessionStore,
};
use orchbee_server::{MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, WorkbenchCheckpoint};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Instant,
};
#[cfg(windows)]
use std::{thread, time::Duration};
pub(super) mod autosave;

/// Whether this platform's managed Session keeps a checkpoint: it restores the last one when it
/// starts, saves while it runs and publishes a final one when it stops. A Linux Session keeps none
/// yet and starts empty. A policy constant, so every branch compiles everywhere.
const KEPT: bool = cfg!(any(windows, target_os = "macos"));

/// The store a Session's saves publish to and, once the Session runs, the saver that publishes
/// while it runs.
pub(super) struct Kept {
    store: Arc<Mutex<SessionStore>>,
    saver: Option<autosave::Saver>,
    /// The loaded fallback was the visible empty one, which stays unsaved until it changes.
    suppress_initial_empty: bool,
}

impl Kept {
    /// Starts the one background writer; the Session's terminals and jobs must already run.
    pub(super) fn start_saver(
        &mut self,
        terminals: &Arc<LocalTerminalService>,
        worker: orchbee_workbench_protocol::WorkerId,
    ) -> io::Result<()> {
        self.saver = Some(autosave::Saver::start(
            Arc::clone(&self.store),
            Arc::clone(terminals),
            worker,
            self.suppress_initial_empty,
        )?);
        Ok(())
    }

    /// Stops the background writer; a write in progress returns first.
    pub(super) fn stop_saver_before(&mut self, deadline: Instant) -> io::Result<()> {
        match &mut self.saver {
            Some(saver) => saver.stop_before(deadline),
            None => Ok(()),
        }
    }

    /// Publishes the final checkpoint, even when it matches the suppressed empty fallback.
    pub(super) fn publish_final(
        &self,
        terminals: &LocalTerminalService,
        bytes: &[u8],
    ) -> io::Result<()> {
        autosave::publish_final(&self.store, bytes)?;
        terminals.after_checkpoint_save(bytes);
        Ok(())
    }

    /// The store kept terminal output shares with the checkpoint.
    #[cfg(windows)]
    pub(super) fn output_store(&self) -> Arc<dyn crate::terminal_output::TerminalOutputStore> {
        Arc::clone(&self.store) as _
    }
}

/// The Session's retained record and, where this platform keeps a checkpoint, its store and the
/// checkpoint to restore, if any. Loading publishes the restore notice the activating Client
/// reads.
pub(super) fn load(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<(
    RetainedSessionRecord,
    Option<Kept>,
    Option<WorkbenchCheckpoint>,
)> {
    let root = DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::other("the data root is unavailable"))?;
    if !KEPT {
        return Ok((RetainedSessionRecord::retain(&root, session)?, None, None));
    }
    let (store, checkpoint, record, suppress_initial_empty) = load_from(&root, session)?;
    let kept = Kept {
        store: Arc::new(Mutex::new(store)),
        saver: None,
        suppress_initial_empty,
    };
    Ok((record, Some(kept), checkpoint))
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
#[cfg(windows)]
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
