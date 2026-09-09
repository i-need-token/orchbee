use super::tab_create::{
    client_geometry, pending_authorization, pending_authorization_from_message,
    pending_cwd_matches, placement_matches_worker, terminal_outcome, terminal_preflight_reason,
    unavailable_by_id,
};
use super::{
    PostcommitCoordinationError, SiteCoordinator, presentation_binding, tree_composition_matches,
};
use crate::terminal_coordinator::{Completion, PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::{CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds};
use orchbee_client_server_protocol::{
    RequestId, StartMode, TerminalCreateRequest, WorkerConstraint, WorkingDirectory,
};
use orchbee_server::{
    PendingWorkbenchPanelSplit, WorkbenchGrant, WorkbenchPanelSplitBegin,
    WorkbenchPanelSplitPreparation, WorkbenchPanelSplitSettlementTokenMismatch,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason,
};

pub(crate) enum PreparedTerminalPanelSplit {
    Ready(WorkbenchCommandResult),
    Dispatch(PendingTerminalPanelSplit),
}

pub(crate) struct PendingTerminalPanelSplit {
    workbench: PendingWorkbenchPanelSplit,
    terminal: Box<PendingOperation>,
    reference: RequestId,
    worker: WorkerPlacement,
    workspace_cwd: Option<WorkingDirectory>,
}

impl PendingTerminalPanelSplit {
    pub(crate) fn command(&self) -> &CapabilityNodeMessage {
        self.terminal.command()
    }

    fn association_matches(&self) -> bool {
        let metadata = self.workbench.metadata();
        let authorization = pending_authorization(&self.terminal);
        self.terminal.create_reference() == &self.reference
            && authorization.and_then(presentation_binding).as_ref() == Some(metadata.binding())
            && pending_cwd_matches(self.terminal.command(), self.workspace_cwd.as_ref())
            && authorization.is_some_and(|authorization| {
                authorization.worker_id() == self.worker.worker_id
                    && authorization.worker_generation() == self.worker.generation
            })
    }
}

impl SiteCoordinator {
    pub(crate) fn prepare_terminal_panel_split(
        &mut self,
        command: WorkbenchCommand,
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        worker: Option<WorkerPlacement>,
    ) -> PreparedTerminalPanelSplit {
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return PreparedTerminalPanelSplit::Ready(unavailable(
                &command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        };
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(workbench.site_id().clone());
        let admission = match workbench.begin_terminal_panel_split(Some(&grant), command) {
            WorkbenchPanelSplitBegin::Ready(result) => {
                return PreparedTerminalPanelSplit::Ready(result);
            }
            WorkbenchPanelSplitBegin::Admitted(admission) => admission,
        };
        if admission
            .staged_metadata()
            .standalone_membership()
            .is_some_and(|membership| !tree_composition_matches(membership, standalone, terminals))
        {
            return PreparedTerminalPanelSplit::Ready(admission.retire_composition_failure());
        }
        let (worker_constraint, workspace_cwd) = if let Some(membership) =
            admission.staged_metadata().standalone_membership()
        {
            let _ = membership;
            (None, None)
        } else {
            let Some((worker_id, path)) = admission.staged_metadata().workspace_launch() else {
                return PreparedTerminalPanelSplit::Ready(admission.retire_composition_failure());
            };
            let Ok(worker_id) = orchbee_client_server_protocol::WorkerId::parse(worker_id.as_str())
            else {
                return PreparedTerminalPanelSplit::Ready(admission.retire_composition_failure());
            };
            let Ok(cwd) = WorkingDirectory::parse(path.as_str()) else {
                return PreparedTerminalPanelSplit::Ready(admission.retire_composition_failure());
            };
            (Some(WorkerConstraint::Exact { worker_id }), Some(cwd))
        };
        let Some(worker) = worker else {
            return PreparedTerminalPanelSplit::Ready(
                admission
                    .runtime_unavailable(WorkbenchCommandUnavailableReason::NoCompatibleWorker),
            );
        };
        if let Some(WorkerConstraint::Exact { worker_id }) = &worker_constraint
            && !placement_matches_worker(worker, worker_id)
        {
            return PreparedTerminalPanelSplit::Ready(
                admission
                    .runtime_unavailable(WorkbenchCommandUnavailableReason::NoCompatibleWorker),
            );
        }
        let (Some(reference), Some(expiry)) = (candidate, expiry) else {
            return PreparedTerminalPanelSplit::Ready(
                admission.runtime_unavailable(WorkbenchCommandUnavailableReason::ServerBusy),
            );
        };
        let Some(geometry) = client_geometry(admission.staged_metadata().geometry()) else {
            return PreparedTerminalPanelSplit::Ready(
                admission.runtime_unavailable(WorkbenchCommandUnavailableReason::ServerBusy),
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
                return PreparedTerminalPanelSplit::Ready(
                    admission.runtime_unavailable(reason.into()),
                );
            }
        };
        let authorization = terminal.authorization();
        let Some(binding) = presentation_binding(authorization) else {
            drop(terminal);
            return PreparedTerminalPanelSplit::Ready(
                admission.runtime_unavailable(WorkbenchCommandUnavailableReason::ServerBusy),
            );
        };
        let workbench_plan = match admission.finish(binding.clone()) {
            WorkbenchPanelSplitPreparation::Ready(result) => {
                return PreparedTerminalPanelSplit::Ready(result);
            }
            WorkbenchPanelSplitPreparation::Prepared(plan) => plan,
        };
        let metadata = workbench_plan.metadata().clone();
        let pending_workbench = workbench_plan.commit_into_pending();
        if terminal.create_reference() != &reference
            || terminal.authorization() != authorization
            || pending_authorization_from_message(terminal.command()) != Some(authorization)
            || !pending_cwd_matches(terminal.command(), workspace_cwd.as_ref())
            || metadata.binding() != &binding
            || authorization.worker_id() != worker.worker_id
            || authorization.worker_generation() != worker.generation
            || metadata.is_standalone() != workspace_cwd.is_none()
        {
            drop(terminal);
            workbench.retire_standalone_composition();
            return PreparedTerminalPanelSplit::Ready(unavailable_by_id(
                metadata.request_id().clone(),
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        }
        let terminal = terminal.commit();
        PreparedTerminalPanelSplit::Dispatch(PendingTerminalPanelSplit {
            workbench: pending_workbench,
            terminal,
            reference,
            worker,
            workspace_cwd,
        })
    }

    pub(crate) fn complete_terminal_panel_split(
        &mut self,
        pending: PendingTerminalPanelSplit,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
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
                .tree_standalone_membership()
                .map_err(|_error| PostcommitCoordinationError)?;
            if !tree_composition_matches(&membership, standalone, terminals) {
                return Err(PostcommitCoordinationError);
            }
        }
        let PendingTerminalPanelSplit {
            workbench: workbench_pending,
            terminal,
            reference,
            worker: _,
            workspace_cwd: _,
        } = pending;
        let terminal = terminals
            .prepare_completion(terminal, completion, current_worker)
            .map_err(|_mismatch| PostcommitCoordinationError)?;
        let terminal_outcome = terminal_outcome(&reference, metadata.geometry(), terminal.result())
            .ok_or(PostcommitCoordinationError)?;
        let workbench = workbench
            .prepare_terminal_panel_split_settlement(workbench_pending, terminal_outcome)
            .map_err(|_mismatch: WorkbenchPanelSplitSettlementTokenMismatch| {
                PostcommitCoordinationError
            })?;
        terminal.commit();
        Ok(workbench.commit())
    }
}

fn unavailable(
    command: &WorkbenchCommand,
    reason: WorkbenchCommandUnavailableReason,
) -> WorkbenchCommandResult {
    WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        outcome: WorkbenchCommandOutcome::Unavailable { reason },
    }
}
