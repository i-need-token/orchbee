//! Drives admitted Tab and Workspace launches through Worker dispatch and exact coordinator
//! settlement while keeping I/O outside canonical locks. It does not allocate a second
//! membership plan or retry unknown launch delivery.

use super::{LocalTerminalService, lease_expiry, placement};
use crate::session_coordinator::{PostcommitCoordinationError, PreparedTerminalTabCreate};
use crate::terminal_coordinator::{CreateInputs, LaunchCapture};
use orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds;
use orchbee_client_server_protocol::RequestId;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason,
};

impl LocalTerminalService {
    pub(crate) fn execute_terminal_tab_create(
        &self,
        command: WorkbenchCommand,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        // Entropy and time are inert candidates. Workbench admission and replay are decided only
        // after both owner gates are held, so candidate failure cannot hide an exact replay.
        self.execute_terminal_tab_create_with_candidates(
            command,
            super::create_reference(),
            lease_expiry(),
            self.launch_capture(),
        )
    }

    fn execute_terminal_tab_create_with_candidates(
        &self,
        command: WorkbenchCommand,
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        capture: LaunchCapture,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        let (prepared, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return Ok(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                ));
            };
            let handle = workers.request_handle();
            let inputs = CreateInputs::admitted_by(candidate, expiry, handle.as_ref(), capture);
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return Ok(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                ));
            };
            let prepared = coordinator.prepare_terminal_tab_create(command, inputs);
            (prepared, handle)
        };
        self.dispatch_terminal_tab_create(prepared, handle)
    }

    /// Dispatch and settle an already admitted creation; all structural resources are committed.
    pub(super) fn dispatch_terminal_tab_create(
        &self,
        prepared: PreparedTerminalTabCreate,
        handle: Option<crate::worker_obcn_carrier::WorkerRequestHandle>,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        let pending = match prepared {
            PreparedTerminalTabCreate::Ready(result) => return Ok(result),
            PreparedTerminalTabCreate::Dispatch(pending) => pending,
        };
        let completion = handle.map_or(super::Completion::Unavailable, |handle| {
            self.submit(&handle, pending.command())
        });

        // Reacquire registry -> Session and retain both gates through the no-fail joint settlement.
        let workers = self
            .workers
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let current = placement(workers.request_handle().as_ref());
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let completed = coordinator.complete_terminal_tab_create(pending, completion, current);
        drop(coordinator);
        drop(workers);
        let completed = completed?;
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
