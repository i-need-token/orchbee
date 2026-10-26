use super::{
    CarrierFailure, CommandLease, EventDeadline, Instant, NativeExecutor, PaneKey, RefreshDecision,
    Result, SetupEntryExpiry, SetupOwner, SetupTarget, TerminalViewerAction, TerminalViewerOutcome,
    UiReply, WorkerEvent,
};
use crate::workbench_client::state::MembershipCleanup;

impl NativeExecutor {
    pub(super) fn run_membership_cleanup(
        &mut self,
        lease: &CommandLease,
        cleanup: MembershipCleanup,
    ) -> Result<()> {
        if matches!(
            cleanup.result(),
            crate::workbench_client::state::MembershipResultKey::Switch(_)
        ) {
            return self.run_switch_membership_cleanup(lease, cleanup);
        }
        // Epoch teardown: every live attachment must sit under the retiring
        // anchor, and the focus pane must still name that anchor.
        if !self.attachments_under(cleanup.old())
            || self.execution_desired(lease)?.as_ref().map(PaneKey::anchor) != Some(cleanup.old())
        {
            return Err(CarrierFailure::Protocol);
        }
        let total = cleanup.deadlines().total();
        if let Some(proof) = cleanup.closed_proof().cloned() {
            if cleanup.close_marker().is_none() || proof.attachment_key() != cleanup.old() {
                return Err(CarrierFailure::Protocol);
            }
            self.dispose_anchor_panes(lease, cleanup.old(), total)?;
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
        self.detach_anchor_panes(lease, cleanup.old(), total)?;
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
                crate::workbench_client::state::MembershipResultKey::Rename(id),
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

    /// True when every live attachment sits under `anchor` — the multi-pane
    /// form of the old "the attached key is the retiring one" gate.
    pub(super) fn attachments_under(
        &self,
        anchor: &crate::workbench_client::state::AttachmentKey,
    ) -> bool {
        !self.attachments.is_empty()
            && self
                .attachments
                .iter()
                .all(|attached| attached.key.pane().anchor() == anchor)
    }

    /// Detaches and canonically disposes every pane under `anchor`, publishing
    /// one `PaneDetached` proof per incarnation before the caller emits the
    /// anchor-level settlement event.
    fn detach_anchor_panes(
        &mut self,
        lease: &CommandLease,
        anchor: &crate::workbench_client::state::AttachmentKey,
        total: Instant,
    ) -> Result<()> {
        loop {
            let Some(pane) = self
                .attachments
                .iter()
                .find(|attached| attached.key.pane().anchor() == anchor)
                .map(|attached| attached.key.clone())
            else {
                return Ok(());
            };
            let response = self.viewer_for_cleanup(
                SetupOwner::Lease(lease),
                TerminalViewerAction::Detach {
                    attachment_id: self.viewer_id_for(&pane)?,
                },
                total,
            )?;
            if !matches!(response, TerminalViewerOutcome::Detached {}) {
                return Err(CarrierFailure::Protocol);
            }
            self.remove_attachment(&pane)?;
        }
    }

    /// Drops every pane under `anchor` without a wire Detach — used when a
    /// definitive closed-attachment proof already settled the viewers. The
    /// anchor-level settlement event is the canonical disposal proof for the
    /// whole epoch, so no per-pane notice is published.
    fn dispose_anchor_panes(
        &mut self,
        _lease: &CommandLease,
        anchor: &crate::workbench_client::state::AttachmentKey,
        _total: Instant,
    ) -> Result<()> {
        while let Some(pane) = self
            .attachments
            .iter()
            .find(|attached| attached.key.pane().anchor() == anchor)
            .map(|attached| attached.key.clone())
        {
            self.remove_attachment(&pane)?;
        }
        Ok(())
    }

    fn finish_close_membership_cleanup(
        &mut self,
        lease: &CommandLease,
        marker: &crate::workbench_client::state::CloseReconciliationMarker,
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
