//! Drives the start of a Presentation a checkpoint restore held for an unavailable directory:
//! it rechecks the saved directory outside every gate, launches through current Worker placement
//! without holding the coordinator lock during I/O, and hands a retained Agent reference to the
//! resume driver once the shell started where it was recorded. It never retries on its own.

use super::{LocalTerminalService, create_reference, lease_expiry, placement};
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
        let resume = self.agent_resume_enabled();
        // The held Presentation keeps its file while held; read it outside every gate.
        let seed = probe
            .output
            .as_ref()
            .filter(|_| self.output_replay_enabled())
            .and_then(|(presentation, token)| self.restored_output_seed(presentation, *token));
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
                    if let Some(seed) = seed {
                        pending.set_output_seed(seed);
                    }
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
        // every one that reaches the started shell. It joins the running driver or starts a new
        // run; only a stopping Server refuses, and the final checkpoint then carries the reference.
        if let Some((presentation, directory)) = completed.agent_resume
            && resume
            && coordinator.has_pending_agent_session(&presentation)
        {
            let _kept_pending =
                self.resume_retried_agent(&coordinator, current, presentation, directory);
        }
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
