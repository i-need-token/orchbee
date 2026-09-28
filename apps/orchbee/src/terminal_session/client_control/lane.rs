//! A separate Windows connection with a bounded latest-sample handoff and owned shutdown.
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewCompletion, ClientViewControl, ClientViewIntent,
    ClientViewObservation, ClientViewOutcome, ClientViewSequence, ProtocolVersion, SiteId,
    Snapshot, SnapshotReadIntent, WorkbenchClientViewRequest, WorkbenchRequestId,
    WorkbenchSnapshotOutcome,
};
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct Sample {
    pub(super) heartbeat: ClientViewSequence,
    pub(super) revision: ClientViewSequence,
    pub(super) view: ClientViewObservation,
}
pub(super) struct Delivery {
    pub(super) command: ClientViewControl,
    pub(super) layout_snapshot: Option<Snapshot>,
}
#[derive(Default)]
struct Handoff {
    sample: Option<Sample>,
    completion: Option<(Sample, ClientViewCompletion)>,
    control: Option<Delivery>,
    stopped: bool,
    failed: bool,
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;

pub(super) struct ClientControlLane {
    shared: Shared,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl ClientControlLane {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        site_id: SiteId,
    ) -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (wake_read, wake) = LocalStream::pair()?;
        let worker_shared = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-control".into())
            .spawn(move || {
                run(&worker_shared, origin, clock, site_id, &wake_read);
                let (lock, signal) = &*worker_shared;
                if let Ok(mut handoff) = lock.lock() {
                    handoff.failed = true;
                }
                signal.notify_all();
            })?;
        Ok(Self {
            shared,
            wake,
            task: Some(task),
        })
    }
    pub(super) fn publish(&self, sample: Sample) -> bool {
        let (lock, signal) = &*self.shared;
        let Ok(mut handoff) = lock.lock() else {
            return false;
        };
        if handoff.failed || handoff.stopped {
            return false;
        }
        handoff.sample = Some(sample);
        signal.notify_one();
        true
    }
    pub(super) fn complete(&self, sample: Sample, completion: ClientViewCompletion) -> bool {
        let (lock, signal) = &*self.shared;
        let Ok(mut handoff) = lock.lock() else {
            return false;
        };
        if handoff.failed || handoff.stopped || handoff.completion.is_some() {
            return false;
        }
        // Completion pins the successful draw's observation. Later UI samples cannot replace it.
        handoff.sample = Some(sample.clone());
        handoff.completion = Some((sample, completion));
        signal.notify_one();
        true
    }
    pub(super) fn take_control(&self) -> Option<Delivery> {
        self.shared.0.lock().ok()?.control.take()
    }
    pub(super) fn failed(&self) -> bool {
        self.shared.0.lock().map_or(true, |handoff| handoff.failed)
    }
    pub(super) fn close(&mut self) {
        if self.task.is_none() {
            return;
        }
        let (lock, signal) = &*self.shared;
        if let Ok(mut handoff) = lock.lock() {
            handoff.stopped = true;
        }
        signal.notify_all();
        // Closing our wake endpoint interrupts admission and exchange without waiting for a peer.
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
impl Drop for ClientControlLane {
    fn drop(&mut self) {
        self.close();
    }
}

fn run(
    shared: &Shared,
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    site_id: SiteId,
    wake: &LocalStream,
) {
    let Ok(mut client) = origin.reconnect_before_cancellable(
        clock.now() + Duration::from_secs(2),
        clock.clone(),
        Some(wake.as_local_io()),
    ) else {
        return;
    };
    let mut exchange_id = 0_u64;
    let mut next_exchange = Instant::now();
    loop {
        let (lock, signal) = &**shared;
        let Ok(mut handoff) = lock.lock() else {
            return;
        };
        while !handoff.stopped && (handoff.sample.is_none() || Instant::now() < next_exchange) {
            if handoff.sample.is_none() {
                let Ok(next) = signal.wait(handoff) else {
                    return;
                };
                handoff = next;
            } else {
                let duration = next_exchange.saturating_duration_since(Instant::now());
                let Ok((next, _)) = signal.wait_timeout(handoff, duration) else {
                    return;
                };
                handoff = next;
            }
        }
        if handoff.stopped {
            return;
        }
        let (sample, completion) = if let Some((sample, completion)) = &handoff.completion {
            (sample.clone(), Some(completion.clone()))
        } else if let Some(sample) = &handoff.sample {
            (sample.clone(), None)
        } else {
            continue;
        };
        drop(handoff);
        let Some(next) = exchange_id.checked_add(1) else {
            return;
        };
        exchange_id = next;
        let Ok(request_id) = WorkbenchRequestId::parse(&format!("{exchange_id:032x}")) else {
            return;
        };
        let request = WorkbenchClientViewRequest {
            protocol_version: ProtocolVersion,
            request_id,
            site_id: site_id.clone(),
            action: ClientViewAction::Synchronize {
                heartbeat: sample.heartbeat,
                view_revision: sample.revision,
                view: Box::new(sample.view),
                completion: completion.clone(),
            },
        };
        let Ok(result) = client.exchange_client_view(
            request,
            clock.now() + Duration::from_secs(2),
            Some(wake.as_local_io()),
        ) else {
            return;
        };
        let ClientViewOutcome::Synchronized { control, .. } = result.outcome else {
            return;
        };
        let delivery = if let Some(control) = control {
            let layout_snapshot = if matches!(
                control.intent,
                ClientViewIntent::Selections { .. }
                    | ClientViewIntent::Presentation { .. }
                    | ClientViewIntent::Viewports { .. }
                    | ClientViewIntent::Layout { .. }
                    | ClientViewIntent::FocusDirection { .. }
                    | ClientViewIntent::Resize { .. }
                    | ClientViewIntent::SetSplitRatio { .. }
            ) {
                let Some(next) = exchange_id.checked_add(1) else {
                    return;
                };
                exchange_id = next;
                let Ok(request_id) = WorkbenchRequestId::parse(&format!("{exchange_id:032x}"))
                else {
                    return;
                };
                let Ok(result) = client.submit_workbench_snapshot_read_before_cancellable(
                    SnapshotReadIntent {
                        protocol_version: ProtocolVersion,
                        request_id,
                    },
                    clock.now() + Duration::from_secs(2),
                    Some(wake.as_local_io()),
                ) else {
                    return;
                };
                match result.outcome {
                    WorkbenchSnapshotOutcome::Available { snapshot } => Some(snapshot),
                    WorkbenchSnapshotOutcome::Unavailable { .. }
                    | WorkbenchSnapshotOutcome::Denied {} => None,
                }
            } else {
                None
            };
            Some(Delivery {
                command: *control,
                layout_snapshot,
            })
        } else {
            None
        };
        let Ok(mut handoff) = lock.lock() else {
            return;
        };
        if let Some(completion) = completion {
            if handoff
                .completion
                .as_ref()
                .is_none_or(|(_, queued)| *queued != completion)
            {
                return;
            }
            handoff.completion = None;
        }
        if let Some(delivery) = delivery {
            if handoff.control.is_some() {
                return;
            }
            handoff.control = Some(delivery);
        }
        next_exchange = Instant::now() + Duration::from_millis(100);
    }
}

#[cfg(test)]
impl ClientControlLane {
    pub(super) fn for_test() -> Self {
        let (_reader, wake) = LocalStream::pair().unwrap();
        Self {
            shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
            wake,
            task: None,
        }
    }
    pub(super) fn inject_for_test(&self, control: ClientViewControl) {
        self.inject_delivery_for_test(Delivery {
            command: control,
            layout_snapshot: None,
        });
    }
    pub(super) fn inject_delivery_for_test(&self, delivery: Delivery) {
        assert!(
            self.shared
                .0
                .lock()
                .unwrap()
                .control
                .replace(delivery)
                .is_none()
        );
    }
    pub(super) fn completion_for_test(&self) -> Option<(Sample, ClientViewCompletion)> {
        self.shared.0.lock().unwrap().completion.clone()
    }
}

#[cfg(test)]
mod tests;
