//! Windows group Close transport and retained continuation; reads never start maintenance.
use super::{LocalTerminalService, worker_identity};
use orchbee_server::WorkbenchGrant;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchWorkspaceGroupCloseRequest, WorkbenchWorkspaceGroupCloseResult,
    WorkspaceGroupCloseAction as Action, WorkspaceGroupCloseOutcome as Outcome,
    WorkspaceGroupCloseUnavailable as Unavailable,
};
use std::sync::{Arc, atomic::AtomicBool};
#[cfg(windows)]
use std::{io, time::Instant};
mod driver;
impl LocalTerminalService {
    pub(crate) fn execute_workspace_group_close(
        self: &Arc<Self>,
        request: WorkbenchWorkspaceGroupCloseRequest,
    ) -> WorkbenchWorkspaceGroupCloseResult {
        let response = |outcome| WorkbenchWorkspaceGroupCloseResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            operation_id: request.action.operation_id().clone(),
            outcome,
        };
        match self.workbench_session_matches(&request.session_id) {
            Some(true) => {}
            Some(false) => return response(Outcome::Denied {}),
            None => {
                return response(Outcome::Unavailable {
                    reason: Unavailable::ServiceUnavailable,
                });
            }
        }
        let mutating = !matches!(request.action, Action::Read { .. });
        if mutating && !cfg!(windows) {
            return response(Outcome::Unavailable {
                reason: Unavailable::Unsupported,
            });
        }
        if mutating && self.group_close_maintenance.ensure(self).is_err() {
            return response(Outcome::Unavailable {
                reason: Unavailable::ServiceUnavailable,
            });
        }
        let result = {
            let Ok(workers) = self.workers.lock() else {
                return response(Outcome::Unavailable {
                    reason: Unavailable::ServiceUnavailable,
                });
            };
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return response(Outcome::Unavailable {
                    reason: Unavailable::ServiceUnavailable,
                });
            };
            let Some(authority) = coordinator.worktree_authority() else {
                return response(Outcome::Unavailable {
                    reason: Unavailable::ServiceUnavailable,
                });
            };
            let grant =
                WorkbenchGrant::read_only(authority.session_id().clone()).with_terminal_lifecycle();
            authority.execute_workspace_group_close(
                Some(&grant),
                request,
                worker_identity(workers.request_handle().as_ref()),
                self.group_close_maintenance.available(),
            )
        };
        if mutating {
            self.group_close_maintenance.notify();
        }
        result
    }
    #[cfg(windows)]
    pub(crate) fn stop_workspace_groups_before(&self, deadline: Instant) -> io::Result<()> {
        self.group_close_maintenance.stop_before(deadline)
    }
    pub(super) fn workspace_group_close_turn(&self, stop: &AtomicBool) {
        let plan = self.coordinator.lock().ok().and_then(|mut coordinator| {
            coordinator
                .worktree_authority()?
                .claim_workspace_group_close()
        });
        let Some(mut plan) = plan else {
            return;
        };
        let status = driver::run(self, &mut plan, stop);
        if let Ok(mut coordinator) = self.coordinator.lock()
            && let Some(authority) = coordinator.worktree_authority()
        {
            let _published = authority.publish_workspace_group_close(&plan, status);
        }
    }
}
