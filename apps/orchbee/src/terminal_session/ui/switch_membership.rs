use super::*;
use crate::terminal_session::{
    mailbox::DesiredInstall,
    state::{MembershipCleanup, MembershipInstallDecision},
};
use orchbee_workbench_protocol::WorkbenchSnapshotOutcome;

#[cfg(test)]
mod tests;

impl NativeUi {
    pub(super) fn apply_switch_forward_expired(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        target: super::super::state::AttachTarget,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut retired = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            let decision = if !state.switch_marker_is_current(target.selected().key()) {
                RefreshDecision::Continue
            } else if transaction.observed_at() < target.deadlines().forward_cutoff() {
                return Err(CarrierFailure::Protocol);
            } else if state.attached_key().is_some() {
                RefreshDecision::Cleanup(
                    state.stage_switch_expiry_cleanup(target.selected().key())?,
                )
            } else {
                state.retire(CarrierFailure::Deadline);
                retired = true;
                RefreshDecision::Retire
            };
            Ok(UiReply::SwitchExpired(RefreshSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if retired {
                    self.retire(CarrierFailure::Deadline);
                    true
                } else {
                    self.validate_state()
                }
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }

    pub(super) fn apply_switch_snapshot(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        marker: AttachmentKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // Canonical validation and all bounded candidate allocations precede the reply mutex.
        let prepared = if state.switch_marker_is_current(&marker)
            && matches!(outcome, WorkbenchSnapshotOutcome::Available { .. })
        {
            match state.prepare_switch_candidate(&marker, outcome) {
                Ok(prepared) => Some(prepared),
                Err(failure) => {
                    self.retire(failure);
                    return true;
                }
            }
        } else {
            None
        };
        let retained_projection = self
            .projection
            .as_ref()
            .map(|projection| projection.key.clone());
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut failure_after_reply = None;
        let mut installed = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            if let Some(discarded) = transaction.take_switch_anchor_invalidation(&marker) {
                state.invalidate_switch_anchor_for(&discarded);
            }
            let decision = if !state.switch_marker_is_current(&marker) {
                RefreshDecision::Continue
            } else if let Some(prepared) = prepared {
                if state.attached_key().is_some() {
                    let deadlines = state
                        .switch_deadlines(&marker)
                        .ok_or(CarrierFailure::Protocol)?;
                    let cleanup =
                        state.stage_switch_membership_cleanup(&marker, prepared, deadlines)?;
                    RefreshDecision::Cleanup(cleanup)
                } else if transaction.observed_at()
                    >= state
                        .switch_deadlines(&marker)
                        .ok_or(CarrierFailure::Protocol)?
                        .forward_cutoff()
                {
                    state.retire(CarrierFailure::Deadline);
                    failure_after_reply = Some(CarrierFailure::Deadline);
                    RefreshDecision::Retire
                } else {
                    let install = prepared
                        .selected_tab_id()
                        .map_or(DesiredInstall::Empty, |tab| {
                            DesiredInstall::Fresh(tab.clone())
                        });
                    let desired = transaction.prepare_desired_install(install)?;
                    let canonical = state.prepare_switch_membership_install(
                        &marker,
                        prepared,
                        desired.desired().cloned(),
                    )?;
                    let decision = canonical.commit();
                    desired.commit();
                    installed = true;
                    membership_decision(decision)
                }
            } else {
                match state.prepare_switch_restore(&marker, retained_projection.as_ref()) {
                    Ok(canonical) => {
                        let desired = transaction.prepare_desired_install(
                            DesiredInstall::RestoreSwitch(canonical.restoration().clone()),
                        )?;
                        canonical.commit();
                        desired.commit();
                        RefreshDecision::Continue
                    }
                    Err(_) => {
                        state.retire(CarrierFailure::AuthorityDivergence);
                        failure_after_reply = Some(CarrierFailure::AuthorityDivergence);
                        RefreshDecision::Retire
                    }
                }
            };
            Ok(UiReply::Switch(RefreshSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if installed {
                    self.clear_projection();
                }
                if let Some(failure) = failure_after_reply {
                    self.retire(failure);
                    true
                } else {
                    self.validate_state()
                }
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }

    pub(super) fn apply_switch_cleanup_skipped(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        cleanup: MembershipCleanup,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |_transaction| {
            state.settle_switch_membership_cleanup(&cleanup, false)?;
            Ok(UiReply::Switch(RefreshSettlement::new(
                RefreshDecision::Continue,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => self.validate_state(),
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}

pub(super) fn membership_decision(decision: MembershipInstallDecision) -> RefreshDecision {
    match decision {
        MembershipInstallDecision::Attach(target) => RefreshDecision::AttachRecovered(target),
        MembershipInstallDecision::SelectedWithoutAttachment | MembershipInstallDecision::Empty => {
            RefreshDecision::SettledWithoutAttachment
        }
        MembershipInstallDecision::Preserved => RefreshDecision::Continue,
    }
}
