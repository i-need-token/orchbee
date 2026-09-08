use super::*;
use crate::terminal_session::state::MembershipCleanup;

impl NativeExecutor {
    pub(super) fn run_membership_cleanup(
        &mut self,
        lease: &CommandLease,
        cleanup: MembershipCleanup,
    ) -> Result<()> {
        if matches!(
            cleanup.result(),
            crate::terminal_session::state::MembershipResultKey::Switch(_)
        ) {
            return self.run_switch_membership_cleanup(lease, cleanup);
        }
        if self.attached_key() != Some(cleanup.old())
            || self.execution_desired(lease)?.as_ref() != Some(cleanup.old())
        {
            return Err(CarrierFailure::Protocol);
        }
        let total = cleanup.deadlines().total();
        let response = self.viewer_for_cleanup(
            SetupOwner::Lease(lease),
            TerminalViewerAction::Detach {},
            total,
        )?;
        if !matches!(response, TerminalViewerOutcome::Detached {})
            || self.attached_key() != Some(cleanup.old())
        {
            return Err(CarrierFailure::Protocol);
        }
        self.attached = None;
        let detached = cleanup.old().clone();
        let reply = self.publish_and_wait(
            WorkerEvent::MembershipDetached { cleanup, detached },
            EventDeadline::TotalCleanup(total),
            total,
        )?;
        let UiReply::Membership(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        match settlement.decision() {
            RefreshDecision::Continue | RefreshDecision::SettledWithoutAttachment => Ok(()),
            RefreshDecision::AttachRecovered(target) => self
                .setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_| ()),
            RefreshDecision::Retire => Err(CarrierFailure::Deadline),
            RefreshDecision::Cleanup(_) => Err(CarrierFailure::Protocol),
        }
    }
}
