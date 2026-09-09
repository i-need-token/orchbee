use crate::terminal_coordinator::{
    Completion, PendingOperation, PreparedOperation, TerminalCompletionTokenMismatch,
    TerminalCoordinator, WorkerPlacement,
};
use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, TerminalOperationAuthorization,
};
use orchbee_client_server_protocol::{
    ClientMessage, CreateAcceptedState, CreateFailureReason, CreateOutcome,
    CreateUnavailableReason, OpenOutcome, RequestId, ServerMessage,
};
use orchbee_server::{
    WorkbenchAuthority, WorkbenchGrant, WorkbenchReadOrNavigationRequest,
    WorkbenchTerminalMembership,
};
use orchbee_workbench_protocol::{PresentationBinding, TerminalId, WorkbenchResponse, WorkerId};

mod standalone;
pub(crate) use standalone::PreparedOpen;
mod tab_close;
mod tab_create;
#[cfg(test)]
pub(crate) use tab_create::PendingTerminalTabCreate;
pub(crate) use tab_create::{PostcommitCoordinationError, PreparedTerminalTabCreate};
mod panel_split;
pub(crate) use panel_split::PreparedTerminalPanelSplit;
mod terminal_selection;
mod workspace_directory;
pub(crate) use terminal_selection::{BindingViewerPreparationError, PreparedBindingViewer};
mod target_lifecycle;
pub(crate) use target_lifecycle::TargetLifecycle;

pub(crate) struct SiteCoordinator {
    terminals: TerminalCoordinator,
    workbench: Option<WorkbenchAuthority>,
    standalone: StandaloneTerminal,
}

#[derive(Debug, Eq, PartialEq)]
enum StandaloneTerminal {
    Empty,
    Creating(RequestId),
    Settled {
        reference: RequestId,
        outcome: OpenOutcome,
    },
}

#[cfg(test)]
mod tests;

impl SiteCoordinator {
    pub(crate) const fn new(workbench: Option<WorkbenchAuthority>) -> Self {
        Self {
            terminals: TerminalCoordinator::new(),
            workbench,
            standalone: StandaloneTerminal::Empty,
        }
    }

    pub(crate) fn workbench_site_matches(
        &self,
        site: &orchbee_workbench_protocol::SiteId,
    ) -> Option<bool> {
        Some(self.workbench.as_ref()?.site_id() == site)
    }

    pub(crate) fn execute_workbench(
        &mut self,
        request: WorkbenchReadOrNavigationRequest,
        manage_navigation: bool,
    ) -> Option<WorkbenchResponse> {
        let authority = self.workbench.as_mut()?;
        let site = authority.site_id().clone();
        let grant = if manage_navigation {
            WorkbenchGrant::manage_navigation(site)
        } else {
            WorkbenchGrant::read_only(site)
        };
        Some(authority.execute(Some(&grant), request))
    }

    pub(crate) fn prepare(
        &mut self,
        request: ClientMessage,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> PreparedOperation {
        let start_fenced = match &request {
            ClientMessage::TerminalStart(request) => self
                .terminals
                .retained_authorization(request.terminal_create_request_id())
                .and_then(presentation_binding)
                .is_some_and(|binding| self.binding_has_close(&binding)),
            _ => false,
        };
        self.terminals
            .prepare(start_fenced, request, worker, expiry)
    }

    pub(crate) fn execute_terminal_tab_move(
        &mut self,
        command: orchbee_workbench_protocol::WorkbenchCommand,
    ) -> Option<orchbee_workbench_protocol::WorkbenchCommandResult> {
        use orchbee_server::{WorkbenchTabMoveBegin, WorkbenchTabMovePreparation};
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.site_id().clone());
        let admission = match workbench.begin_terminal_tab_move(Some(&grant), command) {
            WorkbenchTabMoveBegin::Ready(result) => return Some(result),
            WorkbenchTabMoveBegin::Admitted(admission) => admission,
        };
        if admission
            .standalone_membership()
            .is_some_and(|membership| !composition_matches(membership, standalone, terminals))
        {
            return Some(admission.retire_composition_failure());
        }
        Some(match admission.finish() {
            WorkbenchTabMovePreparation::Ready(result) => result,
            WorkbenchTabMovePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn complete(
        &mut self,
        pending: Box<PendingOperation>,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<ServerMessage, TerminalCompletionTokenMismatch> {
        let reference = pending.create_reference().clone();
        let Self {
            terminals,
            standalone,
            workbench: _,
        } = self;
        let prepared = terminals.prepare_completion(pending, completion, current_worker)?;
        let result = prepared.result().clone();
        let standalone_settlement = if matches!(standalone, StandaloneTerminal::Creating(current) if current == &reference)
        {
            let outcome = open_outcome(&reference, prepared.result());
            Some(StandaloneTerminal::Settled { reference, outcome })
        } else {
            None
        };
        prepared.commit();
        if let Some(settled) = standalone_settlement {
            *standalone = settled;
        }
        Ok(result)
    }

    pub(crate) fn viewer_target(
        &self,
        reference: &RequestId,
        worker: WorkerPlacement,
    ) -> Option<crate::terminal_coordinator::BindingViewerTarget> {
        let target = self.terminals.viewer_target(reference, worker)?;
        (self.target_lifecycle(target) == TargetLifecycle::Unfenced).then_some(target)
    }

    pub(crate) fn allocate_identity(&mut self) -> Option<[u8; 16]> {
        self.terminals.allocate_identity()
    }
}

fn composition_matches(
    membership: &WorkbenchTerminalMembership,
    standalone: &StandaloneTerminal,
    terminals: &TerminalCoordinator,
) -> bool {
    if !membership.is_standalone() {
        return false;
    }
    match standalone {
        StandaloneTerminal::Empty => membership.is_empty(),
        StandaloneTerminal::Creating(reference) | StandaloneTerminal::Settled { reference, .. } => {
            let Some(binding) = terminals
                .retained_authorization(reference)
                .and_then(presentation_binding)
            else {
                return false;
            };
            membership
                .members()
                .iter()
                .filter(|member| member.binding() == &binding)
                .count()
                == 1
        }
    }
}

fn tree_composition_matches(
    membership: &WorkbenchTerminalMembership,
    standalone: &StandaloneTerminal,
    terminals: &TerminalCoordinator,
) -> bool {
    if !membership.is_standalone() {
        return false;
    }
    match standalone {
        StandaloneTerminal::Empty => membership.is_empty(),
        StandaloneTerminal::Creating(_) | StandaloneTerminal::Settled { .. } => {
            !membership.is_empty()
                && membership
                    .members()
                    .iter()
                    .all(|member| terminals.has_retained_binding(member.binding()))
        }
    }
}

fn presentation_binding(
    authorization: TerminalOperationAuthorization,
) -> Option<PresentationBinding> {
    Some(PresentationBinding {
        terminal_id: TerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(authorization.terminal_id().as_bytes())
        ))
        .ok()?,
        worker_id: WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(authorization.worker_id().as_bytes())
        ))
        .ok()?,
    })
}

fn open_outcome(reference: &RequestId, result: &ServerMessage) -> OpenOutcome {
    let ServerMessage::TerminalCreateResult(result) = result else {
        return OpenOutcome::OutcomeUnknown {};
    };
    open_create_outcome(reference, result.outcome())
}

fn open_create_outcome(reference: &RequestId, outcome: CreateOutcome) -> OpenOutcome {
    match outcome {
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { .. },
        } => OpenOutcome::Opened {
            terminal_create_request_id: reference.clone(),
        },
        CreateOutcome::Unavailable { reason } => unavailable(reason),
        CreateOutcome::Failed { reason } => OpenOutcome::Failed { reason },
        CreateOutcome::Rejected { .. } => OpenOutcome::Failed {
            reason: CreateFailureReason::TerminalCreateFailed,
        },
        CreateOutcome::Accepted {
            state: CreateAcceptedState::AwaitingStart,
        }
        | CreateOutcome::OutcomeUnknown => OpenOutcome::OutcomeUnknown {},
    }
}

fn unavailable(reason: CreateUnavailableReason) -> OpenOutcome {
    OpenOutcome::Unavailable { reason }
}
