//! Drives a prepared Panel creation through current Worker placement, dispatch, and exact
//! postcommit settlement without holding the coordinator lock during I/O. It does not decide
//! topology independently or replay an uncertain terminal effect.

use super::{LocalTerminalService, lease_expiry, placement};
use crate::session_coordinator::{PostcommitCoordinationError, PreparedTerminalPanelCreate};
use crate::terminal_coordinator::{CreateInputs, LaunchCapture};
use orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds;
use orchbee_client_server_protocol::RequestId;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason,
};

impl LocalTerminalService {
    pub(crate) fn execute_terminal_panel_create(
        &self,
        command: WorkbenchCommand,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        self.execute_terminal_panel_create_with_candidates(
            command,
            super::create_reference(),
            lease_expiry(),
            self.launch_capture(),
        )
    }

    /// The same creation for a configured Panel command, whose Panel presents `program`, the
    /// program its command line starts, in place of the adapter its launch dispatches.
    pub(in crate::local_terminal_service) fn execute_configured_panel_create(
        &self,
        command: WorkbenchCommand,
        program: Option<orchbee_workbench_protocol::PresentationLaunchProgram>,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        self.execute_terminal_panel_create_with_candidates(
            command,
            super::create_reference(),
            lease_expiry(),
            self.launch_capture().presenting(program),
        )
    }

    fn execute_terminal_panel_create_with_candidates(
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
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return Ok(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                ));
            };
            let handle = super::creation_handle(&workers, &coordinator, &command);
            let inputs = CreateInputs::admitted_by(candidate, expiry, handle.as_ref(), capture);
            let prepared = coordinator.prepare_terminal_panel_create(command, inputs);
            (prepared, handle)
        };
        let pending = match prepared {
            PreparedTerminalPanelCreate::Ready(result) => return Ok(result),
            PreparedTerminalPanelCreate::Dispatch(pending) => pending,
        };
        let dispatched = placement(handle.as_ref());
        let completion = handle.map_or(super::Completion::Unavailable, |handle| {
            self.submit(&handle, pending.command())
        });

        // Reacquire registry -> Session and retain both gates through the no-fail joint settlement.
        let workers = self
            .workers
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let current = super::current_placement(&workers, dispatched);
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let completed = coordinator.complete_terminal_panel_create(pending, completion, current);
        // Offered under both locks, so notes about the new terminal precede any Client input.
        if let Ok(completed) = &completed
            && let Some(launch) = &completed.agent_start
        {
            self.deliver_agent_start(&mut coordinator, current, launch);
        }
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
