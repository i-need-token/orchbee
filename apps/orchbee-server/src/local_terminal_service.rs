use std::{
    num::NonZeroU64,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateUnavailableReason, ServerMessage, StartOutcome,
    StartUnavailableReason, TerminalCreateResult, TerminalStartResult,
};

use crate::{
    terminal_coordinator::{Completion, PreparedOperation, TerminalCoordinator, WorkerPlacement},
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
    worker_registry::WorkerRegistry,
};

mod controller_session;
mod projection;
mod viewer_session;

// One queued plus one in-flight exchange, each with a two-second write and twenty-second read.
const LEASE_LIFETIME_MILLISECONDS: u64 = 46_000;

pub(crate) struct LocalTerminalService {
    workers: Arc<Mutex<WorkerRegistry>>,
    coordinator: Mutex<TerminalCoordinator>,
    controllers: Mutex<
        std::collections::BTreeMap<
            controller_session::ControlTarget,
            orchbee_capability_node_protocol::OperationId,
        >,
    >,
}

impl LocalTerminalService {
    pub(crate) const fn new(workers: Arc<Mutex<WorkerRegistry>>) -> Self {
        Self {
            workers,
            coordinator: Mutex::new(TerminalCoordinator::new()),
            controllers: Mutex::new(std::collections::BTreeMap::new()),
        }
    }

    pub(crate) fn execute(&self, request: ClientMessage) -> ServerMessage {
        let busy = busy_result(&request);
        let (prepared, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return busy;
            };
            let handle = workers.request_handle();
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return busy;
            };
            (
                coordinator.prepare(request, placement(handle.as_ref()), lease_expiry()),
                handle,
            )
        };
        let pending = match prepared {
            PreparedOperation::Ready(result) => return result,
            PreparedOperation::Dispatch(pending) => pending,
        };
        let completion = handle.map_or(Completion::Unavailable, |handle| {
            submit(&handle, pending.command())
        });
        // Retain registry ownership through commit, so a replacement cannot race this snapshot.
        let Ok(workers) = self.workers.lock() else {
            return unknown_result(&busy);
        };
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return unknown_result(&busy);
        };
        coordinator.complete(
            &pending,
            completion,
            placement(workers.request_handle().as_ref()),
        )
    }
}

fn submit(handle: &WorkerRequestHandle, message: &CapabilityNodeMessage) -> Completion {
    let request = match message {
        CapabilityNodeMessage::AuthorizedTerminalLaunch(request) => {
            WorkerOperationRequest::Launch(request.clone())
        }
        CapabilityNodeMessage::AuthorizedTerminalStart(request) => {
            WorkerOperationRequest::Start(*request)
        }
        _ => return Completion::Unavailable,
    };
    match handle.request(request) {
        Ok(WorkerOperationResult::Launch(result)) => Completion::Launch(result),
        Ok(WorkerOperationResult::Start(result)) => Completion::Start(result),
        Ok(WorkerOperationResult::Viewer(_) | WorkerOperationResult::Control(_)) => {
            Completion::Unknown
        }
        Err(WorkerRequestError::Backpressure) => Completion::Busy,
        Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Completion::Unknown,
        Err(
            WorkerRequestError::Unavailable
            | WorkerRequestError::WrongWorker
            | WorkerRequestError::WrongGeneration
            | WorkerRequestError::InvalidRequest(_)
            | WorkerRequestError::NotDelivered(_),
        ) => Completion::Unavailable,
    }
}
fn placement(handle: Option<&WorkerRequestHandle>) -> Option<WorkerPlacement> {
    let handle = handle?;
    Some(WorkerPlacement {
        worker_id: WorkerId::from_bytes(handle.worker_id().as_bytes()).ok()?,
        generation: WorkerGeneration::new(handle.generation()),
    })
}
fn lease_expiry() -> Option<LeaseExpiresAtUnixMilliseconds> {
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()?;
    Some(LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(
        now.checked_add(LEASE_LIFETIME_MILLISECONDS)?,
    )?))
}
fn busy_result(request: &ClientMessage) -> ServerMessage {
    match request {
        ClientMessage::TerminalControl(request) => ServerMessage::TerminalControlResult(
            orchbee_client_server_protocol::TerminalControlResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalControlOutcome::Unavailable {
                    reason: orchbee_client_server_protocol::ControlUnavailableReason::ServerBusy,
                },
            ),
        ),
        ClientMessage::TerminalViewer(request) => ServerMessage::TerminalViewerResult(
            orchbee_client_server_protocol::TerminalViewerResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalViewerOutcome::Unavailable {
                    reason: orchbee_client_server_protocol::ViewerUnavailableReason::ServerBusy,
                },
            ),
        ),
        ClientMessage::TerminalCreate(request) => {
            ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::ServerBusy,
                },
            ))
        }
        ClientMessage::TerminalStart(request) => {
            ServerMessage::TerminalStartResult(TerminalStartResult::new(
                request.request_id().clone(),
                StartOutcome::Unavailable {
                    reason: StartUnavailableReason::ServerBusy,
                },
            ))
        }
    }
}
fn unknown_result(result: &ServerMessage) -> ServerMessage {
    match result {
        ServerMessage::TerminalControlResult(result) => ServerMessage::TerminalControlResult(orchbee_client_server_protocol::TerminalControlResult::new(result.request_id().clone(), orchbee_client_server_protocol::TerminalControlOutcome::OutcomeUnknown {})),
        ServerMessage::TerminalViewerResult(result) => ServerMessage::TerminalViewerResult(orchbee_client_server_protocol::TerminalViewerResult::new(
            result.request_id().clone(), orchbee_client_server_protocol::TerminalViewerOutcome::Unavailable {
                reason: orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            })),
        ServerMessage::TerminalCreateResult(result) => ServerMessage::TerminalCreateResult(
            TerminalCreateResult::new(result.request_id().clone(), CreateOutcome::OutcomeUnknown),
        ),
        ServerMessage::TerminalStartResult(result) => ServerMessage::TerminalStartResult(
            TerminalStartResult::new(result.request_id().clone(), StartOutcome::OutcomeUnknown),
        ),
    }
}
