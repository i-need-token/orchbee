use super::NativeUi;
use crate::terminal_session::{
    mailbox::{CommandMailbox, DesiredInstall, MoveSettlement, MutationResult, UiReply, UiWaitKey},
    state::{
        CarrierFailure, MembershipInstallDecision, MoveDecision, MoveOperation, MoveResult,
        OperationDeadlines,
    },
};
use std::sync::Arc;

impl NativeUi {
    pub(super) fn apply_move_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: UiWaitKey,
        operation: MoveOperation,
        outcome: MoveResult,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let prepared = state.prepare_move(&operation, outcome);
        let membership = prepared.membership_candidate().is_some();
        let deadlines = membership
            .then(|| OperationDeadlines::from_sample(self.clock.now()))
            .flatten();
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut retirement = None;
        let mut clear_projection = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            let decision = if membership {
                let (operation, prepared) = state.retain_prepared_move_membership(prepared)?;
                let deadlines = deadlines.ok_or(CarrierFailure::Deadline)?;
                if transaction.observed_at() >= deadlines.forward_cutoff() {
                    return Err(CarrierFailure::Deadline);
                }
                if state.has_attachments() {
                    MoveDecision::Cleanup(state.stage_move_membership_cleanup(
                        operation.request_id(),
                        prepared,
                        deadlines,
                    )?)
                } else {
                    let install = match prepared.selected_tab_id() {
                        Some(tab) => match prepared.focus_panel_id() {
                            Some(panel) => DesiredInstall::Fresh {
                                tab_id: tab.clone(),
                                panel: panel.clone(),
                            },
                            None => DesiredInstall::Empty,
                        },
                        None => DesiredInstall::Empty,
                    };
                    let desired = transaction.prepare_desired_install(install)?;
                    let state_plan = state.prepare_move_membership_install(
                        operation.request_id(),
                        prepared,
                        desired.desired().map(|key| key.anchor().clone()),
                        Some(deadlines),
                    )?;
                    let installed = state_plan.commit();
                    desired.commit();
                    clear_projection = true;
                    match installed {
                        MembershipInstallDecision::Attach(target) => {
                            MoveDecision::AttachRecovered(target)
                        }
                        MembershipInstallDecision::SelectedWithoutAttachment
                        | MembershipInstallDecision::Empty => {
                            MoveDecision::SettledWithoutAttachment
                        }
                        MembershipInstallDecision::Preserved => MoveDecision::Continue,
                    }
                }
            } else {
                state.apply_prepared_move(prepared)
            };
            if decision == MoveDecision::Retire {
                retirement = state.retired_failure();
            }
            Ok(UiReply::Move(MoveSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if clear_projection {
                    self.clear_projections();
                }
                if let Some(failure) = retirement {
                    self.retire(failure);
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}
