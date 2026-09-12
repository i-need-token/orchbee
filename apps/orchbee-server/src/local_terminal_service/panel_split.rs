use super::{LocalTerminalService, lease_expiry, placement, submit};
use crate::site_coordinator::{PostcommitCoordinationError, PreparedTerminalPanelSplit};
use orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds;
use orchbee_client_server_protocol::RequestId;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason,
};

impl LocalTerminalService {
    pub(crate) fn execute_terminal_panel_split(
        &self,
        command: WorkbenchCommand,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        self.execute_terminal_panel_split_with_candidates(
            command,
            super::create_reference(),
            lease_expiry(),
        )
    }

    fn execute_terminal_panel_split_with_candidates(
        &self,
        command: WorkbenchCommand,
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        let (prepared, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return Ok(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                ));
            };
            let handle = workers.request_handle();
            let current = placement(handle.as_ref());
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return Ok(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                ));
            };
            let prepared =
                coordinator.prepare_terminal_panel_split(command, candidate, expiry, current);
            (prepared, handle)
        };
        let pending = match prepared {
            PreparedTerminalPanelSplit::Ready(result) => return Ok(result),
            PreparedTerminalPanelSplit::Dispatch(pending) => pending,
        };
        let completion = handle.map_or(super::Completion::Unavailable, |handle| {
            submit(&handle, pending.command())
        });

        // Reacquire registry -> Site and retain both gates through the no-fail joint settlement.
        let workers = self
            .workers
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let current = placement(workers.request_handle().as_ref());
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let completed = coordinator.complete_terminal_panel_split(pending, completion, current);
        drop(coordinator);
        drop(workers);
        let completed = completed?;
        if completed.standalone_changed {
            self.standalone_changed.notify_all();
        }
        Ok(completed.result)
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

#[cfg(test)]
mod tests;
