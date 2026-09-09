use std::{
    fs::File,
    io::Read,
    num::NonZeroU64,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateUnavailableReason, RequestId, ServerMessage, StartOutcome,
    StartUnavailableReason, TerminalCreateResult, TerminalStartResult,
};

use crate::{
    site_coordinator::SiteCoordinator,
    terminal_coordinator::{Completion, PreparedOperation, WorkerPlacement},
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
    worker_registry::WorkerRegistry,
};

mod close_maintenance;
mod controller_session;
mod panel_split;
mod projection;
mod standalone;
mod tab_close;
mod tab_create;
mod viewer_session;
mod workspace_directory;

// One queued plus one in-flight exchange, each with a two-second write and twenty-second read.
const LEASE_LIFETIME_MILLISECONDS: u64 = 46_000;

pub(crate) struct LocalTerminalService {
    close_maintenance: close_maintenance::CloseMaintenance,
    standalone_changed: std::sync::Condvar,
    standalone_wait: std::time::Duration,
    workers: Arc<Mutex<WorkerRegistry>>,
    coordinator: Mutex<SiteCoordinator>,
    controllers: Mutex<
        std::collections::BTreeMap<
            controller_session::ControlTarget,
            orchbee_capability_node_protocol::OperationId,
        >,
    >,
}

impl LocalTerminalService {
    pub(crate) fn new(workers: Arc<Mutex<WorkerRegistry>>) -> Self {
        Self::from_identity(workers, random_site_id())
    }

    fn from_identity(
        workers: Arc<Mutex<WorkerRegistry>>,
        identity: std::io::Result<orchbee_workbench_protocol::SiteId>,
    ) -> Self {
        let workbench = identity
            .ok()
            .and_then(|site| orchbee_server::WorkbenchAuthority::new(site).ok());
        Self {
            close_maintenance: close_maintenance::CloseMaintenance::default(),
            standalone_changed: std::sync::Condvar::new(),
            standalone_wait: std::time::Duration::from_secs(46),
            workers,
            coordinator: Mutex::new(SiteCoordinator::new(workbench)),
            controllers: Mutex::new(std::collections::BTreeMap::new()),
        }
    }

    pub(crate) fn workbench_site_matches(
        &self,
        site: &orchbee_workbench_protocol::SiteId,
    ) -> Option<bool> {
        self.coordinator.lock().ok()?.workbench_site_matches(site)
    }

    pub(crate) fn execute_workbench(
        &self,
        request: orchbee_server::WorkbenchReadOrNavigationRequest,
        manage_navigation: bool,
    ) -> Option<orchbee_workbench_protocol::WorkbenchResponse> {
        self.coordinator
            .lock()
            .ok()?
            .execute_workbench(request, manage_navigation)
    }

    pub(crate) fn execute(&self, request: ClientMessage) -> ServerMessage {
        if let ClientMessage::TerminalOpen(request) = request {
            let outcome = self.open_standalone(&request);
            return ServerMessage::TerminalOpenResult(
                orchbee_client_server_protocol::TerminalOpenResult::new(
                    request.request_id().clone(),
                    outcome,
                ),
            );
        }
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
        match coordinator.complete(
            pending,
            completion,
            placement(workers.request_handle().as_ref()),
        ) {
            Ok(result) => result,
            Err(_mismatch) => unknown_result(&busy),
        }
    }

    pub(crate) fn execute_terminal_tab_move(
        &self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        self.coordinator
            .lock()
            .ok()?
            .execute_terminal_tab_move(command)
    }
}

fn random_site_id() -> std::io::Result<orchbee_workbench_protocol::SiteId> {
    let mut bytes = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(std::io::Error::other)
}

fn create_reference() -> Option<RequestId> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    RequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes))).ok()
}

#[cfg(test)]
mod tests;

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
        Ok(
            WorkerOperationResult::CwdValidation(_)
            | WorkerOperationResult::Viewer(_)
            | WorkerOperationResult::Control(_)
            | WorkerOperationResult::Lifecycle(_),
        ) => Completion::Unknown,
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
        ClientMessage::TerminalOpen(request) => ServerMessage::TerminalOpenResult(
            orchbee_client_server_protocol::TerminalOpenResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::OpenOutcome::Unavailable {
                    reason: CreateUnavailableReason::ServerBusy,
                },
            ),
        ),
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
        ServerMessage::TerminalOpenResult(result) => ServerMessage::TerminalOpenResult(orchbee_client_server_protocol::TerminalOpenResult::new(result.request_id().clone(), orchbee_client_server_protocol::OpenOutcome::OutcomeUnknown {})),
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
