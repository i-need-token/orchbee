use super::*;
use crate::terminal_session::{
    mailbox::{CloseAdmissionError, CloseReconciliationSettlement, DesiredInstall, UiWaitKey},
    state::{CloseReconciliationMarker, MembershipInstallDecision},
};

impl NativeUi {
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
            .and_then(|_| OperationDeadlines::from_sample(self.clock.now()));
        let Some(state) = &mut self.state else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut clear = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            let decision = if let Some(prepared) = prepared {
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
                if !prepared.preserves_selection() && state.attached_key().is_some() {
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
                        DesiredInstall::Fresh(tab.clone())
                    } else {
                        DesiredInstall::Empty
                    };
                    let desired_plan = transaction.prepare_desired_install(install)?;
                    let state_plan = state.prepare_close_membership_install(
                        &marker,
                        prepared,
                        desired_plan.desired().cloned(),
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
                if clear {
                    self.clear_projection();
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}
