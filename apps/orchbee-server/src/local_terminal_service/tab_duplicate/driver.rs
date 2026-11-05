//! Ordered copying. Each copy is an ordinary duplicate Tab creation, admitted and committed under
//! the Session gate and launched outside it; this owner records each copy and stops at the first
//! member it cannot copy with a started terminal. Nothing is ever created twice.
use super::LocalTerminalService;
use crate::{
    session_coordinator::{PreparedTerminalTabCreate, SessionCoordinator},
    terminal_coordinator::CreateInputs,
};
use orchbee_server::TabDuplicatePlan;
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, TabCreationPlacement, TabDuplicateCopy,
    TabDuplicateFailure as Failure, TabDuplicateStatus as Status, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchRequestId, WorkbenchTerminalOutcome,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

pub(super) fn run(
    service: &LocalTerminalService,
    plan: &mut TabDuplicatePlan,
    stop: &AtomicBool,
    deadline: Instant,
) -> Status {
    let mut driver = Driver {
        service,
        plan,
        stop,
        deadline,
    };
    match driver.execute() {
        Ok(()) => Status::Duplicated {},
        Err(reason) => Status::Failed { reason },
    }
}

struct Driver<'a> {
    service: &'a LocalTerminalService,
    plan: &'a mut TabDuplicatePlan,
    stop: &'a AtomicBool,
    deadline: Instant,
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
            match self.copy(index, &session_id)? {
                WorkbenchTerminalOutcome::Started { .. } => {}
                WorkbenchTerminalOutcome::Unavailable { .. }
                | WorkbenchTerminalOutcome::Failed { .. } => {
                    return Err(Failure::TerminalNotStarted);
                }
                WorkbenchTerminalOutcome::OutcomeUnknown {} => return Err(Failure::OutcomeUnknown),
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

    /// Creates member `index`'s copy once and returns its terminal outcome. The committed copy is
    /// published under the gate that committed it, before its launch leaves the gate.
    fn copy(
        &mut self,
        index: usize,
        session_id: &SessionId,
    ) -> Result<WorkbenchTerminalOutcome, Failure> {
        self.check()?;
        let service = self.service;
        let request_id = identity()?;
        let candidate = super::super::create_reference();
        let expiry = super::super::lease_expiry();
        let capture = service.launch_capture();
        let (prepared, handle) = {
            let workers = service.workers.lock().map_err(|_| Failure::Internal)?;
            let handle = workers.request_handle();
            let inputs = CreateInputs::admitted_by(candidate, expiry, handle.as_ref(), capture);
            let mut coordinator = service.coordinator.lock().map_err(|_| Failure::Internal)?;
            self.check()?;
            let authority = coordinator.worktree_authority().ok_or(Failure::Internal)?;
            let terminal_context_id = self.plan.terminal_context_id.clone();
            let tab_id = self.plan.members[index].tab_id.clone();
            // A renamed source is copied under its current name; a gone one stops the operation.
            let label = authority
                .tab_duplicate_source(&terminal_context_id, &tab_id)
                .ok_or(Failure::SourceUnavailable)?;
            let command = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: session_id.clone(),
                expected_revision: authority.snapshot().revision,
                action: WorkbenchAction::TabCreate {
                    placement: Some(TabCreationPlacement::Duplicate { tab_id }),
                    automatic_label: false,
                    terminal_context_id,
                    label,
                    geometry: self.plan.geometry,
                    launch: None,
                    profile: None,
                    options: Box::default(),
                },
            };
            let prepared = coordinator.prepare_terminal_tab_create(command, inputs);
            if let PreparedTerminalTabCreate::Dispatch(pending) = &prepared {
                let (tab_id, revision) = pending.created();
                self.plan.members[index].copy = Some(TabDuplicateCopy {
                    tab_id: tab_id.clone(),
                    revision,
                    terminal: None,
                });
                self.publish(&mut coordinator)?;
            }
            (prepared, handle)
        };
        let committed = self.plan.members[index].copy.is_some();
        // A committed copy whose settlement fails has an unknown terminal, never a second try.
        let result = service
            .dispatch_terminal_tab_create(prepared, handle)
            .map_err(|_| Failure::OutcomeUnknown)?;
        let WorkbenchCommandOutcome::TabCreated {
            revision,
            tab_id,
            terminal_outcome,
            ..
        } = result.outcome
        else {
            return Err(if committed {
                Failure::OutcomeUnknown
            } else {
                Failure::CopyRefused
            });
        };
        if self.plan.members[index]
            .copy
            .as_ref()
            .is_some_and(|copy| copy.tab_id != tab_id)
        {
            return Err(Failure::Internal);
        }
        self.plan.members[index].copy = Some(TabDuplicateCopy {
            tab_id,
            revision,
            terminal: Some(terminal_outcome.clone()),
        });
        let mut coordinator = service.coordinator.lock().map_err(|_| Failure::Internal)?;
        self.publish(&mut coordinator)?;
        Ok(terminal_outcome)
    }

    fn publish(&self, coordinator: &mut SessionCoordinator) -> Result<(), Failure> {
        if coordinator
            .worktree_authority()
            .ok_or(Failure::Internal)?
            .publish_tab_duplicate(self.plan, Status::Pending {})
        {
            Ok(())
        } else {
            Err(Failure::Internal)
        }
    }
}

fn identity() -> Result<WorkbenchRequestId, Failure> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Internal)?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Failure::Internal)
}
