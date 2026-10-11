//! One observer owns one native read lane and one coalescing result slot.
use super::{Connection, Overview, Ready};
use crate::{
    local_client::{
        LocalClient,
        workbench::{SnapshotReadError, SnapshotReadFailure},
    },
    machines::{Profile, ProfileId},
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, WorkbenchRequestId, WorkbenchSnapshotOutcome,
    WorkbenchWireError,
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) enum Event {
    Healthy { ready: Ready, online_since: Instant },
    Failed { permanent: bool, at: Instant },
}
pub(super) struct Update {
    pub(super) generation: u64,
    pub(super) event: Event,
}
pub(super) struct Worker {
    cancel: Arc<AtomicU8>,
    latest: Arc<Mutex<Option<Update>>>,
    task: Option<JoinHandle<()>>,
}
impl Worker {
    pub(super) fn start(profile: Profile, generation: u64) -> io::Result<Self> {
        Self::spawn(generation, move |cancel| {
            crate::machine_tunnel::connect(
                &profile,
                Instant::now() + Duration::from_secs(60),
                cancel,
            )
        })
    }
    pub(super) fn spawn(
        generation: u64,
        connect: impl FnOnce(&AtomicU8) -> io::Result<LocalClient> + Send + 'static,
    ) -> io::Result<Self> {
        let cancel = Arc::new(AtomicU8::new(0));
        let latest = Arc::new(Mutex::new(None));
        let worker_cancel = cancel.clone();
        let worker_latest = latest.clone();
        let task = thread::Builder::new()
            .name("orchbee-machine-observe".into())
            .spawn(move || {
                let mut retained_origin = None;
                let result = connect(&worker_cancel).and_then(|client| {
                    retained_origin = Some(client.origin());
                    observe(client, generation, &worker_cancel, &worker_latest)
                });
                if worker_cancel.load(Ordering::Acquire) == 0
                    && let Err(error) = result
                {
                    *worker_latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(Update {
                        generation,
                        event: Event::Failed {
                            permanent: permanent(error.kind()),
                            at: Instant::now(),
                        },
                    });
                }
                // The UI acknowledges failure by cancelling after dropping its cached origin.
                // Keep the last transport guard here so SSH teardown cannot block a UI tick.
                if retained_origin.is_some() {
                    while worker_cancel.load(Ordering::Acquire) == 0 {
                        thread::sleep(Duration::from_millis(20));
                    }
                    worker_latest
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    drop(retained_origin);
                }
            })?;
        Ok(Self {
            cancel,
            latest,
            task: Some(task),
        })
    }
    pub(super) fn cancel(&self) {
        self.cancel.store(1, Ordering::Release);
    }
    pub(super) fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub(super) fn take(&self) -> Option<Update> {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
fn observe(
    mut client: LocalClient,
    generation: u64,
    cancel: &AtomicU8,
    latest: &Mutex<Option<Update>>,
) -> io::Result<()> {
    let connection = Connection {
        origin: client.origin(),
        clock: client.operation_clock(),
    };
    let mut online_since = None;
    while cancel.load(Ordering::Acquire) == 0 {
        let identity = ProfileId::generate().map_err(|_| io::ErrorKind::Other)?;
        let request = SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(identity.as_str()).map_err(io::Error::other)?,
        };
        let result = client
            .submit_workbench_snapshot_read_before_cancellable(
                request,
                Instant::now() + Duration::from_secs(5),
                Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
            )
            .map_err(snapshot_error)?;
        let overview = match result.outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => Overview::from_snapshot(&snapshot),
            WorkbenchSnapshotOutcome::Denied {} => {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            WorkbenchSnapshotOutcome::Unavailable { .. } => {
                return Err(io::ErrorKind::ConnectionRefused.into());
            }
        };
        let observed = Instant::now();
        *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(Update {
            generation,
            event: Event::Healthy {
                ready: Ready {
                    overview,
                    connection: connection.clone(),
                    observed,
                },
                online_since: *online_since.get_or_insert(observed),
            },
        });
        let next = observed + Duration::from_secs(2);
        while cancel.load(Ordering::Acquire) == 0 && Instant::now() < next {
            thread::sleep(Duration::from_millis(20));
        }
    }
    Ok(())
}
fn permanent(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::PermissionDenied
            | io::ErrorKind::InvalidData
            | io::ErrorKind::NotFound
            | io::ErrorKind::Unsupported
            | io::ErrorKind::InvalidInput
    )
}
fn snapshot_error(error: SnapshotReadError) -> io::Error {
    match error {
        SnapshotReadError::SessionUnavailable => io::ErrorKind::NotConnected.into(),
        SnapshotReadError::RequestNotSubmitted(error) => error,
        SnapshotReadError::Failed {
            reason:
                SnapshotReadFailure::Write(error)
                | SnapshotReadFailure::ResponseDeadline(error)
                | SnapshotReadFailure::Response(WorkbenchWireError::Io(error)),
            ..
        } => error,
        _ => io::ErrorKind::InvalidData.into(),
    }
}

#[cfg(test)]
mod tests;
