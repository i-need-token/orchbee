use super::{
    SiteCoordinator, StandaloneTerminal, composition_matches, open_outcome, presentation_binding,
};
use crate::terminal_coordinator::{Completion, PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, TerminalOperationAuthorization,
};
use orchbee_client_server_protocol::{
    CreateAcceptedState, CreateOutcome, CreateUnavailableReason, Geometry, RequestId,
    ServerMessage, StartMode, TerminalCreateRequest, WorkerConstraint, WorkingDirectory,
};
use orchbee_server::{
    PendingWorkbenchTabCreate, WorkbenchGrant, WorkbenchTabCreateBegin,
    WorkbenchTabCreatePreparation, WorkbenchTabCreateRuntimeUnavailableReason,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
};

pub(crate) enum PreparedTerminalTabCreate {
    Ready(WorkbenchCommandResult),
    Dispatch(PendingTerminalTabCreate),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompletedTerminalTabCreate {
    pub(crate) result: WorkbenchCommandResult,
    pub(crate) standalone_changed: bool,
}

pub(crate) struct PendingTerminalTabCreate {
    workbench: PendingWorkbenchTabCreate,
    terminal: Box<PendingOperation>,
    reference: RequestId,
    worker: WorkerPlacement,
    settles_first: bool,
    workspace_cwd: Option<WorkingDirectory>,
}

impl PendingTerminalTabCreate {
    pub(crate) fn command(&self) -> &CapabilityNodeMessage {
        self.terminal.command()
    }

    #[cfg(test)]
    pub(crate) const fn settles_first(&self) -> bool {
        self.settles_first
    }

    fn association_matches(&self) -> bool {
        let metadata = self.workbench.metadata();
        let authorization = pending_authorization(&self.terminal);
        metadata.is_standalone() == self.workspace_cwd.is_none()
            && metadata.is_first() == self.settles_first
            && self.terminal.create_reference() == &self.reference
            && authorization.and_then(presentation_binding).as_ref() == Some(metadata.binding())
            && pending_cwd_matches(self.terminal.command(), self.workspace_cwd.as_ref())
            && authorization.is_some_and(|authorization| {
                authorization.worker_id() == self.worker.worker_id
                    && authorization.worker_generation() == self.worker.generation
            })
    }

    #[cfg(test)]
    pub(crate) fn swap_workbench_token_for_test(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.workbench, &mut other.workbench);
    }

    #[cfg(test)]
    pub(crate) fn replace_reference_for_test(&mut self, reference: RequestId) {
        self.reference = reference;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PostcommitCoordinationError;

impl std::fmt::Display for PostcommitCoordinationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("committed terminal tab coordination failed")
    }
}

impl std::error::Error for PostcommitCoordinationError {}

impl SiteCoordinator {
    pub(crate) fn prepare_terminal_tab_create(
        &mut self,
        command: WorkbenchCommand,
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        worker: Option<WorkerPlacement>,
    ) -> PreparedTerminalTabCreate {
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return PreparedTerminalTabCreate::Ready(unavailable(
                &command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        };
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(workbench.site_id().clone());
        let admission = match workbench.begin_terminal_tab_create(Some(&grant), command) {
            WorkbenchTabCreateBegin::Ready(result) => {
                return PreparedTerminalTabCreate::Ready(result);
            }
            WorkbenchTabCreateBegin::Admitted(admission) => admission,
        };
        let (worker_constraint, workspace_cwd, settles_first) = if let Some(membership) =
            admission.staged_metadata().standalone_membership()
        {
            if !composition_matches(membership, standalone, terminals) {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            }
            (None, None, membership.is_empty())
        } else {
            let Some((worker_id, path)) = admission.staged_metadata().workspace_launch() else {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            };
            let Ok(worker_id) = orchbee_client_server_protocol::WorkerId::parse(worker_id.as_str())
            else {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            };
            let Ok(cwd) = WorkingDirectory::parse(path.as_str()) else {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            };
            (
                Some(WorkerConstraint::Exact { worker_id }),
                Some(cwd),
                false,
            )
        };
        let Some(worker) = worker else {
            return PreparedTerminalTabCreate::Ready(admission.runtime_unavailable(
                WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker,
            ));
        };
        if let Some(WorkerConstraint::Exact { worker_id }) = &worker_constraint
            && !placement_matches_worker(worker, worker_id)
        {
            return PreparedTerminalTabCreate::Ready(admission.runtime_unavailable(
                WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker,
            ));
        }
        let (Some(reference), Some(expiry)) = (candidate, expiry) else {
            return PreparedTerminalTabCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy),
            );
        };
        let Some(geometry) = client_geometry(admission.staged_metadata().geometry()) else {
            return PreparedTerminalTabCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy),
            );
        };
        let terminal = match terminals.plan_create(
            TerminalCreateRequest::new(
                reference.clone(),
                worker_constraint,
                workspace_cwd.clone(),
                StartMode::Immediate { geometry },
            ),
            Some(worker),
            Some(expiry),
        ) {
            Ok(plan) => plan,
            Err(result) => {
                let reason = terminal_preflight_reason(&result);
                return PreparedTerminalTabCreate::Ready(admission.runtime_unavailable(reason));
            }
        };
        let authorization = terminal.authorization();
        let Some(binding) = presentation_binding(authorization) else {
            drop(terminal);
            return PreparedTerminalTabCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy),
            );
        };
        let workbench_plan = match admission.finish(binding.clone()) {
            WorkbenchTabCreatePreparation::Ready(result) => {
                return PreparedTerminalTabCreate::Ready(result);
            }
            WorkbenchTabCreatePreparation::Prepared(plan) => plan,
        };
        let metadata = workbench_plan.metadata().clone();
        let standalone_matches = if metadata.is_standalone() {
            settles_first == matches!(standalone, StandaloneTerminal::Empty)
        } else {
            !settles_first
        };
        if terminal.create_reference() != &reference
            || terminal.authorization() != authorization
            || pending_authorization_from_message(terminal.command()) != Some(authorization)
            || !pending_cwd_matches(terminal.command(), workspace_cwd.as_ref())
            || metadata.binding() != &binding
            || authorization.worker_id() != worker.worker_id
            || authorization.worker_generation() != worker.generation
            || metadata.is_standalone() != workspace_cwd.is_none()
            || metadata.is_first() != settles_first
            || !standalone_matches
        {
            drop(workbench_plan);
            drop(terminal);
            workbench.retire_standalone_composition();
            return PreparedTerminalTabCreate::Ready(unavailable_by_id(
                metadata.request_id().clone(),
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        }

        // Both borrowed plans are complete. Publication below contains only infallible owner moves.
        let terminal = terminal.commit();
        let workbench = workbench_plan.commit().into_pending();
        if settles_first {
            *standalone = StandaloneTerminal::Creating(reference.clone());
        }
        PreparedTerminalTabCreate::Dispatch(PendingTerminalTabCreate {
            workbench,
            terminal,
            reference,
            worker,
            settles_first,
            workspace_cwd,
        })
    }

    pub(crate) fn complete_terminal_tab_create(
        &mut self,
        pending: PendingTerminalTabCreate,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<CompletedTerminalTabCreate, PostcommitCoordinationError> {
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return Err(PostcommitCoordinationError);
        };
        if !pending.association_matches() {
            return Err(PostcommitCoordinationError);
        }
        let metadata = pending.workbench.metadata().clone();
        if metadata.is_standalone() {
            let membership = workbench
                .standalone_membership()
                .map_err(|_invariant| PostcommitCoordinationError)?;
            if !composition_matches(&membership, standalone, terminals) {
                return Err(PostcommitCoordinationError);
            }
        }
        let PendingTerminalTabCreate {
            workbench: workbench_pending,
            terminal,
            reference,
            worker: _,
            settles_first: _,
            workspace_cwd: _,
        } = pending;
        let terminal = terminals
            .prepare_completion(terminal, completion, current_worker)
            .map_err(|_mismatch| PostcommitCoordinationError)?;
        let terminal_outcome = terminal_outcome(&reference, metadata.geometry(), terminal.result())
            .ok_or(PostcommitCoordinationError)?;
        let standalone_changed = metadata.is_standalone()
            && matches!(standalone, StandaloneTerminal::Creating(current) if current == &reference);
        let settled_open = standalone_changed.then(|| open_outcome(&reference, terminal.result()));
        let workbench = workbench
            .prepare_terminal_tab_settlement(workbench_pending, terminal_outcome)
            .map_err(|_mismatch| PostcommitCoordinationError)?;

        terminal.commit();
        let result = workbench.commit();
        if let Some(outcome) = settled_open {
            *standalone = StandaloneTerminal::Settled { reference, outcome };
        }
        Ok(CompletedTerminalTabCreate {
            result,
            standalone_changed,
        })
    }
}

fn pending_cwd_matches(
    message: &CapabilityNodeMessage,
    expected: Option<&WorkingDirectory>,
) -> bool {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(command) = message else {
        return false;
    };
    command.cwd().map(|cwd| cwd.as_str()) == expected.map(WorkingDirectory::as_str)
}

fn placement_matches_worker(
    placement: WorkerPlacement,
    expected: &orchbee_client_server_protocol::WorkerId,
) -> bool {
    expected.as_str()
        == format!(
            "{:032x}",
            u128::from_be_bytes(placement.worker_id.as_bytes())
        )
}

fn pending_authorization(pending: &PendingOperation) -> Option<TerminalOperationAuthorization> {
    pending_authorization_from_message(pending.command())
}

fn pending_authorization_from_message(
    message: &CapabilityNodeMessage,
) -> Option<TerminalOperationAuthorization> {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(command) = message else {
        return None;
    };
    Some(command.authorization())
}

fn terminal_preflight_reason(result: &ServerMessage) -> WorkbenchTabCreateRuntimeUnavailableReason {
    if matches!(
        result,
        ServerMessage::TerminalCreateResult(result)
            if result.outcome() == CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::NoCompatibleWorker,
            }
    ) {
        WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker
    } else {
        WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy
    }
}

fn terminal_outcome(
    reference: &RequestId,
    expected_geometry: WorkbenchTerminalGeometry,
    result: &ServerMessage,
) -> Option<WorkbenchTerminalOutcome> {
    let ServerMessage::TerminalCreateResult(result) = result else {
        return None;
    };
    if result.request_id() != reference {
        return None;
    }
    Some(match result.outcome() {
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { geometry },
        } => {
            let geometry = workbench_geometry(geometry)?;
            if geometry != expected_geometry {
                return None;
            }
            WorkbenchTerminalOutcome::Started { geometry }
        }
        CreateOutcome::Unavailable { reason } => WorkbenchTerminalOutcome::Unavailable {
            reason: match reason {
                CreateUnavailableReason::ServerBusy => {
                    WorkbenchTerminalUnavailableReason::ServerBusy
                }
                CreateUnavailableReason::NoCompatibleWorker
                | CreateUnavailableReason::TerminalRuntimeUnavailable => {
                    WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable
                }
            },
        },
        CreateOutcome::Failed { .. } => WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        },
        CreateOutcome::OutcomeUnknown => WorkbenchTerminalOutcome::OutcomeUnknown {},
        CreateOutcome::Accepted {
            state: CreateAcceptedState::AwaitingStart,
        }
        | CreateOutcome::Rejected { .. } => return None,
    })
}

fn client_geometry(geometry: WorkbenchTerminalGeometry) -> Option<Geometry> {
    Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .ok()
}

fn workbench_geometry(geometry: Geometry) -> Option<WorkbenchTerminalGeometry> {
    WorkbenchTerminalGeometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .ok()
}

fn unavailable(
    command: &WorkbenchCommand,
    reason: WorkbenchCommandUnavailableReason,
) -> WorkbenchCommandResult {
    unavailable_by_id(command.request_id.clone(), reason)
}

fn unavailable_by_id(
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    reason: WorkbenchCommandUnavailableReason,
) -> WorkbenchCommandResult {
    WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id,
        outcome: WorkbenchCommandOutcome::Unavailable { reason },
    }
}
