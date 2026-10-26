//! Consumes exact Panel operation results into prepared membership and mailbox settlement,
//! preserving the operation's captured focus and commit evidence. It does not mutate Server
//! topology or retarget a stale result to the current Panel.

use super::WorkbenchClientUi;
use crate::workbench_client::{
    mailbox::{
        CommandMailbox, DesiredInstall, MutationResult, PanelSettlement, UiReply, UiWaitKey,
    },
    state::{
        CarrierFailure, MembershipInstallDecision, OperationDeadlines, PanelDecision,
        PanelOperation, PanelResult,
    },
};
use std::sync::Arc;

impl WorkbenchClientUi {
    pub(super) fn apply_panel_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: UiWaitKey,
        operation: PanelOperation,
        outcome: PanelResult,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let prepared = state.prepare_panel(&operation, outcome);
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
                let (operation, prepared) = state.retain_prepared_panel_membership(prepared)?;
                let deadlines = deadlines.ok_or(CarrierFailure::Deadline)?;
                if transaction.observed_at() >= deadlines.forward_cutoff() {
                    return Err(CarrierFailure::Deadline);
                }
                if state.has_attachments() {
                    PanelDecision::Cleanup(state.stage_panel_membership_cleanup(
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
                    let state_plan = state.prepare_panel_membership_install(
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
                            PanelDecision::AttachRecovered(target)
                        }
                        MembershipInstallDecision::SelectedWithoutAttachment
                        | MembershipInstallDecision::Empty => {
                            PanelDecision::SettledWithoutAttachment
                        }
                        MembershipInstallDecision::Preserved => PanelDecision::Continue,
                    }
                }
            } else {
                state.apply_prepared_panel(prepared)
            };
            if decision == PanelDecision::Retire {
                retirement = state.retired_failure();
            }
            Ok(UiReply::Panel(PanelSettlement::new(
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
