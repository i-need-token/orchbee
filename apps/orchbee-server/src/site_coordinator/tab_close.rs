use std::time::Instant;

use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalLifecycleAuthorization, TerminalLifecycleRequest, TerminalOperationLease,
};
use orchbee_server::{
    CloseContinuationNext, CloseStepToken, WorkbenchGrant, WorkbenchTabCloseBegin,
    WorkbenchTabClosePreparation, WorkbenchTabCloseResolution, WorkbenchTabCloseResumeBegin,
    WorkbenchTabCloseResumePreparation, WorkbenchTabCloseRuntimeUnavailableReason,
};
use orchbee_workbench_protocol::{
    WorkbenchCommand, WorkbenchCommandResult, WorkbenchRequestId, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeResult,
    WorkbenchTabCloseStatus,
};

use super::{PostcommitCoordinationError, SiteCoordinator, composition_matches};
use crate::terminal_coordinator::WorkerPlacement;

impl SiteCoordinator {
    pub(crate) fn execute_terminal_tab_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
    ) -> Option<WorkbenchCommandResult> {
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.site_id().clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_terminal_tab_close(Some(&grant), command) {
            WorkbenchTabCloseBegin::Ready(result) => return Some(result),
            WorkbenchTabCloseBegin::Admitted(admission) => admission,
        };
        if !composition_matches(admission.standalone_membership(), standalone, terminals) {
            return Some(admission.retire_composition_failure());
        }
        let admission = match admission.resolve_after_composition() {
            WorkbenchTabCloseResolution::Ready(result) => return Some(result),
            WorkbenchTabCloseResolution::Primary(admission) => admission,
        };
        if !owner_available {
            return Some(
                admission
                    .runtime_unavailable(WorkbenchTabCloseRuntimeUnavailableReason::ServerBusy),
            );
        }
        let (runtime, phase) =
            match Self::terminal_tab_close_runtime(terminals, admission.member().binding(), worker)
            {
                Ok(runtime) => runtime,
                Err(reason) => return Some(admission.runtime_unavailable(reason)),
            };
        Some(match admission.finish(runtime, phase, now) {
            WorkbenchTabClosePreparation::Ready(result) => result,
            WorkbenchTabClosePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn read_terminal_tab_close(
        &self,
        request: WorkbenchTabCloseRead,
    ) -> Option<WorkbenchTabCloseReadResult> {
        let workbench = self.workbench.as_ref()?;
        let grant = WorkbenchGrant::read_only(workbench.site_id().clone());
        Some(workbench.read_terminal_tab_close(Some(&grant), request))
    }

    pub(crate) fn resume_terminal_tab_close(
        &mut self,
        request: WorkbenchTabCloseResume,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        local_cleanup_available: bool,
        now: Instant,
    ) -> Option<WorkbenchTabCloseResumeResult> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.site_id().clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_terminal_tab_close_resume(Some(&grant), request) {
            WorkbenchTabCloseResumeBegin::Ready(result) => return Some(result),
            WorkbenchTabCloseResumeBegin::Admitted(admission) => admission,
        };
        if !owner_available {
            return Some(admission.runtime_unavailable());
        }
        if matches!(admission.status(), WorkbenchTabCloseStatus::DisposedAwaitingPublication { reason: orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::TargetCleanupPending, .. }) && !local_cleanup_available {
            return Some(admission.runtime_unavailable());
        }
        // The borrowed primary already proves the exact retained disposed target.
        // Only a continuation that may still contact Host needs a live Worker.
        if !matches!(
            admission.status(),
            WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
        ) && !Self::terminal_tab_close_runtime_matches(
            terminals,
            admission.target().member().binding(),
            admission.target().runtime(),
            worker,
        ) {
            return Some(admission.runtime_unavailable());
        }
        Some(match admission.finish(now) {
            WorkbenchTabCloseResumePreparation::Ready(result) => result,
            WorkbenchTabCloseResumePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn close_authority(
        &mut self,
    ) -> Result<&mut orchbee_server::WorkbenchAuthority, PostcommitCoordinationError> {
        self.workbench.as_mut().ok_or(PostcommitCoordinationError)
    }

    pub(crate) fn close_runtime_matches(
        &self,
        close_id: &WorkbenchRequestId,
        worker: Option<WorkerPlacement>,
    ) -> bool {
        self.workbench
            .as_ref()
            .and_then(|authority| authority.terminal_tab_close_target(close_id))
            .is_some_and(|target| {
                Self::terminal_tab_close_runtime_matches(
                    &self.terminals,
                    target.member().binding(),
                    target.runtime(),
                    worker,
                )
            })
    }

    pub(crate) fn reserve_close_lifecycle_step(
        &mut self,
        close_id: &WorkbenchRequestId,
        now: Instant,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Result<Option<CloseStepToken>, PostcommitCoordinationError> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut().ok_or(PostcommitCoordinationError)?;
        let CloseContinuationNext::Lifecycle {
            action,
            step_ordinal,
        } = workbench
            .next_terminal_tab_close_step(close_id, now)
            .map_err(|_| PostcommitCoordinationError)?
        else {
            return Ok(None);
        };
        let runtime = workbench
            .terminal_tab_close_target(close_id)
            .ok_or(PostcommitCoordinationError)?
            .runtime();
        let generation = workbench
            .terminal_tab_close_generation(close_id)
            .ok_or(PostcommitCoordinationError)?;
        let (Some(request_id), Some(lease_id)) =
            (terminals.allocate_identity(), terminals.allocate_identity())
        else {
            return Ok(None);
        };
        let request = TerminalLifecycleRequest {
            authorization: TerminalLifecycleAuthorization {
                capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
                terminal_id: runtime.terminal_id(),
                worker_id: runtime.worker_id(),
                worker_generation: runtime.worker_generation(),
                binding_fence: runtime.fence(),
                lease: TerminalOperationLease::new(
                    LeaseId::from_bytes(lease_id).map_err(|_| PostcommitCoordinationError)?,
                    expiry,
                ),
            },
            close_operation_id: OperationId::from_bytes(
                u128::from_str_radix(close_id.as_str(), 16)
                    .map_err(|_| PostcommitCoordinationError)?
                    .to_be_bytes(),
            )
            .map_err(|_| PostcommitCoordinationError)?,
            request_id: OperationId::from_bytes(request_id)
                .map_err(|_| PostcommitCoordinationError)?,
            attempt_generation: std::num::NonZeroU64::new(generation.value())
                .ok_or(PostcommitCoordinationError)?,
            step_ordinal,
            target: runtime.host_identity(),
            action,
        };
        workbench
            .reserve_terminal_tab_close_step(close_id, now, request)
            .map(Some)
            .map_err(|_| PostcommitCoordinationError)
    }

    pub(crate) fn publish_terminal_tab_close(
        &mut self,
        close_id: &WorkbenchRequestId,
        now: Instant,
    ) -> Result<bool, PostcommitCoordinationError> {
        let Self {
            terminals,
            workbench,
            standalone,
        } = self;
        let workbench = workbench.as_mut().ok_or(PostcommitCoordinationError)?;
        let Ok(prepared) = workbench.prepare_terminal_tab_close_publication(close_id, now) else {
            return Ok(false);
        };
        let retarget = super::standalone::prepare_standalone_close_settlement(
            standalone,
            terminals,
            prepared.before_membership(),
            prepared.after_membership(),
            prepared.exact_removed_member(),
        );
        let Ok(retarget) = retarget else {
            prepared.retire_composition_failure();
            return Ok(false);
        };
        let _closed = prepared.commit();
        Ok(retarget.commit())
    }
}
