//! Fresh complete inventory preflight grants only guarded Close admission.
//! No Session/registry lock spans IPC; no inspection claims Worker Close ownership.
//!
//! A conditional Tab or Panel Close also reads each live terminal's own evidence through the
//! process observer within one budget, then re-enters final admission with that proof.
use super::super::{LocalTerminalService, lease_expiry, placement};
use super::evidence::CloseFacts;
use crate::session_coordinator::{IdleCloseMember, ProcessObservationTarget};
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
/// How long a conditional Tab or Panel Close reads before it decides with what it has. This
/// matches the Worker's own bound on one process sample, so a slow answer still arrives.
const LEAF_EVIDENCE_BUDGET: Duration = Duration::from_secs(1);

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
            if let Some(observation) = handle
                .as_ref()
                .and_then(|handle| {
                    self.inspect_close_target(
                        handle,
                        &command,
                        target,
                        deadline,
                        &mut remaining_pages,
                    )
                })
                .filter(|observation| permits_unconfirmed_close(*observation))
            {
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

    /// The Server decides whether closing one Tab or Panel asks first. The Host inventory says
    /// which runtimes ended; each live one is read, and closes unasked only when its own evidence
    /// shows nothing beyond an idle shell.
    pub(super) fn execute_terminal_tab_close_if_idle(
        &self,
        command: WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        self.execute_terminal_tab_close_if_idle_with(command, |targets, deadline| {
            self.request_close_facts(targets, deadline)
        })
    }

    /// `read` returns one entry per live target, in order, holding what was read by `deadline`.
    pub(super) fn execute_terminal_tab_close_if_idle_with(
        &self,
        command: WorkbenchCommand,
        read: impl FnOnce(Vec<ProcessObservationTarget>, Instant) -> Vec<CloseFacts>,
    ) -> Option<WorkbenchCommandResult> {
        let deadline = Instant::now().checked_add(LEAF_EVIDENCE_BUDGET)?;
        let (members, handle) = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let prepared = self
                .coordinator
                .lock()
                .ok()?
                .prepare_terminal_tab_idle_close(command.clone(), placement(handle.as_ref()))?;
            match prepared {
                Ok(members) => (members, handle),
                Err(result) => return Some(result),
            }
        };
        let proof = self.leaf_idle_proof(&command, members, handle.as_ref(), deadline, read);
        let result = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            self.coordinator
                .lock()
                .ok()?
                .execute_terminal_tab_close_with_idle_proof(
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

    /// One proof entry per member. A runtime stays unproven, and the close asks, unless the
    /// inventory shows it ended or its read shows nothing beyond an idle shell.
    fn leaf_idle_proof(
        &self,
        command: &WorkbenchCommand,
        members: Vec<IdleCloseMember>,
        handle: Option<&WorkerRequestHandle>,
        deadline: Instant,
        read: impl FnOnce(Vec<ProcessObservationTarget>, Instant) -> Vec<CloseFacts>,
    ) -> Vec<WorkbenchTabCloseMemberRuntime> {
        let mut proof = Vec::with_capacity(members.len());
        let mut live = Vec::new();
        let mut remaining_pages = MAXIMUM_INSPECTION_PAGES;
        let mut inspected = true;
        for (member, observation) in members {
            let index = proof.len();
            proof.push(member);
            let WorkbenchTabCloseMemberRuntime::Runtime { target, .. } = proof[index] else {
                continue;
            };
            let Some(phase) = inspected
                .then(|| {
                    self.inspect_close_target(
                        handle?,
                        command,
                        target,
                        deadline,
                        &mut remaining_pages,
                    )
                })
                .flatten()
                .map(|observed| observed.phase)
            else {
                // One unknown member asks for the whole scope; nothing more is worth reading.
                inspected = false;
                continue;
            };
            if matches!(
                phase,
                LifecycleTerminalPhase::Exited { .. } | LifecycleTerminalPhase::Failed
            ) {
                proof[index] = WorkbenchTabCloseMemberRuntime::QuietRuntime {
                    target,
                    observed_phase: phase,
                };
            } else if let Some(observation) = observation {
                live.push((index, target, phase, observation));
            } else {
                inspected = false;
            }
        }
        if !inspected || live.is_empty() {
            return proof;
        }
        let facts = read(
            live.iter()
                .map(|(.., observation)| observation.clone())
                .collect(),
            deadline,
        );
        for ((index, target, phase, _), facts) in live.into_iter().zip(facts) {
            if facts.runs_nothing() {
                proof[index] = WorkbenchTabCloseMemberRuntime::QuietRuntime {
                    target,
                    observed_phase: phase,
                };
            }
        }
        proof
    }

    fn inspect_close_target(
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
            let request = self.coordinator.lock().ok()?.prepare_close_inspection(
                target,
                &command.request_id,
                cursor,
                expiry,
            )?;
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
                return found;
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
