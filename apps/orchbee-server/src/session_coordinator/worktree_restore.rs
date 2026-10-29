//! Prepares and settles replacement shells for retained worktree recovery plans using exact
//! Workspace members and Worker placement. It does not run Git, replay an uncertain launch, or
//! perform transport I/O.

use super::{SessionCoordinator, presentation_binding, tab_create};
use crate::terminal_coordinator::{Completion, LaunchCapture, PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, WorkerGeneration,
};
use orchbee_client_server_protocol::{
    RequestId, StartMode, TerminalCreateRequest, WorkerConstraint, WorkingDirectory,
};
use orchbee_server::{
    WorkbenchTabCreateRuntimeUnavailableReason, WorkbenchTerminalTab, WorktreeOperationPlan,
};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorktreeOperationFailure as Failure, WorktreeOperationStatus,
};

pub(crate) struct PendingWorktreeShell {
    terminal: Box<PendingOperation>,
    operation: WorkbenchRequestId,
    attempt: u32,
    reference: RequestId,
    geometry: WorkbenchTerminalGeometry,
}
impl PendingWorktreeShell {
    pub(crate) fn command(&self) -> &CapabilityNodeMessage {
        self.terminal.command()
    }
}
impl SessionCoordinator {
    #[expect(
        clippy::too_many_arguments,
        reason = "the recovery plan, its owning close, the member and the admitting Worker's \
                  reservation and capture are independent inputs from separate owners"
    )]
    pub(crate) fn prepare_worktree_shell(
        &mut self,
        plan: &mut WorktreeOperationPlan,
        close: &WorkbenchRequestId,
        member: &WorkbenchTerminalTab,
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        worker: Option<WorkerPlacement>,
        capture: &LaunchCapture,
    ) -> Result<PendingWorktreeShell, Failure> {
        if plan.progress.restoration.is_none() {
            return Err(Failure::Internal {});
        }
        let worker = worker.ok_or(Failure::WorkerUnavailable {})?;
        let worker_id =
            orchbee_client_server_protocol::WorkerId::parse(plan.source.worker_id.as_str())
                .map_err(|_| Failure::Internal {})?;
        if !tab_create::placement_matches_worker(worker, &worker_id)
            || worker.generation != WorkerGeneration::new(plan.worker_generation)
        {
            return Err(Failure::WorkerUnavailable {});
        }
        let reference = candidate.ok_or(Failure::Internal {})?;
        let expiry = expiry.ok_or(Failure::Internal {})?;
        let geometry =
            WorkbenchTerminalGeometry::new(80, 24, 8, 16).map_err(|_| Failure::Internal {})?;
        let cwd = WorkingDirectory::parse(
            plan.progress
                .checkout
                .as_ref()
                .ok_or(Failure::Internal {})?
                .path
                .as_str(),
        )
        .map_err(|_| Failure::Internal {})?;
        let workbench = self.workbench.as_mut().ok_or(Failure::Internal {})?;
        let terminal = self
            .terminals
            .plan_create(
                TerminalCreateRequest::new(
                    reference.clone(),
                    Some(WorkerConstraint::Exact { worker_id }),
                    Some(cwd),
                    StartMode::Immediate {
                        geometry: tab_create::client_geometry(geometry)
                            .ok_or(Failure::Internal {})?,
                    },
                    None,
                ),
                Some(worker),
                Some(expiry),
                capture,
            )
            .map_err(
                |message| match tab_create::terminal_preflight_reason(&message) {
                    WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker => {
                        Failure::WorkerUnavailable {}
                    }
                    _ => Failure::WorkerBusy {},
                },
            )?;
        let binding = presentation_binding(terminal.authorization()).ok_or(Failure::Internal {})?;
        let rebind = workbench
            .prepare_worktree_rebind(
                plan,
                close,
                member,
                binding,
                terminal.launch_program().cloned(),
            )
            .map_err(|_| Failure::AssociationFailed {})?;
        // Both owners are completely prepared; neither commit performs fallible work.
        let terminal = terminal.commit();
        plan.progress = rebind.commit();
        Ok(PendingWorktreeShell {
            terminal,
            operation: plan.operation_id.clone(),
            attempt: plan.attempt,
            reference,
            geometry,
        })
    }

    pub(crate) fn complete_worktree_shell(
        &mut self,
        plan: &mut WorktreeOperationPlan,
        pending: PendingWorktreeShell,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<(), Failure> {
        if pending.operation != plan.operation_id || pending.attempt != plan.attempt {
            return Err(Failure::Internal {});
        }
        let workbench = self.workbench.as_mut().ok_or(Failure::Internal {})?;
        let terminal = self
            .terminals
            .prepare_completion(pending.terminal, completion, current_worker)
            .map_err(|_| Failure::Internal {})?;
        let outcome =
            tab_create::terminal_outcome(&pending.reference, pending.geometry, terminal.result())
                .ok_or(Failure::Internal {})?;
        let mut progress = plan.progress.clone();
        let receipt = progress.restoration.as_mut().ok_or(Failure::Internal {})?;
        if receipt.attempted != receipt.started + receipt.failed + receipt.unknown + 1 {
            return Err(Failure::Internal {});
        }
        match outcome {
            WorkbenchTerminalOutcome::Started { .. } => receipt.started += 1,
            WorkbenchTerminalOutcome::Failed { .. }
            | WorkbenchTerminalOutcome::Unavailable { .. } => receipt.failed += 1,
            WorkbenchTerminalOutcome::OutcomeUnknown {} => receipt.unknown += 1,
        }
        if !workbench.publish_worktree_operation(
            plan,
            WorktreeOperationStatus::Pending {
                phase: plan.phase,
                progress: progress.clone(),
            },
        ) {
            return Err(Failure::Internal {});
        }
        let _result = terminal.commit();
        plan.progress = progress;
        Ok(())
    }
}
