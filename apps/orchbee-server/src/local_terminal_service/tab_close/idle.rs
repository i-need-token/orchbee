//! Fresh complete inventory preflight grants only guarded Close admission.
//! No Session/registry lock spans IPC; no inspection claims Worker Close ownership.
use super::super::{LocalTerminalService, lease_expiry, placement};
use crate::worker_obcn_carrier::WorkerRequestHandle;
use orchbee_capability_node_protocol::{
    LifecycleCloseActivity, LifecycleTargetObservation, LifecycleTerminalPhase,
    TerminalLifecycleOutcome,
};
use orchbee_server::{WorkbenchTabCloseMemberRuntime, WorkbenchTabCloseRuntimeTarget};
use orchbee_workbench_protocol::{WorkbenchCommand, WorkbenchCommandResult};
use std::time::{Duration, Instant};

const INSPECTION_BUDGET: Duration = Duration::from_secs(4);
const MAXIMUM_INSPECTION_PAGES: usize = 32;

impl LocalTerminalService {
    pub(super) fn execute_workspace_close_if_idle(
        &self,
        command: WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        let deadline = Instant::now().checked_add(INSPECTION_BUDGET)?;
        let (mut proof, handle) = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let prepared = self
                .coordinator
                .lock()
                .ok()?
                .prepare_workspace_idle_close(command.clone(), placement(handle.as_ref()))?;
            let proof = match prepared {
                Ok(proof) => proof,
                Err(result) => return Some(result),
            };
            (proof, handle)
        };
        let mut remaining_pages = MAXIMUM_INSPECTION_PAGES;
        for member in &mut proof {
            let WorkbenchTabCloseMemberRuntime::Runtime { target, .. } = member else {
                continue;
            };
            let target = *target;
            if let Some(observation) = handle.as_ref().and_then(|handle| {
                self.inspect_idle_close_target(
                    handle,
                    &command,
                    target,
                    deadline,
                    &mut remaining_pages,
                )
            }) {
                *member = WorkbenchTabCloseMemberRuntime::IdleRuntime {
                    target,
                    observed_phase: observation.phase,
                };
            } else {
                // A single unknown member makes the whole scope confirmation-required.
                break;
            }
        }
        let result = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            self.coordinator
                .lock()
                .ok()?
                .execute_workspace_close_with_idle_proof(
                    command,
                    placement(handle.as_ref()),
                    self.close_maintenance.available(),
                    Instant::now(),
                    &proof,
                )
        };
        self.close_maintenance.notify();
        result
    }

    fn inspect_idle_close_target(
        &self,
        handle: &WorkerRequestHandle,
        command: &WorkbenchCommand,
        target: WorkbenchTabCloseRuntimeTarget,
        deadline: Instant,
        remaining_pages: &mut usize,
    ) -> Option<LifecycleTargetObservation> {
        let expiry = lease_expiry()?;
        let mut cursor = None;
        let mut generation = None;
        let mut found = None;
        loop {
            if Instant::now() >= deadline || *remaining_pages == 0 {
                return None;
            }
            *remaining_pages -= 1;
            let request = self
                .coordinator
                .lock()
                .ok()?
                .prepare_workspace_idle_inspection(target, &command.request_id, cursor, expiry)?;
            let result = handle.submit_lifecycle(request).ok()?.wait().ok()?;
            if result.request() != request || Instant::now() >= deadline {
                return None;
            }
            let TerminalLifecycleOutcome::InventoryPage(page) = result.outcome() else {
                return None;
            };
            if generation.is_some_and(|generation| generation != page.inventory_generation)
                || (found.is_some() && page.target.is_some())
            {
                return None;
            }
            generation = Some(page.inventory_generation);
            found = found.or(page.target);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return found.filter(|observation| permits_unconfirmed_close(*observation));
            }
        }
    }
}

fn permits_unconfirmed_close(observation: LifecycleTargetObservation) -> bool {
    match observation.phase {
        LifecycleTerminalPhase::AwaitingStart
        | LifecycleTerminalPhase::Exited { .. }
        | LifecycleTerminalPhase::Failed => true,
        LifecycleTerminalPhase::Running {
            terminate_requested: false,
        } => observation.close_activity == LifecycleCloseActivity::Idle,
        LifecycleTerminalPhase::Running {
            terminate_requested: true,
        }
        | LifecycleTerminalPhase::Finishing { .. }
        | LifecycleTerminalPhase::ContainingFailure => false,
    }
}
