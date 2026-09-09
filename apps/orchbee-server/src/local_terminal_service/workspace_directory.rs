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
use std::{fs::File, io::Read};

impl LocalTerminalService {
    pub(crate) fn execute_workspace_directory(
        &self,
        command: WorkbenchCommand,
    ) -> WorkbenchCommandResult {
        // Entropy is an inert candidate; its failure must not hide a retained acceptance.
        let operation_id = validation_identity();
        let (handle, request) = {
            let Ok(workers) = self.workers.lock() else {
                return unavailable(&command, WorkbenchCommandUnavailableReason::ServerBusy);
            };
            let Ok(coordinator) = self.coordinator.lock() else {
                return unavailable(&command, WorkbenchCommandUnavailableReason::ServerBusy);
            };
            if let Some(result) = coordinator.preflight_workspace_directory(&command) {
                return result;
            }
            let Some(handle) = workers.request_handle() else {
                return unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                );
            };
            let Some(worker) = placement(Some(&handle)) else {
                return unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                );
            };
            let Some(operation_id) = operation_id else {
                return unavailable(&command, WorkbenchCommandUnavailableReason::ServerBusy);
            };
            let WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } = &command.action
            else {
                return unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServiceUnavailable,
                );
            };
            let Ok(cwd) = AbsoluteCwd::new(path.as_str()) else {
                return unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::DirectoryUnavailable,
                );
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

        // The Worker observes the filesystem without registry or Site ownership held.
        let observed = handle.validate_terminal_cwd(request.clone());
        self.complete_workspace_directory(command, request, observed)
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
        if placement(workers.request_handle().as_ref()).is_none_or(|worker| {
            worker.worker_id != request.worker_id || worker.generation != request.worker_generation
        }) {
            return unavailable(
                &command,
                WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            );
        }
        let outcome = match observed {
            Ok(result) if result.request == request => result.outcome,
            Ok(_) => {
                return unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                );
            }
            Err(WorkerRequestError::Backpressure) => {
                return unavailable(&command, WorkbenchCommandUnavailableReason::ServerBusy);
            }
            Err(_) => {
                return unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                );
            }
        };
        match outcome {
            TerminalCwdValidationOutcome::DirectoryUnavailable => unavailable(
                &command,
                WorkbenchCommandUnavailableReason::DirectoryUnavailable,
            ),
            TerminalCwdValidationOutcome::WorkerUnavailable => unavailable(
                &command,
                WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            ),
            TerminalCwdValidationOutcome::DirectoryAvailable => {
                let Ok(worker_id) = WorkerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(request.worker_id.as_bytes())
                )) else {
                    return unavailable(
                        &command,
                        WorkbenchCommandUnavailableReason::NoCompatibleWorker,
                    );
                };
                // Keep both gates through the authority's repeated admission and atomic commit.
                coordinator.accept_workspace_directory(command, worker_id)
            }
        }
    }
}

fn validation_identity() -> Option<OperationId> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
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
