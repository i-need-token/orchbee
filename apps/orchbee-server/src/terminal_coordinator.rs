//! Owns bounded retained terminal bindings, request identity, leases, fences, and launch or
//! start phases, preparing exact completion tokens before mutation. It issues authorized
//! intents without selecting Workbench topology or executing Host operations.

use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};

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
#[cfg(test)]
use orchbee_workbench_protocol::PresentationBinding;

mod create_inputs;
mod launch_capture;
mod outcomes;
pub(crate) use create_inputs::CreateInputs;
pub(crate) use launch_capture::LaunchCapture;

const MAXIMUM_TERMINAL_BINDINGS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerPlacement {
    pub(crate) worker_id: WorkerId,
    pub(crate) generation: WorkerGeneration,
}

impl WorkerPlacement {
    /// The Worker generation a registry request handle addresses.
    pub(crate) fn of(handle: &crate::worker_obcn_carrier::WorkerRequestHandle) -> Option<Self> {
        Some(Self {
            worker_id: WorkerId::from_bytes(handle.worker_id().as_bytes()).ok()?,
            generation: WorkerGeneration::new(handle.generation()),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BindingViewerTarget {
    pub(crate) terminal_id: TerminalId,
    pub(crate) identity: HostTerminalIdentity,
    pub(crate) worker: WorkerPlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetainedRuntimeTargetError {
    Busy,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BindingViewerTargetError {
    TerminalUnavailable,
    TerminalRuntimeUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ViewerOperationIdentities {
    pub(crate) attachment: OperationId,
    pub(crate) request: OperationId,
    pub(crate) cleanup: OperationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SendOperationIdentities {
    pub(crate) operation: OperationId,
    pub(crate) attachment: OperationId,
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
    completion_seal: Arc<TerminalCompletionSeal>,
}

impl PendingOperation {
    pub(crate) const fn command(&self) -> &CapabilityNodeMessage {
        &self.command
    }

    pub(crate) const fn create_reference(&self) -> &RequestId {
        &self.create_id
    }

    /// Attaches earlier output the new terminal shows before its program starts. Only a launch
    /// not yet dispatched carries it; its authorization and fence are unchanged.
    pub(crate) fn set_output_seed(
        &mut self,
        seed: Vec<u8>,
    ) -> Result<(), orchbee_capability_node_protocol::ProtocolError> {
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = &self.command else {
            return Err(orchbee_capability_node_protocol::ProtocolError::InvalidPayload);
        };
        let launch = launch.clone().with_output_seed(Some(seed))?;
        self.command = CapabilityNodeMessage::AuthorizedTerminalLaunch(launch);
        Ok(())
    }

    fn authorization(&self) -> TerminalOperationAuthorization {
        match &self.command {
            CapabilityNodeMessage::AuthorizedTerminalLaunch(command) => command.authorization(),
            CapabilityNodeMessage::AuthorizedTerminalStart(command) => command.authorization(),
            // Pending terminal operations contain only Create/Start; unrelated messages cannot confirm their phase or authority.
            _ => unreachable!("only the coordinator constructs pending commands"),
        }
    }

    fn matches_phase_and_reference(&self, phase: TerminalPhase) -> bool {
        match (&self.request, &self.command, phase) {
            (
                ClientMessage::TerminalCreate(request),
                CapabilityNodeMessage::AuthorizedTerminalLaunch(_),
                TerminalPhase::Creating,
            ) => request.request_id() == &self.create_id,
            (
                ClientMessage::TerminalStart(request),
                CapabilityNodeMessage::AuthorizedTerminalStart(_),
                TerminalPhase::Starting(_),
            ) => request.terminal_create_request_id() == &self.create_id,
            // Pending terminal operations contain only Create/Start; unrelated messages cannot confirm their phase or authority.
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PlannedCreate<'coordinator> {
    coordinator: &'coordinator mut TerminalCoordinator,
    pending: Box<PendingOperation>,
    binding: TerminalBinding,
    launch_program: Option<orchbee_workbench_protocol::PresentationLaunchProgram>,
    next_identity: Option<u128>,
}

impl PlannedCreate<'_> {
    /// The file name of the program the planned launch starts, derived from the same capture the
    /// dispatched command carries; a Presentation publishes it with this binding.
    pub(crate) const fn launch_program(
        &self,
    ) -> Option<&orchbee_workbench_protocol::PresentationLaunchProgram> {
        self.launch_program.as_ref()
    }

    pub(crate) fn set_editor_source(
        &mut self,
        source: orchbee_capability_node_protocol::TerminalEditorSource,
    ) -> Result<(), orchbee_capability_node_protocol::ProtocolError> {
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = &self.pending.command else {
            unreachable!("planned creation contains a launch")
        };
        let launch = launch.clone().with_editor_source(Some(source))?;
        let command = CapabilityNodeMessage::AuthorizedTerminalLaunch(launch);
        orchbee_capability_node_protocol::encode_message(&command)?;
        self.pending.command = command;
        Ok(())
    }

    pub(crate) fn mark_popup(&mut self) -> TerminalOperationAuthorization {
        self.binding.popup_owned = true;
        self.binding.authorization
    }

    pub(crate) const fn authorization(&self) -> TerminalOperationAuthorization {
        self.binding.authorization
    }

    pub(crate) fn create_reference(&self) -> &RequestId {
        self.pending.create_reference()
    }

    pub(crate) fn command(&self) -> &CapabilityNodeMessage {
        self.pending.command()
    }

    /// Attaches the Server-side profile provenance to the binding being
    /// committed. Called only when the launch command resolved a named
    /// profile at prepare; the Worker-facing request never carries it.
    pub(crate) fn set_profile_provenance(
        &mut self,
        provenance: Option<orchbee_client_server_protocol::LaunchProfileProvenance>,
    ) {
        self.binding.profile_provenance = provenance;
    }

    pub(crate) fn commit(self) -> Box<PendingOperation> {
        let Self {
            coordinator,
            pending,
            binding,
            launch_program: _,
            next_identity,
        } = self;
        coordinator
            .bindings
            .insert(pending.create_reference().clone(), binding);
        coordinator.next_identity = next_identity;
        pending
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalCompletionTokenMismatch;

#[derive(Debug)]
pub(crate) struct PreparedTerminalCompletion<'coordinator> {
    binding: &'coordinator mut TerminalBinding,
    next_phase: TerminalPhase,
    create_outcome: Option<CreateOutcome>,
    result: ServerMessage,
}

impl PreparedTerminalCompletion<'_> {
    pub(crate) const fn result(&self) -> &ServerMessage {
        &self.result
    }

    pub(crate) fn commit(self) -> ServerMessage {
        if let Some(outcome) = self.create_outcome {
            self.binding.create_outcome = Some(outcome);
        }
        self.binding.phase = self.next_phase;
        self.result
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalPhase {
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

#[derive(Debug)]
struct TerminalCompletionSeal;

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
    popup_owned: bool,
    popup_closing: bool,
    authorization: TerminalOperationAuthorization,
    completion_seal: Arc<TerminalCompletionSeal>,
    deferred: bool,
    create_outcome: Option<CreateOutcome>,
    phase: TerminalPhase,
    /// Server-side provenance frozen at prepare when the launch named a
    /// profile; never part of the Worker-facing request and immutable once
    /// the binding commits.
    profile_provenance: Option<orchbee_client_server_protocol::LaunchProfileProvenance>,
    launch_selection: Option<orchbee_client_server_protocol::LaunchSelection>,
    /// Whether the dispatched argv runs an interactive Shell, judged when the launch was planned.
    interactive_shell: bool,
}

pub(crate) struct RetainedTerminalCreate<'coordinator> {
    pub(crate) reference: &'coordinator RequestId,
    pub(crate) outcome: Option<CreateOutcome>,
    pub(crate) phase: TerminalPhase,
    pub(crate) worker_generation: WorkerGeneration,
    pub(crate) profile_provenance: Option<orchbee_client_server_protocol::LaunchProfileProvenance>,
    pub(crate) launch_selection: Option<orchbee_client_server_protocol::LaunchSelection>,
    pub(crate) interactive_shell: bool,
}

/// Resolution of a binding identity pair against the retained creates. `Absent` is a
/// legitimate miss; `Ambiguous` and `Inconsistent` are coordinator invariant violations
/// that must never be collapsed into `Absent`.
pub(crate) enum RetainedBindingResolution<'coordinator> {
    Absent,
    Unique(RetainedTerminalCreate<'coordinator>),
    Ambiguous,
    Inconsistent,
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
    ) -> Option<BindingViewerTarget> {
        let binding = self.bindings.get(reference)?;
        if binding.authorization.worker_id() != worker.worker_id
            || binding.authorization.worker_generation() != worker.generation
        {
            return None;
        }
        match binding.phase {
            TerminalPhase::AwaitingStart(identity) | TerminalPhase::Started { identity, .. } => {
                Some(BindingViewerTarget {
                    terminal_id: binding.authorization.terminal_id(),
                    identity,
                    worker,
                })
            }
            _ => None,
        }
    }

    pub(crate) fn fence_popup(
        &mut self,
        reference: &RequestId,
        expected: TerminalOperationAuthorization,
    ) -> bool {
        let Some(binding) = self
            .bindings
            .get_mut(reference)
            .filter(|binding| binding.popup_owned && binding.authorization == expected)
        else {
            return false;
        };
        binding.popup_closing = true;
        true
    }

    pub(crate) fn target_retired(&self, target: BindingViewerTarget) -> bool {
        !self.bindings.values().any(|binding| {
            binding.authorization.terminal_id() == target.terminal_id
                && binding.authorization.worker_id() == target.worker.worker_id
                && binding.authorization.worker_generation() == target.worker.generation
                && !binding.popup_closing
                && matches!(binding.phase, TerminalPhase::AwaitingStart(identity)
                    | TerminalPhase::Started { identity, .. } if identity == target.identity)
        })
    }

    pub(crate) fn release_popup(
        &mut self,
        reference: &RequestId,
        expected: TerminalOperationAuthorization,
    ) -> bool {
        if !self
            .bindings
            .get(reference)
            .is_some_and(|binding| binding.popup_owned && binding.authorization == expected)
        {
            return false;
        }
        self.bindings.remove(reference);
        true
    }

    pub(crate) fn retained_authorization(
        &self,
        reference: &RequestId,
    ) -> Option<TerminalOperationAuthorization> {
        self.bindings
            .get(reference)
            .map(|binding| binding.authorization)
    }

    #[cfg(test)]
    pub(crate) fn has_retained_binding(&self, expected: &PresentationBinding) -> bool {
        self.bindings.values().any(|binding| {
            if binding.popup_owned {
                return false;
            }
            format!(
                "{:032x}",
                u128::from_be_bytes(binding.authorization.terminal_id().as_bytes())
            ) == expected.terminal_id.as_str()
                && format!(
                    "{:032x}",
                    u128::from_be_bytes(binding.authorization.worker_id().as_bytes())
                ) == expected.worker_id.as_str()
        })
    }

    fn resolve_retained_binding(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
    ) -> RetainedBindingResolution<'_> {
        let mut matches = self.bindings.iter().filter(|(_, binding)| {
            binding.authorization.terminal_id() == terminal_id
                && binding.authorization.worker_id() == worker_id
        });
        let Some((reference, binding)) = matches.next() else {
            return RetainedBindingResolution::Absent;
        };
        if matches.next().is_some() {
            return RetainedBindingResolution::Ambiguous;
        }
        if binding.create_outcome.is_none() != matches!(binding.phase, TerminalPhase::Creating) {
            return RetainedBindingResolution::Inconsistent;
        }
        RetainedBindingResolution::Unique(RetainedTerminalCreate {
            reference,
            outcome: binding.create_outcome,
            phase: binding.phase,
            worker_generation: binding.authorization.worker_generation(),
            profile_provenance: binding.profile_provenance.clone(),
            launch_selection: binding.launch_selection.clone(),
            interactive_shell: binding.interactive_shell,
        })
    }

    pub(crate) fn retained_create_for_binding(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
    ) -> Option<RetainedTerminalCreate<'_>> {
        match self.resolve_retained_binding(terminal_id, worker_id) {
            RetainedBindingResolution::Unique(retained) => Some(retained),
            _ => None,
        }
    }

    pub(crate) fn retained_binding_resolution(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
    ) -> RetainedBindingResolution<'_> {
        self.resolve_retained_binding(terminal_id, worker_id)
    }

    pub(crate) fn binding_viewer_target(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<BindingViewerTarget, BindingViewerTargetError> {
        let Some(worker) = current_worker else {
            return Err(BindingViewerTargetError::TerminalRuntimeUnavailable);
        };
        if worker.worker_id != worker_id {
            return Err(BindingViewerTargetError::TerminalRuntimeUnavailable);
        }
        let Some(binding) = self.bindings.values().find(|binding| {
            binding.authorization.terminal_id() == terminal_id
                && binding.authorization.worker_id() == worker_id
        }) else {
            return Err(BindingViewerTargetError::TerminalUnavailable);
        };
        if binding.authorization.worker_generation() != worker.generation {
            return Err(BindingViewerTargetError::TerminalRuntimeUnavailable);
        }
        let identity = match binding.phase {
            TerminalPhase::AwaitingStart(identity) | TerminalPhase::Started { identity, .. } => {
                identity
            }
            TerminalPhase::Creating
            | TerminalPhase::Starting(_)
            | TerminalPhase::Unknown(_)
            | TerminalPhase::Unavailable(_) => {
                return Err(BindingViewerTargetError::TerminalUnavailable);
            }
        };
        Ok(BindingViewerTarget {
            terminal_id,
            identity,
            worker,
        })
    }

    pub(crate) fn binding_send_target(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<BindingViewerTarget, BindingViewerTargetError> {
        let Some(worker) = current_worker else {
            return Err(BindingViewerTargetError::TerminalRuntimeUnavailable);
        };
        if worker.worker_id != worker_id {
            return Err(BindingViewerTargetError::TerminalRuntimeUnavailable);
        }
        let Some(binding) = self.bindings.values().find(|binding| {
            binding.authorization.terminal_id() == terminal_id
                && binding.authorization.worker_id() == worker_id
        }) else {
            return Err(BindingViewerTargetError::TerminalUnavailable);
        };
        if binding.authorization.worker_generation() != worker.generation {
            return Err(BindingViewerTargetError::TerminalRuntimeUnavailable);
        }
        let TerminalPhase::Started { identity, .. } = binding.phase else {
            return Err(BindingViewerTargetError::TerminalUnavailable);
        };
        Ok(BindingViewerTarget {
            terminal_id,
            identity,
            worker,
        })
    }

    /// A failed Create before Host commit has no runtime or admitted sessions to dispose.
    /// Unknown effects and failures of an already-created terminal cannot prove absence.
    pub(crate) fn is_definitively_uncreated(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
    ) -> bool {
        let mut matches = self.bindings.values().filter(|binding| {
            binding.authorization.terminal_id() == terminal_id
                && binding.authorization.worker_id() == worker_id
        });
        let Some(binding) = matches.next() else {
            return false;
        };
        matches.next().is_none()
            && binding.phase == TerminalPhase::Unavailable(None)
            && matches!(
                binding.create_outcome,
                Some(CreateOutcome::Failed { .. } | CreateOutcome::Unavailable { .. })
            )
    }

    pub(crate) fn retained_runtime_target(
        &self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<(BindingViewerTarget, TerminalFence), RetainedRuntimeTargetError> {
        let mut matches = self.bindings.values().filter(|binding| {
            binding.authorization.terminal_id() == terminal_id
                && binding.authorization.worker_id() == worker_id
        });
        let binding = matches
            .next()
            .ok_or(RetainedRuntimeTargetError::Unavailable)?;
        if matches.next().is_some() {
            return Err(RetainedRuntimeTargetError::Unavailable);
        }
        let identity = match binding.phase {
            TerminalPhase::Creating | TerminalPhase::Starting(_) => {
                return Err(RetainedRuntimeTargetError::Busy);
            }
            TerminalPhase::AwaitingStart(identity) | TerminalPhase::Started { identity, .. } => {
                identity
            }
            TerminalPhase::Unknown(_) | TerminalPhase::Unavailable(_) => {
                return Err(RetainedRuntimeTargetError::Unavailable);
            }
        };
        let worker = current_worker
            .filter(|worker| matches_placement(binding.authorization, *worker))
            .ok_or(RetainedRuntimeTargetError::Unavailable)?;
        Ok((
            BindingViewerTarget {
                terminal_id,
                identity,
                worker,
            },
            binding.authorization.fence(),
        ))
    }

    pub(crate) fn prepare(
        &mut self,
        start_fenced: bool,
        request: ClientMessage,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        capture: &LaunchCapture,
    ) -> PreparedOperation {
        match request {
            ClientMessage::TerminalControl(request) => PreparedOperation::Ready(ServerMessage::TerminalControlResult(orchbee_client_server_protocol::TerminalControlResult::new(request.request_id().clone(),orchbee_client_server_protocol::TerminalControlOutcome::Rejected {reason:orchbee_client_server_protocol::ControlRejectedReason::ViewerUnavailable}))),
            ClientMessage::TerminalViewer(request) => PreparedOperation::Ready(ServerMessage::TerminalViewerResult(
                orchbee_client_server_protocol::TerminalViewerResult::new(request.request_id().clone(),
                    orchbee_client_server_protocol::TerminalViewerOutcome::Rejected {
                        reason: orchbee_client_server_protocol::ViewerRejectedReason::TerminalUnavailable,
                    }))),
            ClientMessage::TerminalCreate(request) => {
                self.prepare_create(request, worker, expiry, capture)
            }
            ClientMessage::TerminalStart(request) => self.prepare_start(request, worker, expiry, start_fenced),
            // Terminal send never enters the pending-operation path; it is
            // admitted and dispatched by the service's one-shot send flow.
            ClientMessage::TerminalSend(request) => PreparedOperation::Ready(
                match orchbee_client_server_protocol::TerminalSendResult::new(
                    request.request_id().clone(),
                    None,
                    orchbee_client_server_protocol::TerminalSendOutcome::Unavailable {
                        reason: orchbee_client_server_protocol::SendUnavailableReason::ServerBusy,
                        progress: orchbee_client_server_protocol::SendProgress::none(),
                        cleanup: orchbee_client_server_protocol::SendCleanupStatus::NotNeeded,
                    },
                ) {
                    Ok(result) => ServerMessage::TerminalSendResult(result),
                    Err(_) => unreachable!("validated send outcomes are encodable"),
                },
            ),
            // Launch reads never enter the pending-operation path; the service
            // answers them from the coordinator under one session lock.
            ClientMessage::TerminalLaunchRead(request) => PreparedOperation::Ready(
                ServerMessage::TerminalLaunchReadResult(
                    orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                        request.request_id().clone(),
                        orchbee_client_server_protocol::TerminalLaunchReadOutcome::Unavailable {
                            reason:
                                orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
                        },
                    ),
                ),
            ),
        }
    }

    fn prepare_create(
        &mut self,
        request: TerminalCreateRequest,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        capture: &LaunchCapture,
    ) -> PreparedOperation {
        match self.plan_create(request, worker, expiry, capture) {
            Ok(plan) => PreparedOperation::Dispatch(plan.commit()),
            Err(result) => PreparedOperation::Ready(result),
        }
    }

    /// Plans one creation whose launch carries `capture`, read before the caller took its locks.
    pub(crate) fn plan_create(
        &mut self,
        request: TerminalCreateRequest,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        capture: &LaunchCapture,
    ) -> Result<PlannedCreate<'_>, ServerMessage> {
        let ready = |outcome| create_result(&request, outcome);
        if self.bindings.contains_key(request.request_id()) {
            return Err(ready(CreateOutcome::Rejected {
                reason: CreateRejectedReason::NotAuthorized,
            }));
        }
        let Some(worker) = worker else {
            return Err(ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::NoCompatibleWorker,
            }));
        };
        if let Some(WorkerConstraint::Exact { worker_id }) = request.worker_constraint()
            && worker_id.as_str() != hex_identity(worker.worker_id.as_bytes())
        {
            return Err(ready(CreateOutcome::Rejected {
                reason: CreateRejectedReason::WorkerConstraintUnsatisfied,
            }));
        }
        if self.bindings.len() >= MAXIMUM_TERMINAL_BINDINGS {
            return Err(ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            }));
        }
        let cwd = match request
            .cwd()
            .map(|cwd| AbsoluteCwd::new(cwd.as_str()))
            .transpose()
        {
            Ok(cwd) => cwd,
            Err(_) => {
                return Err(ready(CreateOutcome::Rejected {
                    reason: CreateRejectedReason::CwdNotAccepted,
                }));
            }
        };
        let mode = match request.start_mode() {
            StartMode::Deferred => TerminalLaunchMode::Deferred,
            StartMode::Immediate { geometry } => match worker_geometry(*geometry) {
                Some(geometry) => TerminalLaunchMode::Immediate(geometry),
                None => {
                    return Err(ready(CreateOutcome::Failed {
                        reason: CreateFailureReason::TerminalCreateFailed,
                    }));
                }
            },
        };
        // A decoded selection already passed the closed lexical contract at
        // the OBCS boundary; re-validation here is the same typed constructor
        // and can only fail on a contract divergence, which is a Server
        // invariant failure, not a client rejection.
        let launch = match request
            .launch()
            .map(|launch| {
                orchbee_capability_node_protocol::LaunchSelection::new(
                    launch.executable().to_owned(),
                    launch.arguments().to_vec(),
                )
            })
            .transpose()
        {
            Ok(launch) => launch,
            Err(_) => {
                return Err(ready(CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                }));
            }
        };
        let environment = match orchbee_capability_node_protocol::LaunchEnvironment::new(
            request.environment().entries().to_vec(),
        ) {
            Ok(environment) => environment,
            Err(_) => {
                return Err(ready(CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                }));
            }
        };
        let Some(expiry) = expiry else {
            return Err(ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            }));
        };
        let Some(identity) = self.next_identity else {
            return Err(ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            }));
        };
        let bytes = identity.to_be_bytes();
        let Some(authorization) = make_authorization(bytes, bytes, worker, NonZeroU64::MIN, expiry)
        else {
            return Err(ready(CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            }));
        };
        let completion_seal = Arc::new(TerminalCompletionSeal);
        let launch_selection = request.launch().cloned();
        let launch_program = capture.launch_program(launch.as_ref());
        let interactive_shell = capture.launches_interactive_shell(launch.as_ref());
        let preferences = capture.preferences(launch.as_ref());
        let command = AuthorizedTerminalLaunch::new(authorization, cwd, mode, launch)
            .with_environment(environment);
        let plugin_environment = match request
            .plugin_environment()
            .map(|environment| {
                orchbee_capability_node_protocol::PluginTerminalEnvironment::new(
                    environment.entries().to_vec(),
                )
            })
            .transpose()
        {
            Ok(environment) => environment,
            Err(_) => {
                return Err(ready(CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                }));
            }
        };
        let command = match command
            .with_plugin_environment(plugin_environment)
            .and_then(|command| command.with_preferences(preferences))
        {
            Ok(command) => command,
            Err(_) => {
                return Err(ready(CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                }));
            }
        };
        // Plugin metadata and the preference capture must fit one OBCN frame before admission.
        if let Err(error) = orchbee_capability_node_protocol::encode_message(
            &CapabilityNodeMessage::AuthorizedTerminalLaunch(command.clone()),
        ) {
            return Err(ready(match error {
                orchbee_capability_node_protocol::ProtocolError::PayloadTooLarge { .. } => {
                    CreateOutcome::Rejected {
                        reason: CreateRejectedReason::LaunchRequestTooLarge,
                    }
                }
                _ => CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                },
            }));
        }
        Ok(PlannedCreate {
            coordinator: self,
            pending: Box::new(PendingOperation {
                command: CapabilityNodeMessage::AuthorizedTerminalLaunch(command),
                create_id: request.request_id().clone(),
                request: ClientMessage::TerminalCreate(request),
                completion_seal: Arc::clone(&completion_seal),
            }),
            binding: TerminalBinding {
                popup_owned: false,
                popup_closing: false,
                authorization,
                completion_seal,
                deferred: matches!(mode, TerminalLaunchMode::Deferred),
                create_outcome: None,
                phase: TerminalPhase::Creating,
                profile_provenance: None,
                launch_selection,
                interactive_shell,
            },
            launch_program,
            next_identity: identity.checked_add(1),
        })
    }

    fn prepare_start(
        &mut self,
        request: TerminalStartRequest,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        target_fenced: bool,
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
        if target_fenced {
            return ready(StartOutcome::Rejected {
                reason: StartRejectedReason::TerminalCreateUnavailable,
            });
        }
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
        let completion_seal = Arc::new(TerminalCompletionSeal);
        binding.authorization = authorization;
        binding.completion_seal = Arc::clone(&completion_seal);
        binding.phase = TerminalPhase::Starting(identity);
        PreparedOperation::Dispatch(Box::new(PendingOperation {
            command: CapabilityNodeMessage::AuthorizedTerminalStart(AuthorizedTerminalStart::new(
                authorization,
                identity,
                geometry,
            )),
            create_id: request.terminal_create_request_id().clone(),
            request: ClientMessage::TerminalStart(request),
            completion_seal,
        }))
    }

    pub(crate) fn prepare_completion(
        &mut self,
        pending: Box<PendingOperation>,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<PreparedTerminalCompletion<'_>, TerminalCompletionTokenMismatch> {
        let Some(binding) = self.bindings.get_mut(&pending.create_id) else {
            return Err(TerminalCompletionTokenMismatch);
        };
        if !pending.matches_phase_and_reference(binding.phase)
            || !Arc::ptr_eq(&binding.completion_seal, &pending.completion_seal)
            || binding.authorization != pending.authorization()
            || matches!(pending.request, ClientMessage::TerminalCreate(_))
                && binding.create_outcome.is_some()
        {
            return Err(TerminalCompletionTokenMismatch);
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
        let (result, phase) = outcomes::settle(&pending, completion, binding.phase);
        let create_outcome = match &result {
            ServerMessage::TerminalCreateResult(result) => Some(result.outcome()),
            ServerMessage::TerminalControlResult(..)
            | ServerMessage::TerminalStartResult(..)
            | ServerMessage::TerminalViewerResult(..)
            | ServerMessage::TerminalSendResult(..)
            | ServerMessage::TerminalLaunchReadResult(..) => None,
        };
        Ok(PreparedTerminalCompletion {
            binding,
            next_phase: phase,
            create_outcome,
            result,
        })
    }

    pub(crate) fn allocate_identity(&mut self) -> Option<[u8; 16]> {
        let identity = self.next_identity?;
        self.next_identity = identity.checked_add(1);
        Some(identity.to_be_bytes())
    }

    /// Reserves `count` sequential identities and returns the block base, so a
    /// caller can derive fresh IDs without re-entering the coordinator lock.
    pub(crate) fn allocate_identity_block(&mut self, count: u128) -> Option<u128> {
        let base = self.next_identity?;
        self.next_identity = Some(base.checked_add(count)?);
        Some(base)
    }

    pub(crate) fn allocate_viewer_identities(&mut self) -> Option<ViewerOperationIdentities> {
        let attachment = self.next_identity?;
        let request = attachment.checked_add(1)?;
        let cleanup = request.checked_add(1)?;
        let next_identity = cleanup.checked_add(1);
        let identities = (
            OperationId::from_bytes(attachment.to_be_bytes()),
            OperationId::from_bytes(request.to_be_bytes()),
            OperationId::from_bytes(cleanup.to_be_bytes()),
        );
        let (Ok(attachment), Ok(request), Ok(cleanup)) = identities else {
            return None;
        };
        self.next_identity = next_identity;
        Some(ViewerOperationIdentities {
            attachment,
            request,
            cleanup,
        })
    }

    pub(crate) fn allocate_send_identities(&mut self) -> Option<SendOperationIdentities> {
        let operation = self.next_identity?;
        let attachment = operation.checked_add(1)?;
        let next_identity = attachment.checked_add(1);
        let (Ok(operation), Ok(attachment)) = (
            OperationId::from_bytes(operation.to_be_bytes()),
            OperationId::from_bytes(attachment.to_be_bytes()),
        ) else {
            return None;
        };
        self.next_identity = next_identity;
        Some(SendOperationIdentities {
            operation,
            attachment,
        })
    }

    #[cfg(test)]
    pub(crate) fn set_next_identity_for_test(&mut self, next_identity: Option<u128>) {
        self.next_identity = next_identity;
    }

    #[cfg(test)]
    pub(crate) fn set_phase_for_test(&mut self, reference: &RequestId, phase: TerminalPhase) {
        self.bindings.get_mut(reference).unwrap().phase = phase;
    }

    #[cfg(test)]
    pub(crate) fn replace_authorization_for_test(
        &mut self,
        reference: &RequestId,
        authorization: TerminalOperationAuthorization,
    ) {
        self.bindings.get_mut(reference).unwrap().authorization = authorization;
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
        ClientMessage::TerminalSend(request) => ServerMessage::TerminalSendResult(
            match orchbee_client_server_protocol::TerminalSendResult::new(
                request.request_id().clone(),
                None,
                orchbee_client_server_protocol::TerminalSendOutcome::OutcomeUnknown {
                    progress: orchbee_client_server_protocol::SendProgress::none(),
                    cleanup: orchbee_client_server_protocol::SendCleanupStatus::Unconfirmed,
                },
            )
            .or_else(|_| {
                orchbee_client_server_protocol::TerminalSendResult::new(
                    request.request_id().clone(),
                    None,
                    orchbee_client_server_protocol::TerminalSendOutcome::Unavailable {
                        reason: orchbee_client_server_protocol::SendUnavailableReason::ServerBusy,
                        progress: orchbee_client_server_protocol::SendProgress::none(),
                        cleanup: orchbee_client_server_protocol::SendCleanupStatus::NotNeeded,
                    },
                )
            }) {
                Ok(result) => result,
                Err(_) => unreachable!("validated send outcomes are encodable"),
            },
        ),
        ClientMessage::TerminalLaunchRead(request) => ServerMessage::TerminalLaunchReadResult(
            orchbee_client_server_protocol::TerminalLaunchReadResult::new(
                request.request_id().clone(),
                orchbee_client_server_protocol::TerminalLaunchReadOutcome::Unavailable {
                    reason:
                        orchbee_client_server_protocol::TerminalLaunchReadUnavailableReason::ServerBusy,
                },
            ),
        ),
    }
}

#[cfg(test)]
mod tests;
