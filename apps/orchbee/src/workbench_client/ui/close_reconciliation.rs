use super::{
    Arc, CarrierFailure, CommandMailbox, MutationResult, OperationDeadlines, RefreshDecision,
    UiReply, WorkbenchClientState, WorkbenchClientUi,
};
use crate::workbench_client::{
    mailbox::{CloseAdmissionError, CloseReconciliationSettlement, DesiredInstall, UiWaitKey},
    state::{CloseReconciliationMarker, MembershipInstallDecision},
};

impl WorkbenchClientUi {
    pub(super) fn schedule_close_reconciliation(&mut self, mailbox: &Arc<CommandMailbox>) {
        let Some(deadline) = self
            .clock
            .now()
            .checked_add(super::super::state::OPERATION_TOTAL)
        else {
            return;
        };
        let Some(state) = &mut self.state else {
            return;
        };
        let deadline = state
            .close_set_deadline()
            .map_or(deadline, |sequence| sequence.min(deadline));
        let requested = state.close_reconciliation_requested_origin();
        let result = mailbox.try_enqueue_close_reconciliation_for(state, deadline, &self.clock);
        if result.is_err()
            && let Some(origin) = requested
        {
            state.refuse_close_reconciliation_admission(&origin);
        }
        if let Err(CloseAdmissionError::Closed(closed)) = result {
            self.retire(closed.failure());
        }
    }

    pub(super) fn apply_close_reconciliation_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: UiWaitKey,
        marker: CloseReconciliationMarker,
        outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome,
    ) -> bool {
        // The board's index consumes the snapshot only when this reply
        // installs a membership candidate from it — a failed
        // reconciliation's `Continue` never feeds it.
        let index_snapshot = match &outcome {
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available { snapshot } => {
                Some(snapshot.clone())
            }
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Denied { .. }
            | orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Unavailable { .. } => None,
        };
        let prepared = self
            .state
            .as_ref()
            .ok_or(CarrierFailure::Protocol)
            .and_then(|state| state.prepare_close_membership_candidate(&marker, outcome));
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(failure) => {
                self.retire(failure);
                return true;
            }
        };
        let deadlines = prepared
            .as_ref()
            .filter(|candidate| !candidate.preserves_selection() || marker.membership_recovery())
            .and_then(|_| OperationDeadlines::from_sample(self.clock.now()))
            .map(|budget| {
                self.state
                    .as_ref()
                    .and_then(WorkbenchClientState::close_set_deadline)
                    .map_or(budget, |cutoff| budget.bounded_by(cutoff))
            });
        let Some(state) = &mut self.state else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut clear = false;
        let mut snapshot_consumed = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            let decision = if let Some(prepared) = prepared {
                snapshot_consumed = true;
                if (!prepared.preserves_selection() || marker.membership_recovery())
                    && deadlines.is_none()
                {
                    return Err(CarrierFailure::Deadline);
                }
                if deadlines
                    .is_some_and(|budget| transaction.observed_at() >= budget.forward_cutoff())
                {
                    return Err(CarrierFailure::Deadline);
                }
                if !prepared.preserves_selection() && state.has_attachments() {
                    RefreshDecision::Cleanup(state.stage_close_membership_cleanup(
                        &marker,
                        prepared,
                        deadlines.ok_or(CarrierFailure::Deadline)?,
                    )?)
                } else {
                    let install = if prepared.preserves_selection() && !marker.membership_recovery()
                    {
                        DesiredInstall::Preserve
                    } else if let Some(tab) = prepared.selected_tab_id() {
                        match prepared.focus_panel_id() {
                            Some(panel) => DesiredInstall::Fresh {
                                tab_id: tab.clone(),
                                panel: panel.clone(),
                            },
                            None => DesiredInstall::Empty,
                        }
                    } else {
                        DesiredInstall::Empty
                    };
                    let desired_plan = transaction.prepare_desired_install(install)?;
                    let state_plan = state.prepare_close_membership_install(
                        &marker,
                        prepared,
                        desired_plan.desired().map(|key| key.anchor().clone()),
                        deadlines,
                    )?;
                    let decision = state_plan.commit();
                    desired_plan.commit();
                    match decision {
                        MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                        MembershipInstallDecision::Attach(target) => {
                            clear = true;
                            RefreshDecision::AttachRecovered(target)
                        }
                        MembershipInstallDecision::SelectedWithoutAttachment
                        | MembershipInstallDecision::Empty => {
                            clear = true;
                            RefreshDecision::SettledWithoutAttachment
                        }
                    }
                }
            } else {
                state
                    .close_reconciliation_failed(&marker)
                    .map_err(|_| CarrierFailure::Protocol)?;
                RefreshDecision::Continue
            };
            Ok(UiReply::CloseReconciliation(Box::new(
                CloseReconciliationSettlement::new(marker, decision, state.desired().cloned()),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if snapshot_consumed && let Some(snapshot) = &index_snapshot {
                    self.agent_index =
                        super::super::agent_board::PresentationIndex::rebuild(snapshot);
                }
                if clear {
                    self.clear_projections();
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}
