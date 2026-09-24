//! Ordered group submission: Close owns physical disposal, this owner preserves child observations.
use super::{LocalTerminalService, worker_identity};
use crate::{local_terminal_service::placement, site_coordinator::SiteCoordinator};
use orchbee_server::WorkspaceGroupClosePlan;
use orchbee_workbench_protocol::{
    ProtocolVersion, SiteId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchRequestId, WorkbenchTabCloseStatus, WorkbenchWorkspaceCloseRead,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkspaceGroupCloseChild,
    WorkspaceGroupCloseFailure as Failure, WorkspaceGroupCloseObservation,
    WorkspaceGroupCloseStatus as Status,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(
    service: &LocalTerminalService,
    plan: &mut WorkspaceGroupClosePlan,
    stop: &AtomicBool,
) -> Status {
    let mut driver = Driver {
        service,
        plan,
        stop,
        deadline: Instant::now() + Duration::from_secs(120),
    };
    match driver.execute() {
        Ok(()) => Status::Closed {},
        Err(reason) => Status::Failed {
            reason,
            resumable: reason.permits_resume()
                && driver.plan.members.iter().all(|member| {
                    member
                        .member
                        .close
                        .as_ref()
                        .is_none_or(WorkspaceGroupCloseChild::can_observe_or_resume)
                }),
        },
    }
}
struct Driver<'a> {
    service: &'a LocalTerminalService,
    plan: &'a mut WorkspaceGroupClosePlan,
    stop: &'a AtomicBool,
    deadline: Instant,
}
impl Driver<'_> {
    fn execute(&mut self) -> Result<(), Failure> {
        let site_id = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal)?
            .worktree_authority()
            .ok_or(Failure::Internal)?
            .site_id()
            .clone();
        for index in 0..self.plan.members.len() {
            self.check()?;
            if self.plan.members[index]
                .member
                .close
                .as_ref()
                .is_some_and(WorkspaceGroupCloseChild::is_closed)
            {
                continue;
            }
            if self.plan.members[index].member.close.is_none() {
                self.admit(index, &site_id)?;
            }
            let mut may_resume = self.plan.attempt > 1;
            loop {
                self.check()?;
                let observation = {
                    let mut coordinator = self
                        .service
                        .coordinator
                        .lock()
                        .map_err(|_| Failure::Internal)?;
                    self.observe(&mut coordinator, index, &site_id)?
                };
                match observation.status {
                    WorkbenchTabCloseStatus::Closed { .. } => break,
                    WorkbenchTabCloseStatus::Closing { .. } => {
                        thread::sleep(Duration::from_millis(50))
                    }
                    WorkbenchTabCloseStatus::CleanupBlocked {
                        resumable: true, ..
                    }
                    | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        resumable: true,
                        ..
                    } if may_resume => {
                        may_resume = false;
                        self.resume(index, &site_id, observation)?;
                    }
                    WorkbenchTabCloseStatus::OutcomeUnknown { .. } => {
                        return Err(Failure::OutcomeUnknown);
                    }
                    _ => return Err(Failure::ChildBlocked),
                }
            }
        }
        Ok(())
    }
    fn check(&self) -> Result<(), Failure> {
        if self.stop.load(Ordering::Acquire) {
            return Err(Failure::SiteStopping);
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::Deadline);
        }
        Ok(())
    }
    fn admit(&mut self, index: usize, site_id: &SiteId) -> Result<(), Failure> {
        let request_id = identity()?;
        let service = self.service;
        {
            let workers = service.workers.lock().map_err(|_| Failure::Internal)?;
            let handle = workers.request_handle();
            self.check_worker(handle.as_ref())?;
            let mut coordinator = service.coordinator.lock().map_err(|_| Failure::Internal)?;
            self.check()?;
            if !service.close_maintenance.available() {
                return Err(Failure::ChildAdmission);
            }
            let authority = coordinator.worktree_authority().ok_or(Failure::Internal)?;
            let member = &self.plan.members[index].member;
            let command = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id,
                site_id: site_id.clone(),
                expected_revision: authority.snapshot().revision,
                action: WorkbenchAction::WorkspaceClose {
                    project_id: member.project_id.clone(),
                    workspace_id: member.workspace_id.clone(),
                },
            };
            let result = coordinator
                .execute_workspace_group_member_close(
                    command,
                    placement(handle.as_ref()),
                    service.close_maintenance.available(),
                    Instant::now(),
                    (&self.plan.operation_id, self.plan.attempt, index),
                )
                .ok_or(Failure::Internal)?;
            let close_request_id = match result.outcome {
                WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                    close_request_id, ..
                } => close_request_id,
                WorkbenchCommandOutcome::Denied {}
                | WorkbenchCommandOutcome::Rejected {
                    reason:
                        orchbee_workbench_protocol::WorkbenchRejectionReason::ProjectUnavailable
                        | orchbee_workbench_protocol::WorkbenchRejectionReason::WorkspaceUnavailable
                        | orchbee_workbench_protocol::WorkbenchRejectionReason::SiteMismatch,
                    ..
                } => return Err(Failure::SourceChanged),
                _ => return Err(Failure::ChildAdmission),
            };
            self.plan.members[index].member.close = Some(WorkspaceGroupCloseChild {
                request_id: close_request_id,
                observation: None,
            });
            self.publish(&mut coordinator)?;
            // Accepted identity is already retained if this read detects an internal contradiction.
            self.observe(&mut coordinator, index, site_id)?;
        }
        service.close_maintenance.notify();
        Ok(())
    }
    fn observe(
        &mut self,
        coordinator: &mut SiteCoordinator,
        index: usize,
        site_id: &SiteId,
    ) -> Result<WorkspaceGroupCloseObservation, Failure> {
        let member = &self.plan.members[index].member;
        let expected = member
            .close
            .as_ref()
            .ok_or(Failure::Internal)?
            .request_id
            .clone();
        let result = coordinator
            .read_workspace_close(WorkbenchWorkspaceCloseRead {
                protocol_version: ProtocolVersion,
                request_id: identity()?,
                site_id: site_id.clone(),
                project_id: member.project_id.clone(),
                workspace_id: member.workspace_id.clone(),
            })
            .ok_or(Failure::Internal)?;
        let WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id,
            attempt_generation,
            status,
            ..
        } = result.outcome
        else {
            return Err(Failure::OutcomeUnknown);
        };
        if close_request_id != expected {
            return Err(Failure::OutcomeUnknown);
        }
        let observation = WorkspaceGroupCloseObservation {
            attempt_generation,
            status,
        };
        self.plan.members[index].member.close = Some(WorkspaceGroupCloseChild {
            request_id: expected,
            observation: Some(observation.clone()),
        });
        self.publish(coordinator)?;
        Ok(observation)
    }
    fn publish(&self, coordinator: &mut SiteCoordinator) -> Result<(), Failure> {
        if !coordinator
            .worktree_authority()
            .ok_or(Failure::Internal)?
            .publish_workspace_group_close(self.plan, Status::Pending {})
        {
            return Err(Failure::Internal);
        }
        Ok(())
    }
    fn check_worker(
        &self,
        handle: Option<&crate::worker_obcn_carrier::WorkerRequestHandle>,
    ) -> Result<(), Failure> {
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
    fn resume(
        &self,
        index: usize,
        site_id: &SiteId,
        observation: WorkspaceGroupCloseObservation,
    ) -> Result<(), Failure> {
        let request_id = identity()?;
        let member = &self.plan.members[index].member;
        let close_request_id = member
            .close
            .as_ref()
            .ok_or(Failure::Internal)?
            .request_id
            .clone();
        let result = {
            let workers = self.service.workers.lock().map_err(|_| Failure::Internal)?;
            let handle = workers.request_handle();
            self.check_worker(handle.as_ref())?;
            let mut coordinator = self
                .service
                .coordinator
                .lock()
                .map_err(|_| Failure::Internal)?;
            self.check()?;
            let cleanup_available = self.service.controllers.lock().is_ok();
            coordinator
                .resume_workspace_close(
                    WorkbenchWorkspaceCloseResume {
                        protocol_version: ProtocolVersion,
                        request_id,
                        site_id: site_id.clone(),
                        project_id: member.project_id.clone(),
                        workspace_id: member.workspace_id.clone(),
                        close_request_id,
                        expected_attempt_generation: observation.attempt_generation,
                    },
                    placement(handle.as_ref()),
                    self.service.close_maintenance.available(),
                    cleanup_available,
                    Instant::now(),
                )
                .ok_or(Failure::Internal)?
        };
        self.service.close_maintenance.notify();
        match result.outcome {
            WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
            | WorkbenchWorkspaceCloseResumeOutcome::Stale { .. } => Ok(()),
            _ => Err(Failure::ChildBlocked),
        }
    }
}
fn identity() -> Result<WorkbenchRequestId, Failure> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Internal)?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Failure::Internal)
}
