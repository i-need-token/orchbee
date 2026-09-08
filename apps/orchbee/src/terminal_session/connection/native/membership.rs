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
        if let Some(proof) = cleanup.closed_proof().cloned() {
            if cleanup.close_marker().is_none() || proof.attachment_key() != cleanup.old() {
                return Err(CarrierFailure::Protocol);
            }
            self.attached = None;
            let marker = cleanup
                .close_marker()
                .cloned()
                .ok_or(CarrierFailure::Protocol)?;
            let reply = self.publish_and_wait(
                WorkerEvent::MembershipGone { cleanup, proof },
                EventDeadline::TotalCleanup(total),
                total,
            )?;
            return self.finish_close_membership_cleanup(lease, &marker, reply);
        }
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
        let close_marker = cleanup.close_marker().cloned();
        let origin = cleanup.result().clone();
        let reply = self.publish_and_wait(
            WorkerEvent::MembershipDetached { cleanup, detached },
            EventDeadline::TotalCleanup(total),
            total,
        )?;
        if let Some(marker) = close_marker {
            return self.finish_close_membership_cleanup(lease, &marker, reply);
        }
        let decision = match (&origin, &reply) {
            (
                crate::terminal_session::state::MembershipResultKey::Rename(id),
                UiReply::Rename(settlement),
            ) if settlement.request_id() == id => settlement.decision(),
            (_, UiReply::Membership(settlement)) => settlement.decision(),
            _ => return Err(CarrierFailure::Protocol),
        };
        match decision {
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

    fn finish_close_membership_cleanup(
        &mut self,
        lease: &CommandLease,
        marker: &crate::terminal_session::state::CloseReconciliationMarker,
        reply: UiReply,
    ) -> Result<()> {
        let UiReply::CloseReconciliation(reply) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        if reply.marker() != marker {
            return Err(CarrierFailure::Protocol);
        }
        match reply.settlement().decision() {
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
