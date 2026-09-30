use orchbee_capability_node_protocol::{TerminalId, WorkerId};
use orchbee_server::{
    WorkbenchTabCloseMemberRuntime, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTabCloseRuntimeUnavailableReason,
};
use orchbee_workbench_protocol::{
    PresentationBinding, WorkbenchRequestId, WorkbenchTabCloseStatus,
};

use super::SiteCoordinator;
use crate::terminal_coordinator::{
    BindingViewerTarget, RetainedRuntimeTargetError, TerminalCoordinator, WorkerPlacement,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetLifecycle {
    Unfenced,
    Fenced,
    Disposed,
}

impl SiteCoordinator {
    pub(super) fn binding_has_close(&self, binding: &PresentationBinding) -> bool {
        self.workbench
            .as_ref()
            .is_some_and(|authority| authority.has_terminal_tab_close(binding))
    }

    pub(crate) fn target_lifecycle(&self, target: BindingViewerTarget) -> TargetLifecycle {
        let Some(authority) = self.workbench.as_ref() else {
            return TargetLifecycle::Unfenced;
        };
        #[cfg(windows)]
        if self.terminals.target_retired(target) {
            return TargetLifecycle::Fenced;
        }
        let terminal_id = orchbee_workbench_protocol::TerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(target.terminal_id.as_bytes())
        ));
        let worker_id = orchbee_workbench_protocol::WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(target.worker.worker_id.as_bytes())
        ));
        let (Ok(terminal_id), Ok(worker_id)) = (terminal_id, worker_id) else {
            return TargetLifecycle::Fenced;
        };
        let binding = PresentationBinding {
            terminal_id,
            worker_id,
        };
        let Some(close_id) = authority.terminal_tab_close_request_for_binding(&binding) else {
            return TargetLifecycle::Unfenced;
        };
        let Some(retained) = authority.terminal_tab_close_target_for_binding(close_id, &binding)
        else {
            return TargetLifecycle::Fenced;
        };
        let Some(runtime) = retained.runtime() else {
            return TargetLifecycle::Fenced;
        };
        if runtime.worker_id() == target.worker.worker_id
            && runtime.worker_generation() == target.worker.generation
            && runtime.terminal_id() == target.terminal_id
            && runtime.host_identity() == target.identity
            && is_disposed(authority.terminal_tab_close_target_status(close_id, runtime))
        {
            TargetLifecycle::Disposed
        } else {
            TargetLifecycle::Fenced
        }
    }

    pub(crate) fn is_disposed_close_target(
        &self,
        close_id: &WorkbenchRequestId,
        runtime: WorkbenchTabCloseRuntimeTarget,
    ) -> bool {
        self.workbench.as_ref().is_some_and(|authority| {
            is_disposed(authority.terminal_tab_close_target_status(close_id, runtime))
        })
    }

    pub(crate) fn terminal_tab_close_runtime(
        terminals: &TerminalCoordinator,
        binding: &PresentationBinding,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<WorkbenchTabCloseMemberRuntime, WorkbenchTabCloseRuntimeUnavailableReason> {
        let unavailable = WorkbenchTabCloseRuntimeUnavailableReason::TerminalRuntimeUnavailable;
        let terminal =
            u128::from_str_radix(binding.terminal_id.as_str(), 16).map_err(|_| unavailable)?;
        let worker =
            u128::from_str_radix(binding.worker_id.as_str(), 16).map_err(|_| unavailable)?;
        let terminal = TerminalId::from_bytes(terminal.to_be_bytes()).map_err(|_| unavailable)?;
        let worker = WorkerId::from_bytes(worker.to_be_bytes()).map_err(|_| unavailable)?;
        if terminals.is_definitively_uncreated(terminal, worker) {
            return Ok(WorkbenchTabCloseMemberRuntime::Uncreated {
                binding: binding.clone(),
            });
        }
        let (target, fence) = terminals
            .retained_runtime_target(terminal, worker, current_worker)
            .map_err(|error| match error {
                RetainedRuntimeTargetError::Busy => {
                    WorkbenchTabCloseRuntimeUnavailableReason::TerminalBusy
                }
                RetainedRuntimeTargetError::Unavailable => unavailable,
            })?;
        // A retained Started completion is not a current Host process-state observation.
        Ok(WorkbenchTabCloseMemberRuntime::Runtime {
            target: WorkbenchTabCloseRuntimeTarget::new(
                target.worker.worker_id,
                target.worker.generation,
                target.terminal_id,
                target.identity,
                fence,
            ),
            observed_phase: None,
        })
    }

    pub(crate) fn terminal_tab_close_runtime_matches(
        terminals: &TerminalCoordinator,
        binding: &PresentationBinding,
        expected: Option<WorkbenchTabCloseRuntimeTarget>,
        current_worker: Option<WorkerPlacement>,
    ) -> bool {
        Self::terminal_tab_close_runtime(terminals, binding, current_worker)
            .is_ok_and(|evidence| matches!(evidence, WorkbenchTabCloseMemberRuntime::Runtime { target, .. } if Some(target) == expected))
    }
}

fn is_disposed(status: Option<WorkbenchTabCloseStatus>) -> bool {
    matches!(
        status,
        Some(
            WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
                | WorkbenchTabCloseStatus::Closed { .. }
        )
    )
}
