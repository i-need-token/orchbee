//! A shared-connection control subscription with bounded observations and owned cancellation.
use crate::background_thread::BackgroundThread;
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewCompletion, ClientViewControl, ClientViewControlReason as Reason,
    ClientViewIntent, ClientViewObservation, ClientViewOutcome, ClientViewSequence,
    ProtocolVersion, SessionId, Snapshot, SnapshotReadIntent, WorkbenchClientViewRequest,
    WorkbenchRequestId, WorkbenchSnapshotOutcome,
};
use std::thread::{self, JoinHandle};
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
mod configured_command;
mod split_ratio;
pub(in crate::workbench_client) use split_ratio::LayoutCompletion;

const STARTUP_ADMISSION_TOTAL_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub(in crate::workbench_client) struct Sample {
    pub(in crate::workbench_client) heartbeat: ClientViewSequence,
    pub(in crate::workbench_client) revision: ClientViewSequence,
    pub(in crate::workbench_client) view: ClientViewObservation,
    pub(in crate::workbench_client) graphics:
        Option<orchbee_workbench_protocol::PaneGraphicsClientObservation>,
}
pub(in crate::workbench_client) struct Delivery {
    pub(in crate::workbench_client) command: ClientViewControl,
    pub(in crate::workbench_client) workbench_snapshot: Option<Snapshot>,
}
/// Delivery certainty and definite local refusal are separate command failure outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum CommandFailure {
    Refused(orchbee_workbench_protocol::ConfiguredCommandFailure),
    NotSubmitted,
    OutcomeUnknown,
}

pub(in crate::workbench_client) struct CommandCompletion {
    /// Exact installed identity, retained even when the command was refused before transport.
    pub(crate) command_id: String,
    pub(crate) reply: Result<orchbee_workbench_protocol::ConfiguredCommandResult, CommandFailure>,
    pub(crate) focus: Option<Result<orchbee_workbench_protocol::WorkbenchClientViewResult, bool>>,
}
impl CommandCompletion {
    /// Reports a definite local refusal without inventing a Server result.
    pub(in crate::workbench_client) fn refused(
        command_id: String,
        reason: orchbee_workbench_protocol::ConfiguredCommandFailure,
    ) -> Self {
        Self {
            command_id,
            reply: Err(CommandFailure::Refused(reason)),
            focus: None,
        }
    }
}

struct QueuedInvocation {
    command_id: String,
    source: Option<orchbee_workbench_protocol::ClientPopupSource>,
    revision: ClientViewSequence,
    view: ClientViewObservation,
}

impl QueuedInvocation {
    fn bind(
        self,
        record: &orchbee_workbench_protocol::ClientViewRecord,
    ) -> Result<
        orchbee_workbench_protocol::ConfiguredCommandInvocation,
        orchbee_workbench_protocol::ConfiguredCommandFailure,
    > {
        if record.view != self.view
            || record.view_revision != self.revision
            || record.availability != orchbee_workbench_protocol::ClientViewAvailability::Current
        {
            return Err(orchbee_workbench_protocol::ConfiguredCommandFailure::StaleSource);
        }
        Ok(orchbee_workbench_protocol::ConfiguredCommandInvocation {
            command_id: self.command_id,
            selection: None,
            expected_revision: self.view.workbench_revision,
            context: Box::new(orchbee_workbench_protocol::PluginInvocationContext {
                tab_id: self
                    .source
                    .as_ref()
                    .map(|source| source.tab_id.as_str().into()),
                focused_panel_id: self
                    .source
                    .as_ref()
                    .map(|source| source.panel_id.as_str().into()),
                invocation_source: Some("tui".into()),
                ..Default::default()
            }),
            source: self.source.map(|presentation| {
                orchbee_workbench_protocol::ConfiguredCommandSource {
                    client_id: record.client_id.clone(),
                    view_revision: record.view_revision,
                    presentation,
                }
            }),
        })
    }
}
#[derive(Default)]
struct Handoff {
    #[cfg(test)]
    registered: Option<orchbee_workbench_protocol::ClientViewRecord>,
    invocation: Option<QueuedInvocation>,
    invocation_pending: bool,
    invocation_result: Option<CommandCompletion>,
    layout: Option<orchbee_workbench_protocol::WorkbenchCommand>,
    layout_pending: bool,
    layout_result: Option<LayoutCompletion>,
    sample: Option<Sample>,
    completion: Option<(Sample, ClientViewCompletion)>,
    control: Option<Delivery>,
    stopped: bool,
    failed: bool,
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;

impl Handoff {
    fn take_ready_invocation(
        &mut self,
        record: &orchbee_workbench_protocol::ClientViewRecord,
    ) -> Option<QueuedInvocation> {
        // A synchronization already in flight may still describe the armed prefix. Wait for
        // the requested Ready observation; never bind to that earlier Busy registration.
        if self.invocation.as_ref()?.revision > record.view_revision {
            return None;
        }
        self.invocation.take()
    }
    fn layout_admission(&self) -> Result<(), Reason> {
        if self.stopped || self.failed {
            Err(Reason::ProjectionUnavailable)
        } else if self.layout_pending || self.layout_result.is_some() {
            Err(Reason::Busy)
        } else {
            Ok(())
        }
    }
}

pub(in crate::workbench_client) struct ClientControlLane {
    shared: Shared,
    wake: LocalStream,
    changed: LocalStream,
    task: BackgroundThread,
}
impl ClientControlLane {
    pub(in crate::workbench_client) fn layout_admission(&self) -> Result<(), Reason> {
        self.shared
            .0
            .lock()
            .map_err(|_| Reason::ProjectionUnavailable)?
            .layout_admission()
    }
    pub(in crate::workbench_client) fn submit_layout(
        &self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Result<(), Reason> {
        let (lock, signal) = &*self.shared;
        let mut handoff = lock.lock().map_err(|_| Reason::ProjectionUnavailable)?;
        handoff.layout_admission()?;
        handoff.layout_pending = true;
        handoff.layout = Some(command);
        signal.notify_one();
        drop(handoff);
        let _notified = crate::local_client::subscription::notify(&self.changed);
        Ok(())
    }
    pub(in crate::workbench_client) fn take_layout_result(&self) -> Option<LayoutCompletion> {
        self.shared.0.lock().ok()?.layout_result.take()
    }
    pub(in crate::workbench_client) fn invoke_configured(
        &self,
        id: String,
        source: Option<orchbee_workbench_protocol::ClientPopupSource>,
        sample: Sample,
    ) -> Result<(), orchbee_workbench_protocol::ConfiguredCommandFailure> {
        use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
        let (lock, signal) = &*self.shared;
        let Ok(mut handoff) = lock.lock() else {
            return Err(F::Unavailable);
        };
        if handoff.stopped || handoff.failed {
            return Err(F::Unavailable);
        }
        if handoff.invocation_pending || handoff.invocation_result.is_some() {
            return Err(F::Busy);
        }
        let invocation = QueuedInvocation {
            command_id: id,
            source,
            revision: sample.revision,
            view: sample.view.clone(),
        };
        handoff.sample = Some(sample);
        handoff.invocation_pending = true;
        handoff.invocation = Some(invocation);
        signal.notify_one();
        drop(handoff);
        let _notified = crate::local_client::subscription::notify(&self.changed);
        Ok(())
    }
    pub(in crate::workbench_client) fn take_invocation_result(&self) -> Option<CommandCompletion> {
        self.shared.0.lock().ok()?.invocation_result.take()
    }
    pub(in crate::workbench_client) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session_id: SessionId,
    ) -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (wake_read, wake) = LocalStream::pair()?;
        let (changes, changed) = LocalStream::pair()?;
        changes.set_nonblocking(true)?;
        changed.set_nonblocking(true)?;
        let worker_shared = Arc::clone(&shared);
        let task = BackgroundThread::spawn("orchbee-client-control", move || {
            run(
                &worker_shared,
                origin,
                clock,
                session_id,
                &wake_read,
                &changes,
            );
            let (lock, signal) = &*worker_shared;
            if let Ok(mut handoff) = lock.lock() {
                handoff.failed = true;
            }
            signal.notify_all();
        })?;
        Ok(Self {
            shared,
            wake,
            changed,
            task,
        })
    }
    pub(in crate::workbench_client) fn publish(&self, sample: Sample) -> bool {
        let (lock, signal) = &*self.shared;
        let Ok(mut handoff) = lock.lock() else {
            return false;
        };
        if handoff.failed || handoff.stopped {
            return false;
        }
        if handoff.sample.as_ref().is_some_and(|previous| {
            previous.revision == sample.revision
                && previous.view == sample.view
                && previous.graphics == sample.graphics
        }) {
            return true;
        }
        handoff.sample = Some(sample);
        signal.notify_one();
        drop(handoff);
        crate::local_client::subscription::notify(&self.changed).is_ok()
    }
    pub(in crate::workbench_client) fn complete(
        &self,
        sample: Sample,
        completion: ClientViewCompletion,
    ) -> bool {
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
        drop(handoff);
        crate::local_client::subscription::notify(&self.changed).is_ok()
    }
    pub(in crate::workbench_client) fn take_control(&self) -> Option<Delivery> {
        self.shared.0.lock().ok()?.control.take()
    }
    pub(in crate::workbench_client) fn failed(&self) -> bool {
        self.shared.0.lock().map_or(true, |handoff| handoff.failed)
    }
    pub(in crate::workbench_client) fn close(&mut self) {
        self.request_stop();
        let _ = self.task.join();
    }
    /// Stops the lane without waiting. The lane thread joins its own command and split-ratio
    /// helpers before it ends.
    pub(in crate::workbench_client) fn request_stop(&mut self) {
        let (lock, signal) = &*self.shared;
        if let Ok(mut handoff) = lock.lock() {
            handoff.stopped = true;
        }
        signal.notify_all();
        // Closing our wake endpoint interrupts admission and exchange without waiting for a peer.
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
        let _ = self.changed.shutdown(std::net::Shutdown::Both);
    }
    /// Process exit: observes the lane thread against the shared deadline.
    pub(in crate::workbench_client) fn stop_before(
        &mut self,
        deadline: std::time::Instant,
    ) -> std::io::Result<()> {
        self.request_stop();
        self.task.join_before(deadline)
    }
}
impl Drop for ClientControlLane {
    fn drop(&mut self) {
        // Only close and stop_before wait; an unfinished thread is detached here.
        self.request_stop();
    }
}

fn run(
    shared: &Shared,
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    session_id: SessionId,
    wake: &LocalStream,
    changes: &LocalStream,
) {
    let Ok(mut client) = origin.lane_before_cancellable(
        clock.now() + STARTUP_ADMISSION_TOTAL_TIMEOUT,
        clock.clone(),
        Some(wake.as_local_io()),
    ) else {
        return;
    };
    let mut invocation_task: Option<configured_command::Invocation> = None;
    let mut layout_task: Option<split_ratio::Mutation> = None;
    let mut exchange_id = 0_u64;
    let mut last_sample: Option<Sample> = None;
    let mut delivered_operation = None;
    // Register once before publishing any view. This stream only invalidates; a
    // replaceable stream must never own a control already marked Delivered by Server.
    let Ok(subscription) = origin.watch_changes() else {
        return;
    };
    loop {
        let (lock, signal) = &**shared;
        let Ok(mut handoff) = lock.lock() else {
            return;
        };
        while !handoff.stopped && handoff.sample.is_none() {
            let Ok(next) = signal.wait(handoff) else {
                return;
            };
            handoff = next;
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
        let changed = last_sample.as_ref().is_none_or(|previous| {
            previous.revision != sample.revision || previous.graphics != sample.graphics
        }) || completion.is_some()
            || handoff.invocation.is_some()
            || handoff.layout.is_some();
        drop(handoff);
        if !changed {
            match subscription.wait(changes) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(_) => return,
            }
        }

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
            session_id: session_id.clone(),
            action: ClientViewAction::Synchronize {
                heartbeat: sample.heartbeat,
                view_revision: sample.revision,
                view: Box::new(sample.view.clone()),
                completion: completion.clone(),
            },
        };
        // Every response is consumed before another sample or invocation can advance this
        // owner. Completion and control delivery are never rebuilt or replayed on reconnect.
        let Ok(result) = client.exchange_client_view(
            request,
            clock.now() + Duration::from_secs(2),
            Some(wake.as_local_io()),
        ) else {
            return;
        };
        last_sample = Some(sample.clone());
        let ClientViewOutcome::Synchronized {
            client: registered,
            control,
        } = result.outcome
        else {
            return;
        };
        if changed && let Some(observation) = sample.graphics {
            let Some(next) = exchange_id.checked_add(1) else {
                return;
            };
            exchange_id = next;
            let Ok(request_id) = WorkbenchRequestId::parse(&format!("{exchange_id:032x}")) else {
                return;
            };
            let request = orchbee_workbench_protocol::PaneGraphicsRequest {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: session_id.clone(),
                action: orchbee_workbench_protocol::PaneGraphicsAction::Observe {
                    client_id: registered.client_id.clone(),
                    heartbeat: registered.heartbeat,
                    observation: Box::new(observation),
                },
            };
            if client
                .pane_graphics(
                    &request,
                    clock.now() + Duration::from_secs(2),
                    Some(wake.as_local_io()),
                )
                .is_err()
            {
                return;
            }
        }
        let invocation = {
            let Ok(mut handoff) = lock.lock() else {
                return;
            };
            #[cfg(test)]
            {
                handoff.registered = Some((*registered).clone());
            }
            handoff.take_ready_invocation(&registered)
        };
        if layout_task
            .as_ref()
            .is_some_and(split_ratio::Mutation::finished)
        {
            layout_task.take();
        }
        let layout = {
            let Ok(mut handoff) = lock.lock() else {
                return;
            };
            handoff.layout.take()
        };
        if let Some(command) = layout {
            let operation_id = command.request_id.clone();
            // Admission required the previous mutation's delivered completion, so that thread is
            // only exiting; joining it lets an immediate resubmission start instead of failing.
            drop(layout_task.take());
            let started = split_ratio::Mutation::start(
                Arc::clone(shared),
                origin.clone(),
                clock.clone(),
                command,
            );
            match started {
                Ok(task) => layout_task = Some(task),
                Err(_) => {
                    let Ok(mut handoff) = lock.lock() else {
                        return;
                    };
                    handoff.layout_pending = false;
                    handoff.layout_result = Some(LayoutCompletion {
                        operation_id,
                        reply: Err(
                            crate::local_client::workbench::split_ratio::SplitRatioFailure {
                                may_have_committed: false,
                                reason: orchbee_workbench_protocol::ClientViewControlReason::Busy,
                            },
                        ),
                    });
                }
            }
        }
        if invocation_task
            .as_ref()
            .is_some_and(configured_command::Invocation::finished)
        {
            invocation_task.take();
        }
        let invocation = match invocation.map(|queued| {
            let command_id = queued.command_id.clone();
            (command_id, queued.bind(&registered))
        }) {
            Some((_, Ok(invocation))) => Some(invocation),
            Some((command_id, Err(reason))) => {
                let Ok(mut handoff) = lock.lock() else {
                    return;
                };
                handoff.invocation_pending = false;
                handoff.invocation_result = Some(CommandCompletion::refused(command_id, reason));
                None
            }
            None => None,
        };
        if let Some(invocation) = invocation {
            let command_id = invocation.command_id.clone();
            let started = if invocation_task.is_none() {
                configured_command::Invocation::start(
                    Arc::clone(shared),
                    origin.clone(),
                    clock.clone(),
                    session_id.clone(),
                    (*registered).clone(),
                    invocation,
                )
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
            match started {
                Ok(task) => invocation_task = Some(task),
                Err(_) => {
                    let Ok(mut handoff) = lock.lock() else {
                        return;
                    };
                    handoff.invocation_pending = false;
                    handoff.invocation_result = Some(CommandCompletion::refused(
                        command_id,
                        orchbee_workbench_protocol::ConfiguredCommandFailure::Unavailable,
                    ));
                }
            }
        }
        let control =
            control.filter(|control| delivered_operation.as_ref() != Some(&control.operation_id));
        let delivery = if let Some(control) = control {
            delivered_operation = Some(control.operation_id.clone());
            let workbench_snapshot = if matches!(
                control.intent,
                ClientViewIntent::CommandPopupOpen { .. }
                    | ClientViewIntent::PopupOpen { .. }
                    | ClientViewIntent::EditorOpen { .. }
                    | ClientViewIntent::Overlay { .. }
                    | ClientViewIntent::Panel { .. }
                    | ClientViewIntent::Selections { .. }
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
                workbench_snapshot,
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
    }
}

pub(in crate::workbench_client) fn random_request_id() -> Option<WorkbenchRequestId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes))).ok()
}

#[cfg(test)]
impl ClientControlLane {
    pub(in crate::workbench_client) fn for_test() -> Self {
        // Without a task, change notifications drain into the retained wake endpoint, so a write
        // never observes a closed peer.
        let (wake, changed) = LocalStream::pair().unwrap();
        changed.set_nonblocking(true).unwrap();
        Self {
            shared: Arc::new((Mutex::new(Handoff::default()), Condvar::new())),
            changed,
            wake,
            task: BackgroundThread::ended(),
        }
    }
    pub(in crate::workbench_client) fn inject_delivery_for_test(&self, delivery: Delivery) {
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
    pub(in crate::workbench_client) fn completion_for_test(
        &self,
    ) -> Option<(Sample, ClientViewCompletion)> {
        self.shared.0.lock().unwrap().completion.clone()
    }
    pub(in crate::workbench_client) fn take_layout_for_test(
        &self,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommand> {
        self.shared.0.lock().unwrap().layout.take()
    }
    pub(in crate::workbench_client) fn finish_layout_for_test(&self, result: LayoutCompletion) {
        let mut handoff = self.shared.0.lock().unwrap();
        assert!(handoff.layout_pending);
        handoff.layout_pending = false;
        assert!(handoff.layout_result.replace(result).is_none());
    }
}

#[cfg(test)]
mod tests;
