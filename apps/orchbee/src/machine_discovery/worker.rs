//! One observer promotes its bootstrap carrier before publishing an origin, then consumes push.
use super::{Connection, Overview, Ready};
use crate::local_client::cancellation::{Cancellation, OwnedCancellation};
use crate::{
    local_client::{LocalClient, multiplex::StreamEvent},
    machines::{Profile, ProfileId},
};
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, StreamMode, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response,
};
use std::{
    io,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) enum Event {
    Healthy { ready: Ready, online_since: Instant },
    Reset,
    Failed { permanent: bool, at: Instant },
}
pub(super) struct Update {
    pub(super) generation: u64,
    pub(super) event: Event,
}
pub(super) struct Worker {
    cancel: Arc<OwnedCancellation>,
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
        connect: impl FnOnce(Cancellation<'_>) -> io::Result<LocalClient> + Send + 'static,
    ) -> io::Result<Self> {
        let cancel = Arc::new(OwnedCancellation::new()?);
        let latest = Arc::new(Mutex::new(None));
        let worker_cancel = cancel.clone();
        let worker_latest = latest.clone();
        let task = thread::Builder::new()
            .name("orchbee-machine-observe".into())
            .spawn(move || {
                let mut retained_origin = None;
                let result = connect(worker_cancel.borrow())
                    .and_then(LocalClient::into_multiplexed)
                    .and_then(|client| {
                        // No Direct origin escapes: every derived read uses this actor's carrier.
                        retained_origin = Some(client.origin());
                        observe(client, generation, worker_cancel.borrow(), &worker_latest)
                    });
                if !worker_cancel.is_cancelled()
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
                    while !worker_cancel.is_cancelled() {
                        let mut sources = [PollFd::new(
                            worker_cancel
                                .borrow()
                                .wake()
                                .expect("owned cancellation wake"),
                            PollFlags::POLLIN,
                        )];
                        if poll(&mut sources, PollTimeout::NONE).is_err()
                            || sources[0].revents().is_some_and(|flags| !flags.is_empty())
                        {
                            break;
                        }
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
        self.cancel.cancel();
    }
    pub(super) fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
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
    client: LocalClient,
    generation: u64,
    cancel: Cancellation<'_>,
    latest: &Mutex<Option<Update>>,
) -> io::Result<()> {
    let connection = Connection {
        origin: client.origin(),
        clock: client.operation_clock(),
    };
    let mut online_since = None;
    let identity = ProfileId::generate().map_err(|_| io::ErrorKind::Other)?;
    let request = SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(identity.as_str()).map_err(io::Error::other)?,
    };
    let stream = client.origin().subscribe(
        &WorkbenchRequest::SnapshotRead(request.clone()),
        StreamMode::State,
    )?;
    while !cancel.is_cancelled()? {
        let record = match stream.wait(cancel.wake())? {
            StreamEvent::Record(record) => record,
            StreamEvent::Gap | StreamEvent::Disconnected => {
                online_since = None;
                *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(Update {
                    generation,
                    event: Event::Reset,
                });
                continue;
            }
        };
        let mut encoded = std::io::Cursor::new(record.as_slice());
        let WorkbenchResponse::Snapshot(result) =
            read_workbench_response(&mut encoded).map_err(|_| io::ErrorKind::InvalidData)?
        else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        if encoded.position() != record.len() as u64 || result.request_id != request.request_id {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let overview = match result.outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => {
                Overview::from_snapshot(&snapshot).map_err(|_| io::ErrorKind::InvalidData)?
            }
            WorkbenchSnapshotOutcome::Denied {} => {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            WorkbenchSnapshotOutcome::Unavailable { .. } => {
                online_since = None;
                *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(Update {
                    generation,
                    event: Event::Reset,
                });
                continue;
            }
        };
        let observed = Instant::now();
        *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(Update {
            generation,
            event: Event::Healthy {
                ready: Ready {
                    overview,
                    connection: connection.clone(),
                },
                online_since: *online_since.get_or_insert(observed),
            },
        });
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
#[cfg(test)]
mod tests;
