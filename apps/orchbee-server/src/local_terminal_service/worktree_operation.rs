//! Local transport and retained continuation for canonical Worktree operations.
use super::LocalTerminalService;
use orchbee_server::WorkbenchGrant;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchWorktreeAction, WorkbenchWorktreeOutcome, WorkbenchWorktreeRequest,
    WorkbenchWorktreeResult, WorkerId, WorktreeAdmissionFailure,
};
use std::sync::Arc;
#[cfg(any(windows, test))]
use std::{io, time::Instant};
mod driver;

impl LocalTerminalService {
    pub(crate) fn execute_worktree_operation(
        self: &Arc<Self>,
        request: WorkbenchWorktreeRequest,
    ) -> WorkbenchWorktreeResult {
        let unavailable = |reason| WorkbenchWorktreeResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            operation_id: request.action.operation_id().clone(),
            outcome: WorkbenchWorktreeOutcome::Unavailable { reason },
        };
        match self.workbench_site_matches(&request.site_id) {
            Some(true) => {}
            Some(false) => {
                return WorkbenchWorktreeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    operation_id: request.action.operation_id().clone(),
                    outcome: WorkbenchWorktreeOutcome::Denied {},
                };
            }
            None => return unavailable(WorktreeAdmissionFailure::ServiceUnavailable),
        }
        let mutating = !matches!(request.action, WorkbenchWorktreeAction::Read { .. });
        if mutating && !cfg!(windows) {
            return unavailable(WorktreeAdmissionFailure::Unsupported);
        }
        if mutating && self.worktree_maintenance.ensure(self).is_err() {
            return unavailable(WorktreeAdmissionFailure::ServiceUnavailable);
        }
        let result = {
            let Ok(workers) = self.workers.lock() else {
                return unavailable(WorktreeAdmissionFailure::ServiceUnavailable);
            };
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return unavailable(WorktreeAdmissionFailure::ServiceUnavailable);
            };
            let Some(authority) = coordinator.worktree_authority() else {
                return unavailable(WorktreeAdmissionFailure::ServiceUnavailable);
            };
            let worker = workers.request_handle().and_then(|handle| {
                WorkerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(handle.worker_id().as_bytes())
                ))
                .ok()
                .map(|id| (id, handle.generation()))
            });
            let grant =
                WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone())
                    .with_directory_association()
                    .with_terminal_lifecycle();
            authority.execute_worktree_request(
                Some(&grant),
                request,
                worker,
                self.worktree_maintenance.available(),
            )
        };
        if mutating {
            self.worktree_maintenance.notify();
        }
        result
    }

    #[cfg(any(windows, test))]
    pub(crate) fn stop_worktrees_before(&self, deadline: Instant) -> io::Result<()> {
        self.worktree_maintenance.stop_before(deadline)
    }

    pub(super) fn worktree_turn(&self, stop: &std::sync::atomic::AtomicBool) {
        let plan = self.coordinator.lock().ok().and_then(|mut coordinator| {
            coordinator.worktree_authority()?.claim_worktree_operation()
        });
        let Some(mut plan) = plan else {
            return;
        };
        let status = driver::run(self, &mut plan, stop);
        if let Ok(mut coordinator) = self.coordinator.lock()
            && let Some(authority) = coordinator.worktree_authority()
        {
            let _published = authority.publish_worktree_operation(&plan, status);
        }
    }
}

#[cfg(all(test, windows))]
mod tests;
