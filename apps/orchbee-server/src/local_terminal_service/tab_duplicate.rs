//! Tab duplication transport and its retained driver. A Start is admitted under the Session gate
//! and copied by one owner thread; reads never start the driver.
use super::LocalTerminalService;
use orchbee_server::WorkbenchGrant;
use orchbee_workbench_protocol::{
    ProtocolVersion, TabDuplicateAction as Action, TabDuplicateOutcome as Outcome,
    TabDuplicateUnavailable as Unavailable, WorkbenchTabDuplicateRequest,
    WorkbenchTabDuplicateResult,
};
#[cfg(windows)]
use std::io;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
mod driver;

/// How long one operation may copy its members before it stops.
const DEADLINE: Duration = Duration::from_secs(120);

impl LocalTerminalService {
    pub(crate) fn execute_tab_duplicate(
        self: &Arc<Self>,
        request: WorkbenchTabDuplicateRequest,
    ) -> WorkbenchTabDuplicateResult {
        let start = matches!(request.action, Action::Start { .. });
        if start && self.tab_duplicate_maintenance.ensure(self).is_err() {
            return unavailable(&request);
        }
        let result = self.answer_tab_duplicate(request, self.tab_duplicate_maintenance.available());
        if start {
            self.tab_duplicate_maintenance.notify();
        }
        result
    }

    /// Answers a Read or a known Start, or retains a new Start for the driver.
    pub(super) fn answer_tab_duplicate(
        &self,
        request: WorkbenchTabDuplicateRequest,
        owner_available: bool,
    ) -> WorkbenchTabDuplicateResult {
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return unavailable(&request);
        };
        let Some(authority) = coordinator.worktree_authority() else {
            return unavailable(&request);
        };
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
        authority.tab_duplicate(Some(&grant), request, owner_available)
    }

    #[cfg(windows)]
    pub(crate) fn stop_tab_duplicates_before(&self, deadline: Instant) -> io::Result<()> {
        self.tab_duplicate_maintenance.stop_before(deadline)
    }

    pub(super) fn tab_duplicate_turn(&self, stop: &AtomicBool) {
        self.run_tab_duplicates(stop, DEADLINE);
    }

    /// Runs every pending operation in admission order, each for at most `budget`, and publishes
    /// how it ended. A stopping Session fails every operation it reaches.
    pub(super) fn run_tab_duplicates(&self, stop: &AtomicBool, budget: Duration) {
        loop {
            let plan = self.coordinator.lock().ok().and_then(|mut coordinator| {
                coordinator.worktree_authority()?.claim_tab_duplicate()
            });
            let Some(mut plan) = plan else {
                return;
            };
            let status = driver::run(self, &mut plan, stop, Instant::now() + budget);
            if let Ok(mut coordinator) = self.coordinator.lock()
                && let Some(authority) = coordinator.worktree_authority()
                && !authority.publish_tab_duplicate(&plan, status)
            {
                authority.abandon_tab_duplicate(&plan.operation_id);
            }
        }
    }
}

fn unavailable(request: &WorkbenchTabDuplicateRequest) -> WorkbenchTabDuplicateResult {
    WorkbenchTabDuplicateResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        operation_id: request.action.operation_id().clone(),
        outcome: Outcome::Unavailable {
            reason: Unavailable::ServiceUnavailable,
        },
    }
}

#[cfg(test)]
mod tests;
