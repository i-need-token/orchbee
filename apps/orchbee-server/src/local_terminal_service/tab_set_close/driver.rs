//! Ordered set submission. Each member's own Tab Close owns its disposal, publication and Resume;
//! this owner submits one member after another and keeps what it observed of each.
use super::{CloseFactsRead, LocalTerminalService};
use crate::{
    local_terminal_service::{placement, tab_close::idle::LEAF_EVIDENCE_BUDGET, worker_identity},
    session_coordinator::SessionCoordinator,
    worker_obcn_carrier::WorkerRequestHandle,
};
use orchbee_server::{TabSetCloseMemberState as MemberState, TabSetClosePlan};
use orchbee_workbench_protocol::{
    CloseOperationOutcome, CloseOperationRead, CloseOperationTarget, ProtocolVersion, SessionId,
    TabSetCloseChild, TabSetCloseCondition as Condition, TabSetCloseFailure as Failure,
    TabSetCloseObservation, TabSetCloseStatus as Status, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchRejectionReason, WorkbenchRequestId,
    WorkbenchTabClosePublicationReason as PublicationReason, WorkbenchTabCloseStatus,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// How often the set reads the Close of the member it waits for.
const OBSERVATION_INTERVAL: Duration = Duration::from_millis(50);

pub(super) fn run(
    service: &LocalTerminalService,
    plan: &mut TabSetClosePlan,
    stop: &AtomicBool,
    deadline: Instant,
    read: &CloseFactsRead<'_>,
) -> Status {
    let mut driver = Driver {
        service,
        plan,
        stop,
        deadline,
        read,
    };
    match driver.execute() {
        Ok(()) => Status::Closed {},
        Err(reason) => Status::Failed { reason },
    }
}

struct Driver<'a> {
    service: &'a LocalTerminalService,
    plan: &'a mut TabSetClosePlan,
    stop: &'a AtomicBool,
    deadline: Instant,
    read: &'a CloseFactsRead<'a>,
}

impl Driver<'_> {
    fn execute(&mut self) -> Result<(), Failure> {
        let session_id = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal)?
            .worktree_authority()
            .ok_or(Failure::Internal)?
            .session_id()
            .clone();
        for index in 0..self.plan.members.len() {
            self.check()?;
            if self.plan.members[index].member.is_done() {
                continue;
            }
            if self.plan.members[index].member.close.is_none()
                && !self.submit(index, &session_id)?
            {
                continue;
            }
            loop {
                self.check()?;
                let status = {
                    let mut coordinator = self
                        .service
                        .coordinator
                        .lock()
                        .map_err(|_| Failure::Internal)?;
                    self.observe(&mut coordinator, index, &session_id)?
                };
                match status {
                    WorkbenchTabCloseStatus::Closed { .. } => break,
                    WorkbenchTabCloseStatus::Closing { .. }
                    | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        reason:
                            PublicationReason::Preparing | PublicationReason::TargetCleanupPending,
                        resumable: false,
                    } => thread::sleep(OBSERVATION_INTERVAL),
                    WorkbenchTabCloseStatus::OutcomeUnknown { .. } => {
                        return Err(Failure::OutcomeUnknown);
                    }
                    // Only the member's own Resume continues it from here.
                    WorkbenchTabCloseStatus::CleanupBlocked { .. }
                    | WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. } => {
                        return Err(Failure::ChildBlocked);
                    }
                }
            }
        }
        Ok(())
    }

    fn check(&self) -> Result<(), Failure> {
        if self.stop.load(Ordering::Acquire) {
            return Err(Failure::SessionStopping);
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::Deadline);
        }
        Ok(())
    }

    /// Submits member `index`'s own Close, or publishes that its Tab is already gone. Returns
    /// whether a child now runs.
    fn submit(&mut self, index: usize, session_id: &SessionId) -> Result<bool, Failure> {
        let request_id = identity()?;
        let service = self.service;
        let read = self.read;
        let proof = match self.plan.condition {
            Condition::Confirmed => None,
            Condition::IfIdle => {
                let (members, handle) = {
                    let workers = service.workers.lock().map_err(|_| Failure::Internal)?;
                    let handle = workers.request_handle();
                    self.check_worker(handle.as_ref())?;
                    let mut coordinator =
                        service.coordinator.lock().map_err(|_| Failure::Internal)?;
                    self.check()?;
                    if !self.unchanged(&mut coordinator, index)? {
                        return Ok(false);
                    }
                    // A runtime this owner cannot resolve is not read: the child's own admission
                    // then joins a Close of the Tab that is already open, or asks.
                    let members = coordinator
                        .tab_set_member_idle_close(
                            &self.plan.members[index].presentations,
                            placement(handle.as_ref()),
                        )
                        .ok();
                    (members, handle)
                };
                // The member's terminals may have started something since the set was admitted.
                members.map(|members| {
                    service.idle_proof(
                        &request_id,
                        members,
                        handle.as_ref(),
                        Instant::now() + LEAF_EVIDENCE_BUDGET,
                        read,
                    )
                })
            }
        };
        {
            let workers = service.workers.lock().map_err(|_| Failure::Internal)?;
            let handle = workers.request_handle();
            self.check_worker(handle.as_ref())?;
            let mut coordinator = service.coordinator.lock().map_err(|_| Failure::Internal)?;
            self.check()?;
            if !self.unchanged(&mut coordinator, index)? {
                return Ok(false);
            }
            if !service.close_maintenance.available() {
                return Err(Failure::ChildAdmission);
            }
            let terminal_context_id = self.plan.terminal_context_id.clone();
            let tab_id = self.plan.members[index].member.tab_id.clone();
            let command = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: session_id.clone(),
                expected_revision: coordinator
                    .worktree_authority()
                    .ok_or(Failure::Internal)?
                    .snapshot()
                    .revision,
                action: match self.plan.condition {
                    Condition::Confirmed => WorkbenchAction::TabClose {
                        terminal_context_id,
                        tab_id,
                    },
                    Condition::IfIdle => WorkbenchAction::TabCloseIfIdle {
                        terminal_context_id,
                        tab_id,
                    },
                },
            };
            let result = coordinator
                .execute_tab_set_member_close(
                    command,
                    placement(handle.as_ref()),
                    service.close_maintenance.available(),
                    Instant::now(),
                    (&self.plan.operation_id, index),
                    proof.as_deref(),
                )
                .ok_or(Failure::Internal)?;
            let close_request_id = match result.outcome {
                WorkbenchCommandOutcome::TabCloseAccepted {
                    close_request_id, ..
                } => close_request_id,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::ConfirmationRequired,
                    ..
                } => return Err(Failure::ChildBusy),
                // The set proved its token under this same gate.
                WorkbenchCommandOutcome::Denied {} => return Err(Failure::Internal),
                // Every other answer refused the child without running it.
                _ => return Err(Failure::ChildAdmission),
            };
            self.plan.members[index].member.close = Some(TabSetCloseChild {
                request_id: close_request_id,
                observation: None,
            });
            self.publish(&mut coordinator)?;
            // The accepted identity is already retained if this read finds a contradiction.
            self.observe(&mut coordinator, index, session_id)?;
        }
        service.close_maintenance.notify();
        Ok(true)
    }

    /// Whether member `index` still holds exactly its frozen terminals. A gone Tab is published
    /// absent and counts as done.
    fn unchanged(
        &mut self,
        coordinator: &mut SessionCoordinator,
        index: usize,
    ) -> Result<bool, Failure> {
        let state = coordinator
            .worktree_authority()
            .ok_or(Failure::Internal)?
            .tab_set_close_member_state(&self.plan.operation_id, index)
            .ok_or(Failure::Internal)?;
        match state {
            MemberState::Unchanged => Ok(true),
            MemberState::Changed => Err(Failure::SourceChanged),
            MemberState::Absent => {
                self.plan.members[index].member.absent = true;
                self.publish(coordinator)?;
                Ok(false)
            }
        }
    }

    fn observe(
        &mut self,
        coordinator: &mut SessionCoordinator,
        index: usize,
        session_id: &SessionId,
    ) -> Result<WorkbenchTabCloseStatus, Failure> {
        let member = &self.plan.members[index].member;
        let child = member
            .close
            .as_ref()
            .ok_or(Failure::Internal)?
            .request_id
            .clone();
        let result = coordinator
            .read_close_operation(CloseOperationRead {
                protocol_version: ProtocolVersion,
                request_id: identity()?,
                session_id: session_id.clone(),
                operation_id: child.clone(),
            })
            .ok_or(Failure::Internal)?;
        let CloseOperationOutcome::Available {
            target:
                CloseOperationTarget::Tab {
                    terminal_context_id,
                    tab_id,
                },
            generation,
            status,
        } = result.outcome
        else {
            return Err(Failure::OutcomeUnknown);
        };
        if terminal_context_id != self.plan.terminal_context_id || tab_id != member.tab_id {
            return Err(Failure::OutcomeUnknown);
        }
        self.plan.members[index].member.close = Some(TabSetCloseChild {
            request_id: child,
            observation: Some(TabSetCloseObservation {
                attempt_generation: generation,
                status: status.clone(),
            }),
        });
        self.publish(coordinator)?;
        Ok(status)
    }

    fn publish(&self, coordinator: &mut SessionCoordinator) -> Result<(), Failure> {
        if coordinator
            .worktree_authority()
            .ok_or(Failure::Internal)?
            .publish_tab_set_close(self.plan, Status::Pending {})
        {
            Ok(())
        } else {
            Err(Failure::Internal)
        }
    }

    fn check_worker(&self, handle: Option<&WorkerRequestHandle>) -> Result<(), Failure> {
        let current = worker_identity(handle);
        if current != self.plan.worker {
            return Err(if current.is_none() {
                Failure::WorkerUnavailable
            } else {
                Failure::SourceChanged
            });
        }
        Ok(())
    }
}

fn identity() -> Result<WorkbenchRequestId, Failure> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Internal)?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Failure::Internal)
}
