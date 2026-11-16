//! Drives the start of a Presentation a checkpoint restore held for an unavailable directory:
//! it rechecks the saved directory outside every gate, launches through current Worker placement
//! without holding the coordinator lock during I/O, and hands a retained Agent reference to the
//! resume driver once the shell started where it was recorded. It never retries on its own.

use super::{
    LocalTerminalService,
    checkpoint::{Riders, attach_held_seed},
    create_reference, lease_expiry, placement,
};
use crate::session_coordinator::PostcommitCoordinationError;
use crate::terminal_coordinator::{Completion, CreateInputs};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason,
};
use std::sync::Arc;

impl LocalTerminalService {
    pub(crate) fn execute_panel_restore(
        self: &Arc<Self>,
        command: WorkbenchCommand,
    ) -> Result<WorkbenchCommandResult, PostcommitCoordinationError> {
        let busy = |command: &WorkbenchCommand| {
            unavailable(command, WorkbenchCommandUnavailableReason::ServerBusy)
        };
        let probe = {
            let Ok(coordinator) = self.coordinator.lock() else {
                return Ok(busy(&command));
            };
            match coordinator.held_restore_probe(&command) {
                Ok(probe) => probe,
                Err(result) => return Ok(*result),
            }
        };
        // The same check startup restore makes; a still unavailable directory keeps the hold.
        if probe
            .directory
            .as_ref()
            .is_some_and(|directory| !std::path::Path::new(directory.as_str()).is_dir())
        {
            return Ok(unavailable(
                &command,
                WorkbenchCommandUnavailableReason::DirectoryUnavailable,
            ));
        }
        let capture = self.launch_capture();
        let riders = Riders::current(self);
        // The held Presentation keeps its file while held; read it outside every gate.
        let seed = riders.held_seed(self, probe.output.as_ref());
        let (pending, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return Ok(busy(&command));
            };
            let handle = workers.request_handle();
            let inputs = CreateInputs::admitted_by(
                create_reference(),
                lease_expiry(),
                handle.as_ref(),
                capture,
            );
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return Ok(busy(&command));
            };
            match coordinator.prepare_held_restore(&command, inputs, probe.directory.as_ref()) {
                Ok(mut pending) => {
                    attach_held_seed(&mut pending, seed);
                    (pending, handle)
                }
                Err(result) => return Ok(*result),
            }
        };
        let completion = handle.map_or(Completion::Unavailable, |handle| {
            self.submit(&handle, pending.command())
        });

        // Reacquire registry -> Session and retain both gates through the no-fail settlement.
        let workers = self
            .workers
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let current = placement(workers.request_handle().as_ref());
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_poison| PostcommitCoordinationError)?;
        let completed = coordinator.complete_held_restore(pending, completion, current)?;
        // Handed over under the gate Client views and input are noted under, so the driver sees
        // every one that reaches the started shell.
        riders.resume_held_agent(self, &coordinator, current, completed.agent_resume);
        drop(coordinator);
        drop(workers);
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
