//! Loads a bounded Session checkpoint from the data root, holds the store and saver its saves
//! publish through, and coordinates cleanup when a Windows startup aborts before launch, retaining
//! exact child and generation ownership. It does not interpret Workbench topology or claim that a
//! pathname or pipe disappearance proves process exit.

use crate::local_terminal_service::LocalTerminalService;
#[cfg(any(windows, target_os = "macos"))]
use orchbee_local_runtime::RetainedChild;
use orchbee_local_runtime::{
    DataRoot, LogLevel, LogValue, ModeCheckedLocalSessionRendezvous, RetainedSessionRecord,
    SessionStore,
};
use orchbee_server::{MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, WorkbenchCheckpoint};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Instant,
};
#[cfg(any(windows, target_os = "macos"))]
use std::{thread, time::Duration};
pub(super) mod autosave;
mod bindings;
#[cfg(windows)]
mod moves;

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
    /// What keeps the binding record, where this platform keeps one; the saver and the final
    /// capture write through it, one at a time.
    bindings: Option<Arc<Mutex<bindings::Writer>>>,
    /// The move turns of a Worker that adopted its Hosts, which record each rebinding through the
    /// binding record before the next turn.
    #[cfg(windows)]
    turns: Option<moves::Turns>,
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
            self.bindings.clone(),
        )?);
        Ok(())
    }

    /// Starts the move turns of the Worker `handle` names, which adopted its Hosts, in place of any
    /// earlier generation's.
    #[cfg(windows)]
    pub(super) fn start_turns(
        &mut self,
        terminals: &Arc<LocalTerminalService>,
        handle: &crate::worker_obcn_carrier::WorkerRequestHandle,
        worker: orchbee_workbench_protocol::WorkerId,
    ) {
        if let Some(writer) = &self.bindings {
            self.turns = moves::Turns::start(
                handle.clone(),
                Arc::clone(terminals),
                Arc::clone(writer),
                worker,
            );
        }
    }

    /// Whether move turns were started for this Session's Worker, even if they ended since.
    #[cfg(windows)]
    pub(super) const fn turns_started(&self) -> bool {
        self.turns.is_some()
    }

    /// Stops the move turns and the background writer; a write in progress returns first.
    pub(super) fn stop_saver_before(&mut self, deadline: Instant) -> io::Result<()> {
        #[cfg(windows)]
        if let Some(turns) = &self.turns {
            turns.stop();
        }
        match &mut self.saver {
            Some(saver) => saver.stop_before(deadline),
            None => Ok(()),
        }
    }

    /// Publishes the final checkpoint, even when it matches the suppressed empty fallback, and
    /// then the binding record of the same moment. A binding record that cannot be written is
    /// reported and removed; it never fails the capture.
    pub(super) fn publish_final(
        &self,
        terminals: &LocalTerminalService,
        worker: &orchbee_workbench_protocol::WorkerId,
        bytes: &[u8],
    ) -> io::Result<()> {
        autosave::publish_final(&self.store, bytes)?;
        terminals.after_checkpoint_save(bytes);
        if let Some(Ok(mut bindings)) = self.bindings.as_ref().map(|bindings| bindings.lock()) {
            bindings.write_final(terminals, worker);
        }
        Ok(())
    }

    /// Publishes a restart's exact capture: the checkpoint, then the binding record of the same
    /// moment, which the successor adopts the Host from, so a binding record that cannot be written
    /// fails it.
    #[cfg(windows)]
    pub(super) fn publish_exact(
        &self,
        terminals: &LocalTerminalService,
        worker: &orchbee_workbench_protocol::WorkerId,
        bytes: &[u8],
    ) -> io::Result<()> {
        autosave::publish_final(&self.store, bytes)?;
        terminals.after_checkpoint_save(bytes);
        match self.bindings.as_ref().map(|bindings| bindings.lock()) {
            Some(Ok(mut bindings)) => bindings.write_exact(terminals, worker),
            _ => Err(io::Error::other("no binding record is kept")),
        }
    }

    /// Writes the binding record a restart's successor describes once its Worker adopted the Host,
    /// before the commit; a write that is deferred or fails fails the restart.
    #[cfg(windows)]
    pub(super) fn write_bindings_exact(
        &self,
        terminals: &LocalTerminalService,
        worker: &orchbee_workbench_protocol::WorkerId,
    ) -> io::Result<()> {
        match self.bindings.as_ref().map(|bindings| bindings.lock()) {
            Some(Ok(mut bindings)) => bindings.write_exact(terminals, worker),
            _ => Err(io::Error::other("no binding record is kept")),
        }
    }

    /// Moves the binding writer to `session`, the generation a restart's successor committed.
    #[cfg(windows)]
    pub(super) fn committed(&self, session: RetainedSessionRecord) -> io::Result<()> {
        match self.bindings.as_ref().map(|bindings| bindings.lock()) {
            Some(Ok(mut bindings)) => {
                bindings.renew(session);
                Ok(())
            }
            _ => Err(io::Error::other("no binding record is kept")),
        }
    }

    /// Lets the successor of a restart open the store; every write refuses until
    /// [`Self::retake_store`].
    #[cfg(windows)]
    pub(super) fn release_store(&self) -> io::Result<()> {
        self.store
            .lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))?
            .release();
        Ok(())
    }

    /// Holds the store again after a restart that did not commit, waiting until `deadline` for the
    /// ended successor's hold on it to go. The binding record is written again at the next change,
    /// since the successor may have replaced it.
    #[cfg(windows)]
    pub(super) fn retake_store(&self, deadline: Instant) -> io::Result<()> {
        loop {
            let retaken = self
                .store
                .lock()
                .map_err(|_| io::Error::other("checkpoint store failed"))?
                .retake();
            match retaken {
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(50));
                }
                retaken => break retaken?,
            }
        }
        if let Some(Ok(mut bindings)) = self.bindings.as_ref().map(|bindings| bindings.lock()) {
            bindings.forget_written();
        }
        Ok(())
    }

    /// Removes the binding record once the Host has ended, so nothing is left to adopt; a failure
    /// is reported only.
    pub(super) fn remove_bindings(&self) {
        if let Some(Ok(mut bindings)) = self.bindings.as_ref().map(|bindings| bindings.lock()) {
            bindings.remove();
        }
    }

    /// The binding record this generation adopts its predecessor's Host from, read through the
    /// store; the record this generation writes keeps its `extensions`. An absent or unusable
    /// record fails.
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) fn adopted_record(&self) -> io::Result<crate::binding_record::BindingRecord> {
        let bytes = self
            .store
            .lock()
            .map_err(|_| io::Error::other("checkpoint store failed"))?
            .read_bindings()?
            .ok_or_else(|| io::Error::other("no binding record names the kept Terminal Host"))?;
        let record = crate::binding_record::decode(&bytes)
            .ok_or_else(|| io::Error::other("the binding record is unusable"))?;
        if let Some(Ok(mut bindings)) = self.bindings.as_ref().map(|bindings| bindings.lock()) {
            bindings.keep_extensions(record.extensions.clone());
        }
        Ok(record)
    }

    /// The store kept terminal output shares with the checkpoint.
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
    let store = Arc::new(Mutex::new(store));
    let bindings = if bindings::RECORDED {
        let session = RetainedSessionRecord::retain(&root, session)?;
        Some(Arc::new(Mutex::new(bindings::Writer::new(
            Arc::clone(&store),
            session,
        ))))
    } else {
        None
    };
    let kept = Kept {
        store,
        saver: None,
        suppress_initial_empty,
        bindings,
        #[cfg(windows)]
        turns: None,
    };
    Ok((record, Some(kept), checkpoint))
}

/// The store a restart's successor takes once the Server it replaces released it, and the exact
/// capture that Server published, which must be the newest checkpoint and readable by this build;
/// the binding writer reads the replaced generation's record until [`Kept::committed`]. Nothing is
/// restored after a fallback and no restore notice is published: no activation waits for one.
#[cfg(windows)]
pub(super) fn take(
    root: &DataRoot,
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<(RetainedSessionRecord, Kept, WorkbenchCheckpoint)> {
    let record = RetainedSessionRecord::retain(root, session)?;
    let mut store = SessionStore::open(root, session, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)?;
    let loaded = store.read_validated(|bytes| WorkbenchCheckpoint::parse(bytes).ok())?;
    let (Some(checkpoint), None) = (loaded.checkpoint, loaded.notice) else {
        return Err(io::Error::other(
            "the exact capture is not the newest readable checkpoint",
        ));
    };
    // Before this successor serves anything; Workers and terminals name no Project.
    let checkpoint = upgraded(root, &mut store, checkpoint);
    let store = Arc::new(Mutex::new(store));
    let bindings = bindings::Writer::new(
        Arc::clone(&store),
        RetainedSessionRecord::retain(root, session)?,
    );
    let kept = Kept {
        store,
        saver: None,
        suppress_initial_empty: false,
        bindings: Some(Arc::new(Mutex::new(bindings))),
        turns: None,
    };
    Ok((record, kept, checkpoint))
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
    let mut store = SessionStore::open(root, session, MAXIMUM_WORKBENCH_CHECKPOINT_BYTES)?;
    // After a fallback, what the held version migrated comes back before anything reads it.
    store.restore_after_fallback(
        root,
        |bytes| WorkbenchCheckpoint::parse(bytes).is_ok(),
        crate::terminal_output::readable,
    );
    let loaded = store.read_validated(|bytes| WorkbenchCheckpoint::parse(bytes).ok())?;
    let suppress_initial_empty = loaded.checkpoint.is_none()
        && loaded.notice == Some(orchbee_local_runtime::CheckpointRestoreNotice::Empty);
    store.publish_restore_notice(&record, loaded.notice)?;
    let checkpoint = loaded
        .checkpoint
        .map(|checkpoint| upgraded(root, &mut store, checkpoint));
    Ok((store, checkpoint, record, suppress_initial_empty))
}

const CHECKPOINT_UPGRADE_FAILED: &str = "checkpoint-upgrade-failed";

/// Takes a checkpoint of an earlier format to this build's before anything is served: the step
/// runs in memory with a seed from operating-system randomness, every slot is copied into this
/// version's upgrade folder, and the new payload is published. When any of them fails, nothing is
/// replaced, the failure is logged, and the Session runs on the checkpoint as it was, which the
/// next start upgrades again.
fn upgraded(
    root: &DataRoot,
    store: &mut SessionStore,
    checkpoint: WorkbenchCheckpoint,
) -> WorkbenchCheckpoint {
    if !checkpoint.upgrade_needed() {
        return checkpoint;
    }
    let mut seed = [0; 16];
    if getrandom::fill(&mut seed).is_err() {
        failed("seed", None);
        return checkpoint;
    }
    let (next, encoded) = match checkpoint.upgrade(&seed) {
        Ok(Some(upgrade)) => upgrade,
        Ok(None) => return checkpoint,
        Err(_) => {
            failed("step", None);
            return checkpoint;
        }
    };
    match store.publish_upgrade(root, &encoded, |bytes| {
        WorkbenchCheckpoint::parse(bytes).is_ok()
    }) {
        Ok(()) => next,
        Err(error) => {
            failed("publish", Some(error.kind()));
            checkpoint
        }
    }
}

fn failed(stage: &'static str, error: Option<io::ErrorKind>) {
    let mut fields = vec![("stage", LogValue::Token(stage))];
    if let Some(error) = error {
        fields.push(("error", LogValue::from(error)));
    }
    orchbee_local_runtime::log_event(LogLevel::Warning, CHECKPOINT_UPGRADE_FAILED, &fields);
}

#[cfg(test)]
mod tests;

/// No launch has been sent; this Worker cannot own a Host or terminal.
#[cfg(any(windows, target_os = "macos"))]
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
