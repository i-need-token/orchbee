use std::{collections::BTreeMap, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, CapabilityNodeMessage,
    HostLaunchState, HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchFailure,
    TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection,
    TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
    TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection, WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateAcceptedState, CreateFailureReason, CreateOutcome, CreateRejectedReason,
    CreateUnavailableReason, Geometry, RequestId, ServerMessage, StartFailureReason, StartMode,
    StartOutcome, StartRejectedReason, StartTransition, StartUnavailableReason,
    TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest, TerminalStartResult,
    WorkerConstraint,
};

mod outcomes;

const MAXIMUM_TERMINAL_BINDINGS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerPlacement {
    pub(crate) worker_id: WorkerId,
    pub(crate) generation: WorkerGeneration,
}

#[derive(Debug)]
pub(crate) enum PreparedOperation {
    Ready(ServerMessage),
    Dispatch(Box<PendingOperation>),
}

#[derive(Debug)]
pub(crate) struct PendingOperation {
    command: CapabilityNodeMessage,
    request: ClientMessage,
    create_id: RequestId,
}

impl PendingOperation {
    pub(crate) const fn command(&self) -> &CapabilityNodeMessage {
        &self.command
    }

    fn authorization(&self) -> TerminalOperationAuthorization {
        match &self.command {
            CapabilityNodeMessage::AuthorizedTerminalLaunch(command) => command.authorization(),
            CapabilityNodeMessage::AuthorizedTerminalStart(command) => command.authorization(),
            _ => unreachable!("only the coordinator constructs pending commands"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Completion {
    Launch(orchbee_capability_node_protocol::TerminalLaunchResult),
    Start(orchbee_capability_node_protocol::TerminalStartResult),
    Unavailable,
    Busy,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
enum TerminalPhase {
    Creating,
    AwaitingStart(HostTerminalIdentity),
    Starting(HostTerminalIdentity),
    Started {
        identity: HostTerminalIdentity,
        geometry: Geometry,
    },
    Unknown(Option<HostTerminalIdentity>),
    Unavailable(Option<HostTerminalIdentity>),
}

impl TerminalPhase {
    const fn identity(self) -> Option<HostTerminalIdentity> {
        match self {
            Self::Creating => None,
            Self::AwaitingStart(identity)
            | Self::Starting(identity)
            | Self::Started { identity, .. } => Some(identity),
            Self::Unknown(identity) | Self::Unavailable(identity) => identity,
        }
    }
}

#[derive(Debug)]
struct TerminalBinding {
    authorization: TerminalOperationAuthorization,
    deferred: bool,
    phase: TerminalPhase,
}

#[derive(Debug)]
pub(crate) struct TerminalCoordinator {
    bindings: BTreeMap<RequestId, TerminalBinding>,
    next_identity: Option<u128>,
}

impl TerminalCoordinator {
    pub(crate) const fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
            next_identity: Some(1),
        }
    }

    pub(crate) fn viewer_target(
        &self,
        reference: &RequestId,
        worker: WorkerPlacement,
    ) -> Option<HostTerminalIdentity> {
        let binding = self.bindings.get(reference)?;
        if binding.authorization.worker_id() != worker.worker_id
            || binding.authorization.worker_generation() != worker.generation
        {
            return None;
        }
        match binding.phase {
            TerminalPhase::AwaitingStart(identity) | TerminalPhase::Started { identity, .. } => {
                Some(identity)
            }
            _ => None,
        }
    }

    pub(crate) fn prepare(
        &mut self,
        request: ClientMessage,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> PreparedOperation {
        match request {
            ClientMessage::TerminalControl(request) => PreparedOperation::Ready(ServerMessage::TerminalControlResult(orchbee_client_server_protocol::TerminalControlResult::new(request.request_id().clone(),orchbee_client_server_protocol::TerminalControlOutcome::Rejected {reason:orchbee_client_server_protocol::ControlRejectedReason::ViewerUnavailable}))),
            ClientMessage::TerminalViewer(request) => PreparedOperation::Ready(ServerMessage::TerminalViewerResult(
                orchbee_client_server_protocol::TerminalViewerResult::new(request.request_id().clone(),
                    orchbee_client_server_protocol::TerminalViewerOutcome::Rejected {
                        reason: orchbee_client_server_protocol::ViewerRejectedReason::TerminalUnavailable,
                    }))),
            ClientMessage::TerminalCreate(request) => self.prepare_create(request, worker, expiry),
            ClientMessage::TerminalStart(request) => self.prepare_start(request, worker, expiry),
        }
    }

    fn prepare_create(
        &mut self,
        request: TerminalCreateRequest,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> PreparedOperation {
        let ready = |outcome| PreparedOperation::Ready(create_result(&request, outcome));
        if self.bindings.contains_key(request.request_id()) {
            return ready(CreateOutcome::Rejected {
                reason: CreateRejectedReason::NotAuthorized,
            });
        }
        let Some(worker) = worker else {
            return ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::NoCompatibleWorker,
            });
        };
        if let Some(WorkerConstraint::Exact { worker_id }) = request.worker_constraint()
            && worker_id.as_str() != hex_identity(worker.worker_id.as_bytes())
        {
            return ready(CreateOutcome::Rejected {
                reason: CreateRejectedReason::WorkerConstraintUnsatisfied,
            });
        }
        if self.bindings.len() >= MAXIMUM_TERMINAL_BINDINGS {
            return ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            });
        }
        let cwd = match request
            .cwd()
            .map(|cwd| AbsoluteCwd::new(cwd.as_str()))
            .transpose()
        {
            Ok(cwd) => cwd,
            Err(_) => {
                return ready(CreateOutcome::Rejected {
                    reason: CreateRejectedReason::CwdNotAccepted,
                });
            }
        };
        let mode = match request.start_mode() {
            StartMode::Deferred => TerminalLaunchMode::Deferred,
            StartMode::Immediate { geometry } => match worker_geometry(*geometry) {
                Some(geometry) => TerminalLaunchMode::Immediate(geometry),
                None => {
                    return ready(CreateOutcome::Failed {
                        reason: CreateFailureReason::TerminalCreateFailed,
                    });
                }
            },
        };
        let Some(expiry) = expiry else {
            return ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            });
        };
        let Some(bytes) = self.allocate_identity() else {
            return ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            });
        };
        let Some(authorization) = make_authorization(bytes, bytes, worker, NonZeroU64::MIN, expiry)
        else {
            return ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            });
        };
        self.bindings.insert(
            request.request_id().clone(),
            TerminalBinding {
                authorization,
                deferred: matches!(mode, TerminalLaunchMode::Deferred),
                phase: TerminalPhase::Creating,
            },
        );
        PreparedOperation::Dispatch(Box::new(PendingOperation {
            command: CapabilityNodeMessage::AuthorizedTerminalLaunch(
                AuthorizedTerminalLaunch::new(authorization, cwd, mode),
            ),
            create_id: request.request_id().clone(),
            request: ClientMessage::TerminalCreate(request),
        }))
    }

    fn prepare_start(
        &mut self,
        request: TerminalStartRequest,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> PreparedOperation {
        let ready = |outcome| PreparedOperation::Ready(start_result(&request, outcome));
        if self.bindings.contains_key(request.request_id()) {
            return ready(StartOutcome::Rejected {
                reason: StartRejectedReason::NotAuthorized,
            });
        }
        let Some(binding) = self.bindings.get(request.terminal_create_request_id()) else {
            return ready(StartOutcome::Rejected {
                reason: StartRejectedReason::TerminalCreateUnavailable,
            });
        };
        if matches!(
            binding.phase,
            TerminalPhase::Creating | TerminalPhase::Starting(_)
        ) {
            return ready(StartOutcome::Unavailable {
                reason: StartUnavailableReason::ServerBusy,
            });
        }
        if matches!(
            binding.phase,
            TerminalPhase::Unknown(_) | TerminalPhase::Unavailable(_)
        ) {
            return ready(StartOutcome::Rejected {
                reason: StartRejectedReason::TerminalCreateUnavailable,
            });
        }
        if !binding.deferred {
            return ready(StartOutcome::Rejected {
                reason: StartRejectedReason::TerminalNotAwaitingStart,
            });
        }
        let Some(worker) =
            worker.filter(|worker| matches_placement(binding.authorization, *worker))
        else {
            return ready(StartOutcome::Unavailable {
                reason: StartUnavailableReason::TerminalRuntimeUnavailable,
            });
        };
        let identity = match binding.phase {
            TerminalPhase::AwaitingStart(identity) => identity,
            TerminalPhase::Started {
                identity: _,
                geometry,
            } => {
                return ready(if geometry == request.geometry() {
                    StartOutcome::Accepted {
                        geometry,
                        transition: StartTransition::AlreadyStarted,
                    }
                } else {
                    StartOutcome::Rejected {
                        reason: StartRejectedReason::GeometryConflict,
                    }
                });
            }
            _ => {
                return ready(StartOutcome::Rejected {
                    reason: StartRejectedReason::TerminalCreateUnavailable,
                });
            }
        };
        let Some(fence) = binding
            .authorization
            .fence()
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
        else {
            return ready(StartOutcome::Unavailable {
                reason: StartUnavailableReason::ServerBusy,
            });
        };
        let terminal_bytes = binding.authorization.terminal_id().as_bytes();
        let Some((expiry, geometry)) = expiry.zip(worker_geometry(request.geometry())) else {
            return ready(StartOutcome::Unavailable {
                reason: StartUnavailableReason::ServerBusy,
            });
        };
        let Some(bytes) = self.allocate_identity() else {
            return ready(StartOutcome::Unavailable {
                reason: StartUnavailableReason::ServerBusy,
            });
        };
        let Some(authorization) = make_authorization(terminal_bytes, bytes, worker, fence, expiry)
        else {
            return ready(StartOutcome::Unavailable {
                reason: StartUnavailableReason::ServerBusy,
            });
        };
        let Some(binding) = self.bindings.get_mut(request.terminal_create_request_id()) else {
            return ready(StartOutcome::Rejected {
                reason: StartRejectedReason::TerminalCreateUnavailable,
            });
        };
        binding.authorization = authorization;
        binding.phase = TerminalPhase::Starting(identity);
        PreparedOperation::Dispatch(Box::new(PendingOperation {
            command: CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
                authorization,
                identity,
                geometry,
            )),
            create_id: request.terminal_create_request_id().clone(),
            request: ClientMessage::TerminalStart(request),
        }))
    }

    pub(crate) fn complete(
        &mut self,
        pending: &PendingOperation,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> ServerMessage {
        let Some(binding) = self.bindings.get_mut(&pending.create_id) else {
            return unknown_result(&pending.request);
        };
        if binding.authorization != pending.authorization()
            || !matches!(
                binding.phase,
                TerminalPhase::Creating | TerminalPhase::Starting(_)
            )
        {
            return unknown_result(&pending.request);
        }
        // A registry snapshot is taken while the service holds the admission lock through commit.
        let completion = if matches!(completion, Completion::Launch(_) | Completion::Start(_))
            && !current_worker
                .is_some_and(|worker| matches_placement(binding.authorization, worker))
        {
            Completion::Unknown
        } else {
            completion
        };
        let (result, phase) = outcomes::settle(pending, completion, binding.phase);
        binding.phase = phase;
        result
    }

    pub(crate) fn allocate_identity(&mut self) -> Option<[u8; 16]> {
        let identity = self.next_identity?;
        self.next_identity = identity.checked_add(1);
        Some(identity.to_be_bytes())
    }
}

fn make_authorization(
    terminal: [u8; 16],
    operation: [u8; 16],
    worker: WorkerPlacement,
    fence: NonZeroU64,
    expiry: LeaseExpiresAtUnixMilliseconds,
) -> Option<TerminalOperationAuthorization> {
    Some(TerminalOperationAuthorization::new(
        TerminalId::from_bytes(terminal).ok()?,
        OperationId::from_bytes(operation).ok()?,
        worker.worker_id,
        worker.generation,
        TerminalFence::new(fence),
        TerminalOperationLease::new(LeaseId::from_bytes(operation).ok()?, expiry),
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    ))
}

fn matches_placement(
    authorization: TerminalOperationAuthorization,
    worker: WorkerPlacement,
) -> bool {
    authorization.worker_id() == worker.worker_id
        && authorization.worker_generation() == worker.generation
}
fn worker_geometry(geometry: Geometry) -> Option<TerminalPixelGeometry> {
    TerminalPixelGeometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .ok()
}
fn client_geometry(geometry: TerminalPixelGeometry) -> Option<Geometry> {
    Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .ok()
}
fn hex_identity(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}
fn create_result(request: &TerminalCreateRequest, outcome: CreateOutcome) -> ServerMessage {
    ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
        request.request_id().clone(),
        outcome,
    ))
}
fn start_result(request: &TerminalStartRequest, outcome: StartOutcome) -> ServerMessage {
    ServerMessage::TerminalStartResult(TerminalStartResult::new(
        request.request_id().clone(),
        outcome,
    ))
}
fn unknown_result(request: &ClientMessage) -> ServerMessage {
    match request {
        ClientMessage::TerminalControl(request) => ServerMessage::TerminalControlResult(orchbee_client_server_protocol::TerminalControlResult::new(request.request_id().clone(),orchbee_client_server_protocol::TerminalControlOutcome::OutcomeUnknown {})),
        ClientMessage::TerminalViewer(request) => ServerMessage::TerminalViewerResult(orchbee_client_server_protocol::TerminalViewerResult::new(
            request.request_id().clone(), orchbee_client_server_protocol::TerminalViewerOutcome::Unavailable {
                reason: orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            })),
        ClientMessage::TerminalCreate(request) => {
            create_result(request, CreateOutcome::OutcomeUnknown)
        }
        ClientMessage::TerminalStart(request) => {
            start_result(request, StartOutcome::OutcomeUnknown)
        }
    }
}

#[cfg(test)]
mod tests;
