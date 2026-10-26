//! Validates a Workspace launch directory through the selected current Worker and commits only
//! the matching observation before continuing the requested launch. It does not infer
//! filesystem usability from wire syntax or modify a running shell's cwd.

use super::{LocalTerminalService, placement};
use crate::worker_obcn_carrier::WorkerRequestError;
use orchbee_capability_node_protocol::{
    AbsoluteCwd, OperationId, TerminalCwdValidationOutcome, TerminalCwdValidationRequest,
    TerminalCwdValidationResult,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkerId,
};

impl LocalTerminalService {
    pub(crate) fn execute_workspace_directory(
        &self,
        command: WorkbenchCommand,
    ) -> WorkbenchCommandResult {
        match self.observe_workspace_directory(&command) {
            Ok((request, observed)) => {
                self.complete_workspace_directory(command, request, observed)
            }
            Err(result) => *result,
        }
    }

    fn observe_workspace_directory(
        &self,
        command: &WorkbenchCommand,
    ) -> Result<
        (
            TerminalCwdValidationRequest,
            Result<TerminalCwdValidationResult, WorkerRequestError>,
        ),
        Box<WorkbenchCommandResult>,
    > {
        // Entropy is an inert candidate; its failure must not hide a retained acceptance.
        let operation_id = validation_identity();
        let (handle, request) = {
            let Ok(workers) = self.workers.lock() else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                )));
            };
            let Ok(coordinator) = self.coordinator.lock() else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                )));
            };
            if let Some(result) = coordinator.preflight_workspace_directory(command) {
                return Err(Box::new(result));
            }
            let Some(handle) = workers.request_handle() else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                )));
            };
            let Some(worker) = placement(Some(&handle)) else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                )));
            };
            let Some(operation_id) = operation_id else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::ServerBusy,
                )));
            };
            let path = match &command.action {
                WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } => Some(path),
                WorkbenchAction::WorkspaceLaunch { options, .. } => options.cwd.as_ref(),
                WorkbenchAction::PanelRelocate {
                    destination:
                        orchbee_workbench_protocol::WorkbenchPanelRelocateDestination::NewWorkspace {
                            primary_directory_path,
                            ..
                        },
                    ..
                } => primary_directory_path.as_ref(),
                // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
                _ => None,
            };
            let Some(path) = path else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::ServiceUnavailable,
                )));
            };
            let Ok(cwd) = AbsoluteCwd::new(path.as_str()) else {
                return Err(Box::new(unavailable(
                    command,
                    WorkbenchCommandUnavailableReason::DirectoryUnavailable,
                )));
            };
            (
                handle,
                TerminalCwdValidationRequest {
                    operation_id,
                    worker_id: worker.worker_id,
                    worker_generation: worker.generation,
                    cwd,
                },
            )
        };

        // The Worker observes the filesystem without registry or Session ownership held.
        let observed = handle.validate_terminal_cwd(request.clone());
        Ok((request, observed))
    }

    pub(crate) fn execute_workspace_launch(
        &self,
        command: WorkbenchCommand,
    ) -> Result<WorkbenchCommandResult, crate::session_coordinator::PostcommitCoordinationError>
    {
        let (request, observed) = match self.observe_workspace_directory(&command) {
            Ok(probe) => probe,
            Err(result) => return Ok(*result),
        };
        let candidate = super::create_reference();
        let expiry = super::lease_expiry();
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
            if let Some(result) = coordinator.preflight_workspace_directory(&command) {
                return Ok(result);
            }
            let handle = workers.request_handle();
            let current = placement(handle.as_ref());
            if let Err(reason) = validate_observation(&request, observed, current.as_ref()) {
                return Ok(unavailable(&command, reason));
            }
            let Some(current) = current else {
                return Ok(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                ));
            };
            (
                coordinator.prepare_workspace_launch(command, candidate, expiry, current),
                handle,
            )
        };
        self.dispatch_terminal_tab_create(prepared, handle)
    }

    fn complete_workspace_directory(
        &self,
        command: WorkbenchCommand,
        request: TerminalCwdValidationRequest,
        observed: Result<TerminalCwdValidationResult, WorkerRequestError>,
    ) -> WorkbenchCommandResult {
        let Ok(workers) = self.workers.lock() else {
            return unavailable(&command, WorkbenchCommandUnavailableReason::ServerBusy);
        };
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return unavailable(&command, WorkbenchCommandUnavailableReason::ServerBusy);
        };
        // A concurrent exact request may already have committed, even if our observation failed.
        if let Some(result) = coordinator.preflight_workspace_directory(&command) {
            return result;
        }
        let current = placement(workers.request_handle().as_ref());
        match validate_observation(&request, observed, current.as_ref()) {
            Ok(worker_id) => coordinator.accept_workspace_directory(command, worker_id),
            Err(reason) => unavailable(&command, reason),
        }
    }
}

fn validate_observation(
    request: &TerminalCwdValidationRequest,
    observed: Result<TerminalCwdValidationResult, WorkerRequestError>,
    current: Option<&crate::terminal_coordinator::WorkerPlacement>,
) -> Result<WorkerId, WorkbenchCommandUnavailableReason> {
    use WorkbenchCommandUnavailableReason::{DirectoryUnavailable, NoCompatibleWorker, ServerBusy};
    if current.is_none_or(|worker| {
        worker.worker_id != request.worker_id || worker.generation != request.worker_generation
    }) {
        return Err(NoCompatibleWorker);
    }
    match observed {
        Ok(result) if result.request == *request => match result.outcome {
            TerminalCwdValidationOutcome::DirectoryAvailable => WorkerId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(request.worker_id.as_bytes())
            ))
            .map_err(|_| NoCompatibleWorker),
            TerminalCwdValidationOutcome::DirectoryUnavailable => Err(DirectoryUnavailable),
            TerminalCwdValidationOutcome::WorkerUnavailable => Err(NoCompatibleWorker),
        },
        Err(WorkerRequestError::Backpressure) => Err(ServerBusy),
        _ => Err(NoCompatibleWorker),
    }
}

fn validation_identity() -> Option<OperationId> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    OperationId::from_bytes(bytes).ok()
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
