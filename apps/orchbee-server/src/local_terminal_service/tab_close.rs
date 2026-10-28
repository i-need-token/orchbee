//! Drives retained Tab and Workspace close continuations, explicit resumes, lifecycle
//! settlement, and publication through bounded maintenance turns. It does not own the close
//! state machine or replace physical disposal evidence with topology changes.

use std::time::{Duration, Instant};

use orchbee_capability_node_protocol::LifecycleUnknownReason;
use orchbee_server::{CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure};
use orchbee_workbench_protocol::{
    WorkbenchCommand, WorkbenchCommandResult, WorkbenchRequestId, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeResult,
    WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeResult,
};

use super::{LocalTerminalService, lease_expiry, placement};
use crate::{
    session_coordinator::PostcommitCoordinationError,
    worker_obcn_carrier::{WorkerRequestError, WorkerResponseFailure},
};

// Inert candidate failure yields at this cadence; it never renews the retained attempt.
const CANDIDATE_RETRY: Duration = Duration::from_millis(250);
mod idle;

#[cfg(test)]
mod tests;

impl LocalTerminalService {
    pub(crate) fn execute_project_dissolve_if_idle(
        &self,
        command: WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        let workers = self.workers.lock().ok();
        let handle = workers
            .as_ref()
            .and_then(|workers| workers.request_handle());
        self.coordinator
            .lock()
            .ok()?
            .execute_project_dissolve_if_idle(command, placement(handle.as_ref()))
    }

    pub(crate) fn execute_terminal_tab_close(
        &self,
        command: WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        let result = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let mut coordinator = self.coordinator.lock().ok()?;
            coordinator.execute_terminal_tab_close(
                command,
                placement(handle.as_ref()),
                self.close_maintenance.available(),
                Instant::now(),
            )
        };
        // Replays can harmlessly notify, but never change an attempt or wait for IPC.
        self.close_maintenance.notify();
        result
    }

    /// Server validated an exact Host exit record with exit code 0 for `target` on a viewer issued
    /// by `handle`. The Session decides whether that terminal's Panel closes; an admitted Close then
    /// runs the ordinary continuation. Ineligible, fenced or stale targets change nothing.
    pub(super) fn close_after_normal_exit(
        &self,
        handle: &crate::worker_obcn_carrier::WorkerRequestHandle,
        target: crate::terminal_coordinator::BindingViewerTarget,
    ) {
        let admitted = {
            let Ok(workers) = self.workers.lock() else {
                return;
            };
            let current = placement(workers.request_handle().as_ref());
            if !handle.is_active()
                || current != Some(target.worker)
                || placement(Some(handle)) != Some(target.worker)
            {
                return;
            }
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return;
            };
            coordinator.close_after_normal_exit(
                target,
                close_request_id,
                current,
                self.close_maintenance.available(),
                Instant::now(),
            )
        };
        if admitted {
            self.close_maintenance.notify();
        }
    }

    pub(crate) fn execute_workspace_close(
        &self,
        command: WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        if matches!(
            command.action,
            orchbee_workbench_protocol::WorkbenchAction::WorkspaceCloseIfIdle { .. }
        ) {
            return self.execute_workspace_close_if_idle(command);
        }
        let result = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let mut coordinator = self.coordinator.lock().ok()?;
            coordinator.execute_workspace_close(
                command,
                placement(handle.as_ref()),
                self.close_maintenance.available(),
                Instant::now(),
            )
        };
        // Replays can harmlessly notify, but never change an attempt or wait for IPC.
        self.close_maintenance.notify();
        result
    }

    pub(crate) fn read_close_operation(
        &self,
        request: orchbee_workbench_protocol::CloseOperationRead,
    ) -> Option<orchbee_workbench_protocol::CloseOperationResult> {
        self.coordinator.lock().ok()?.read_close_operation(request)
    }

    pub(crate) fn read_workspace_close(
        &self,
        request: WorkbenchWorkspaceCloseRead,
    ) -> Option<WorkbenchWorkspaceCloseReadResult> {
        self.coordinator.lock().ok()?.read_workspace_close(request)
    }

    pub(crate) fn resume_workspace_close(
        &self,
        request: WorkbenchWorkspaceCloseResume,
    ) -> Option<WorkbenchWorkspaceCloseResumeResult> {
        let result = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let mut coordinator = self.coordinator.lock().ok()?;
            let local_cleanup_available = self.controllers.lock().is_ok();
            coordinator.resume_workspace_close(
                request,
                placement(handle.as_ref()),
                self.close_maintenance.available(),
                local_cleanup_available,
                Instant::now(),
            )
        };
        self.close_maintenance.notify();
        result
    }

    pub(crate) fn read_terminal_tab_close(
        &self,
        request: WorkbenchTabCloseRead,
    ) -> Option<WorkbenchTabCloseReadResult> {
        self.coordinator
            .lock()
            .ok()?
            .read_terminal_tab_close(request)
    }

    pub(crate) fn resume_terminal_tab_close(
        &self,
        request: WorkbenchTabCloseResume,
    ) -> Option<WorkbenchTabCloseResumeResult> {
        let result = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let mut coordinator = self.coordinator.lock().ok()?;
            let local_cleanup_available = self.controllers.lock().is_ok();
            coordinator.resume_terminal_tab_close(
                request,
                placement(handle.as_ref()),
                self.close_maintenance.available(),
                local_cleanup_available,
                Instant::now(),
            )
        };
        self.close_maintenance.notify();
        result
    }

    pub(super) fn close_maintenance_turn(
        &self,
        cursor: &mut Option<WorkbenchRequestId>,
    ) -> Result<Option<Instant>, PostcommitCoordinationError> {
        let expiry = lease_expiry();
        let (close_id, token, handle, dependencies_match) = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let worker = placement(handle.as_ref());
            let mut coordinator = self
                .coordinator
                .lock()
                .map_err(|_| PostcommitCoordinationError)?;
            let operations: Vec<_> = coordinator
                .close_authority()?
                .terminal_tab_close_operations()
                .cloned()
                .collect();
            let pivot = cursor
                .as_ref()
                .map_or(0, |cursor| operations.partition_point(|id| id <= cursor));
            let mut due = None;
            let mut selected = None;
            for id in operations[pivot..].iter().chain(&operations[..pivot]) {
                let now = Instant::now();
                let step = coordinator
                    .close_authority()?
                    .next_terminal_tab_close_step_target(id, now)
                    .map_err(|_| PostcommitCoordinationError)?;
                let next = step.next();
                match next {
                    CloseContinuationNext::WaitUntil(at) => {
                        due = Some(due.map_or(at, |due: Instant| due.min(at)))
                    }
                    CloseContinuationNext::Parked
                    | CloseContinuationNext::Closed
                    | CloseContinuationNext::Outstanding => {}
                    actionable => {
                        selected = Some((id.clone(), step, actionable));
                        break;
                    }
                }
            }
            let Some((id, step, next)) = selected else {
                return Ok(due);
            };
            *cursor = Some(id.clone());
            match next {
                CloseContinuationNext::TargetCleanup => {
                    let runtime = step
                        .target()
                        .ok_or(PostcommitCoordinationError)?
                        .runtime()
                        .ok_or(PostcommitCoordinationError)?;
                    // One Session guard spans exact removal and its retained completion marker.
                    if self
                        .complete_disposed_target_cleanup(&coordinator, &id, runtime)
                        .is_ok()
                    {
                        coordinator
                            .close_authority()?
                            .complete_terminal_tab_close_target_cleanup(&id, runtime)
                            .map_err(|_| PostcommitCoordinationError)?;
                    } else {
                        coordinator
                            .close_authority()?
                            .park_terminal_tab_close_target_cleanup(&id, runtime)
                            .map_err(|_| PostcommitCoordinationError)?;
                    }
                    return Ok(Some(Instant::now()));
                }
                CloseContinuationNext::Publication => {
                    coordinator.publish_terminal_tab_close(&id, Instant::now())?;
                    drop(coordinator);
                    drop(workers);
                    return Ok(Some(Instant::now()));
                }
                CloseContinuationNext::Lifecycle {
                    action,
                    step_ordinal,
                } => {
                    let target = step.target().cloned().ok_or(PostcommitCoordinationError)?;
                    let now = Instant::now();
                    let Some(expiry) = expiry else {
                        return Ok(now.checked_add(CANDIDATE_RETRY));
                    };
                    let matches = coordinator.close_runtime_matches_for_member(
                        &id,
                        target.member().binding(),
                        worker,
                    );
                    let Some(token) = coordinator.reserve_close_lifecycle_step_for_target(
                        &id,
                        target,
                        action,
                        step_ordinal,
                        now,
                        expiry,
                    )?
                    else {
                        return Ok(now.checked_add(CANDIDATE_RETRY));
                    };
                    (id, token, handle, matches)
                }
                _ => return Err(PostcommitCoordinationError),
            }
        };

        // Prepared state is already retained. No owner gate spans queue admission or wait.
        let submitted = match handle.filter(|_| dependencies_match) {
            Some(handle) => handle.submit_lifecycle(token.request()),
            None => Err(WorkerRequestError::Unavailable),
        };
        let receipt = match submitted {
            Ok(receipt) => receipt,
            Err(error) => {
                let failure = match error {
                    WorkerRequestError::Backpressure => CloseStepPrequeueFailure::Backpressure,
                    WorkerRequestError::Unavailable => CloseStepPrequeueFailure::Unavailable,
                    _ => CloseStepPrequeueFailure::IdentityConflict,
                };
                self.coordinator
                    .lock()
                    .map_err(|_| PostcommitCoordinationError)?
                    .close_authority()?
                    .reject_terminal_tab_close_step_prequeue(
                        &close_id,
                        token,
                        Instant::now(),
                        failure,
                    )
                    .map_err(|_| PostcommitCoordinationError)?;
                return Ok(Some(Instant::now()));
            }
        };
        // Confirmation cannot be suppressed by attempt expiry or Worker replacement.
        #[cfg(test)]
        self.run_close_admission_hook();
        let confirmation = self
            .coordinator
            .lock()
            .map_err(|_| PostcommitCoordinationError)
            .and_then(|mut coordinator| {
                coordinator
                    .close_authority()?
                    .confirm_terminal_tab_close_step_admitted(
                        &close_id,
                        &token,
                        receipt.admitted_observed_at(),
                    )
                    .map_err(|_| PostcommitCoordinationError)
            });
        let result = receipt.wait();
        confirmation?;
        #[cfg(test)]
        self.run_close_result_hook();
        let observation = match &result {
            Ok(result) => CloseStepObservation::Result(result),
            Err(WorkerRequestError::Unavailable | WorkerRequestError::NotDelivered(_)) => {
                CloseStepObservation::NotDelivered
            }
            Err(WorkerRequestError::PostDeliveryOutcomeUnknown(reason)) => {
                CloseStepObservation::OutcomeUnknown(match reason {
                    WorkerResponseFailure::TimedOut => LifecycleUnknownReason::Deadline,
                    WorkerResponseFailure::Disconnected
                    | WorkerResponseFailure::CarrierStopped
                    | WorkerResponseFailure::Io(_) => LifecycleUnknownReason::TransportLost,
                    _ => LifecycleUnknownReason::InvalidResponse,
                })
            }
            Err(_) => CloseStepObservation::OutcomeUnknown(LifecycleUnknownReason::InvalidResponse),
        };
        self.coordinator
            .lock()
            .map_err(|_| PostcommitCoordinationError)?
            .close_authority()?
            .settle_terminal_tab_close_step(&close_id, token, Instant::now(), observation)
            .map_err(|_| PostcommitCoordinationError)?;
        Ok(Some(Instant::now()))
    }
}

fn close_request_id() -> Option<WorkbenchRequestId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes))).ok()
}
