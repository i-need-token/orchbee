use super::{NativeExecutor, Result, closed, event, submission};
use crate::{
    local_client::SubmissionError,
    workbench_client::{
        mailbox::{CommandLease, EventDeadline, UiReply, WorkerEvent},
        state::{AttachTarget, CarrierFailure, MembershipCleanup, RefreshDecision},
    },
};
use orchbee_client_server_protocol::{
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerUnavailableReason,
};

impl NativeExecutor {
    pub(super) fn run_switch_membership(
        &mut self,
        lease: &CommandLease,
        target: AttachTarget,
    ) -> Result<()> {
        let permit = match self
            .mailbox
            .authorize_switch_snapshot(lease, &target, &self.clock)
        {
            Ok(Some(permit)) => permit,
            Ok(None) => return Ok(()),
            Err(CarrierFailure::Deadline) => {
                let Some(permit) =
                    self.mailbox
                        .authorize_switch_expiry(lease, &target, &self.clock)?
                else {
                    return Ok(());
                };
                let total = target.deadlines().total();
                let reply = self.publish_and_wait(
                    WorkerEvent::SwitchForwardExpired { permit },
                    EventDeadline::TotalCleanup(total),
                    total,
                )?;
                let UiReply::SwitchExpired(settlement) = reply else {
                    return Err(CarrierFailure::Protocol);
                };
                return self.continue_switch_membership(lease, settlement.decision());
            }
            Err(failure) => return Err(failure),
        };
        let cutoff = target.deadlines().forward_cutoff();
        let outcome = self.read_workbench_before(permit.request_id().clone(), cutoff)?;
        self.mailbox.answer_switch_snapshot(permit)?;
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::SwitchSnapshotResult {
                    marker: target.selected().key().clone(),
                    outcome,
                },
                EventDeadline::PlacementOnly(cutoff),
                &self.clock,
            )
            .map_err(event)?;
        #[cfg(test)]
        self.mailbox.stop_at_switch_snapshot_wait_for_test();
        // Timely placement settles transport; only exact UI settlement or closure ends this wait.
        let reply = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?;
        let UiReply::Switch(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        self.continue_switch_membership(lease, settlement.decision())
    }

    fn continue_switch_membership(
        &mut self,
        lease: &CommandLease,
        decision: &RefreshDecision,
    ) -> Result<()> {
        match decision {
            RefreshDecision::Continue | RefreshDecision::SettledWithoutAttachment => Ok(()),
            RefreshDecision::Cleanup(cleanup) => {
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            RefreshDecision::AttachRecovered(target) => {
                self.continue_switch_without_viewer(lease, target.clone())
            }
            RefreshDecision::Retire => Err(CarrierFailure::Deadline),
        }
    }

    pub(super) fn run_switch_membership_cleanup(
        &mut self,
        lease: &CommandLease,
        cleanup: MembershipCleanup,
    ) -> Result<()> {
        let total = cleanup.deadlines().total();
        loop {
            // The cleanup retires the whole anchor: every live attachment must
            // sit under it, exactly as the old single-attachment gate required.
            if !self.attachments_under(cleanup.old()) {
                return Err(CarrierFailure::Protocol);
            }
            let Some(permit) = self.mailbox.authorize_switch_cleanup(
                lease,
                &cleanup,
                cleanup.old(),
                &self.clock,
            )?
            else {
                // A newer marker already discarded this unsubmitted candidate under the same gate.
                return Ok(());
            };
            let pending: Vec<crate::workbench_client::state::PaneRef> = self
                .attachments
                .iter()
                .filter(|attached| attached.key.pane().anchor() == cleanup.old())
                .map(|attached| attached.key.clone())
                .collect();
            let mut busy = false;
            for pane in pending {
                let attachment_id = self.viewer_id_for(&pane)?;
                let request_id = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
                let response = self.client.submit_terminal_viewer_before(
                    TerminalViewerRequest::new(
                        request_id,
                        TerminalViewerAction::Detach { attachment_id },
                    ),
                    total,
                );
                let outcome = match response {
                    Ok(result) => result.into_outcome(),
                    Err(
                        error @ (SubmissionError::RequestNotSubmitted(_)
                        | SubmissionError::ConnectionUnavailable
                        | SubmissionError::RequestEncoding(_)),
                    ) => {
                        if self.mailbox.answer_switch_cleanup(permit, false)? {
                            return self.settle_switch_cleanup_skipped(lease, cleanup);
                        }
                        return Err(submission(error));
                    }
                    Err(error) => return Err(submission(error)),
                };
                match outcome {
                    TerminalViewerOutcome::Detached {} => {
                        self.remove_attachment(&pane)?;
                    }
                    TerminalViewerOutcome::Unavailable {
                        reason: ViewerUnavailableReason::ServerBusy,
                    } => {
                        busy = true;
                        break;
                    }
                    // This compatible response settles the invocation, but it never proves Detached.
                    _ => {
                        self.mailbox.answer_switch_cleanup(permit, false)?;
                        return Err(CarrierFailure::Protocol);
                    }
                }
            }
            if busy {
                if self.mailbox.answer_switch_cleanup(permit, false)? {
                    return self.settle_switch_cleanup_skipped(lease, cleanup);
                }
                self.mailbox
                    .await_execution_change_before(lease, total, &self.clock)
                    .map_err(closed)?;
                continue;
            }
            self.mailbox.answer_switch_cleanup(permit, true)?;
            let detached = cleanup.old().clone();
            let reply = self.publish_and_wait(
                WorkerEvent::MembershipDetached { cleanup, detached },
                EventDeadline::TotalCleanup(total),
                total,
            )?;
            let UiReply::Switch(settlement) = reply else {
                return Err(CarrierFailure::Protocol);
            };
            return self.continue_switch_membership(lease, settlement.decision());
        }
    }

    fn settle_switch_cleanup_skipped(
        &mut self,
        lease: &CommandLease,
        cleanup: MembershipCleanup,
    ) -> Result<()> {
        let total = cleanup.deadlines().total();
        let reply = self.publish_and_wait(
            WorkerEvent::MembershipCleanupSkipped { cleanup },
            EventDeadline::TotalCleanup(total),
            total,
        )?;
        let UiReply::Switch(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        self.continue_switch_membership(lease, settlement.decision())
    }
}
