//! Tab set Close transport and its retained submitter. An if-idle Start reads every member's
//! terminals once, outside the gates; reads never start the submitter.
use super::{
    LocalTerminalService, placement,
    tab_close::{evidence::CloseFacts, idle::WORKSPACE_EVIDENCE_BUDGET},
    worker_identity,
};
use crate::session_coordinator::{ProcessObservationTarget, TabSetCloseStart};
use orchbee_workbench_protocol::{
    ProtocolVersion, TabSetCloseAction as Action, TabSetCloseOutcome as Outcome,
    TabSetCloseUnavailable as Unavailable, WorkbenchTabSetCloseRequest, WorkbenchTabSetCloseResult,
};
#[cfg(windows)]
use std::io;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
mod driver;

/// How long one set may run its members' Closes before it stops.
const SET_DEADLINE: Duration = Duration::from_secs(120);

/// What the evidence read of a conditional Close returns: one entry per live target, in order.
pub(super) type CloseFactsRead<'a> =
    dyn Fn(Vec<ProcessObservationTarget>, Instant) -> Vec<CloseFacts> + 'a;

impl LocalTerminalService {
    pub(crate) fn execute_tab_set_close(
        self: &Arc<Self>,
        request: WorkbenchTabSetCloseRequest,
    ) -> WorkbenchTabSetCloseResult {
        let start = matches!(request.action, Action::Start { .. });
        if start && self.tab_set_maintenance.ensure(self).is_err() {
            return unavailable(&request);
        }
        let result = self.start_tab_set_close(
            request,
            self.tab_set_maintenance.available(),
            &|targets, deadline| self.request_close_facts(targets, deadline),
        );
        if start {
            self.tab_set_maintenance.notify();
        }
        result
    }

    /// Answers a set request. An if-idle Start reads every member's terminals within one budget and
    /// is admitted only when final admission finds each exact runtime proven quiet or never created.
    pub(super) fn start_tab_set_close(
        &self,
        request: WorkbenchTabSetCloseRequest,
        owner_available: bool,
        read: &CloseFactsRead<'_>,
    ) -> WorkbenchTabSetCloseResult {
        let Some(deadline) = Instant::now().checked_add(WORKSPACE_EVIDENCE_BUDGET) else {
            return unavailable(&request);
        };
        let (members, handle) = {
            let workers = self.workers.lock().ok();
            let handle = workers
                .as_ref()
                .and_then(|workers| workers.request_handle());
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return unavailable(&request);
            };
            match coordinator.start_tab_set_close(
                request.clone(),
                placement(handle.as_ref()),
                worker_identity(handle.as_ref()),
                owner_available,
            ) {
                Some(TabSetCloseStart::Answered(result)) => return result,
                Some(TabSetCloseStart::Read(members)) => (members, handle),
                None => return unavailable(&request),
            }
        };
        let proof = self.idle_proof(
            request.action.operation_id(),
            members,
            handle.as_ref(),
            deadline,
            read,
        );
        let workers = self.workers.lock().ok();
        let handle = workers
            .as_ref()
            .and_then(|workers| workers.request_handle());
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return unavailable(&request);
        };
        coordinator
            .finish_tab_set_close_if_idle(
                request.clone(),
                placement(handle.as_ref()),
                worker_identity(handle.as_ref()),
                owner_available,
                &proof,
            )
            .unwrap_or_else(|| unavailable(&request))
    }

    #[cfg(windows)]
    pub(crate) fn stop_tab_sets_before(&self, deadline: Instant) -> io::Result<()> {
        self.tab_set_maintenance.stop_before(deadline)
    }

    pub(super) fn tab_set_close_turn(&self, stop: &AtomicBool) {
        self.run_tab_sets(stop, SET_DEADLINE, &|targets, deadline| {
            self.request_close_facts(targets, deadline)
        });
    }

    /// Runs every pending set in admission order, each for at most `budget`, and publishes how it
    /// ended. A stopping Session fails every set it reaches.
    pub(super) fn run_tab_sets(
        &self,
        stop: &AtomicBool,
        budget: Duration,
        read: &CloseFactsRead<'_>,
    ) {
        loop {
            let plan = self.coordinator.lock().ok().and_then(|mut coordinator| {
                coordinator.worktree_authority()?.claim_tab_set_close()
            });
            let Some(mut plan) = plan else {
                return;
            };
            let status = driver::run(self, &mut plan, stop, Instant::now() + budget, read);
            if let Ok(mut coordinator) = self.coordinator.lock()
                && let Some(authority) = coordinator.worktree_authority()
                && !authority.publish_tab_set_close(&plan, status)
            {
                authority.abandon_tab_set_close(&plan.operation_id);
            }
        }
    }
}

fn unavailable(request: &WorkbenchTabSetCloseRequest) -> WorkbenchTabSetCloseResult {
    WorkbenchTabSetCloseResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        operation_id: request.action.operation_id().clone(),
        outcome: Outcome::Unavailable {
            reason: Unavailable::ServiceUnavailable,
        },
    }
}
