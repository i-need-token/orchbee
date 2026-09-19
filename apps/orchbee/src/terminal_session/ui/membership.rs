use super::*;
use crate::terminal_session::state::{
    MembershipCleanup, MembershipInstallDecision, MembershipResultKey,
};

impl NativeUi {
    pub(super) fn apply_membership_detached(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        cleanup: MembershipCleanup,
        detached: AttachmentKey,
    ) -> bool {
        let prepared = self
            .state
            .as_ref()
            .ok_or(CarrierFailure::Protocol)
            .and_then(|state| {
                cleanup
                    .installs_candidate()
                    .then(|| state.prepare_cleaned_membership_candidate(&cleanup, &detached))
                    .transpose()
            });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(failure) => {
                self.retire(failure);
                return true;
            }
        };
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut expired = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            if let MembershipResultKey::Switch(marker) = cleanup.result() {
                if !state.switch_marker_is_current(marker) {
                    state.settle_switch_membership_cleanup(&cleanup, true)?;
                    return Ok(UiReply::Switch(RefreshSettlement::new(
                        RefreshDecision::Continue,
                        state.desired().cloned(),
                    )));
                }
                if transaction.observed_at() >= cleanup.deadlines().forward_cutoff() {
                    state.settle_switch_membership_cleanup(&cleanup, true)?;
                    state.retire(CarrierFailure::Deadline);
                    expired = true;
                    return Ok(UiReply::Switch(RefreshSettlement::new(
                        RefreshDecision::Retire,
                        state.desired().cloned(),
                    )));
                }
            }
            if transaction.observed_at() >= cleanup.deadlines().forward_cutoff() {
                state.retire(CarrierFailure::Deadline);
                expired = true;
                if let Some(marker) = cleanup.close_marker() {
                    return Ok(UiReply::CloseReconciliation(Box::new(
                        super::super::mailbox::CloseReconciliationSettlement::new(
                            marker.clone(),
                            RefreshDecision::Retire,
                            state.desired().cloned(),
                        ),
                    )));
                }
                if let MembershipResultKey::Rename(id) = cleanup.result() {
                    return Ok(UiReply::Rename(
                        super::super::mailbox::RenameSettlement::new(
                            id.clone(),
                            RefreshDecision::Retire,
                            state.desired().cloned(),
                        ),
                    ));
                }
                return Ok(UiReply::Membership(RefreshSettlement::new(
                    RefreshDecision::Retire,
                    state.desired().cloned(),
                )));
            }
            let prepared = prepared.ok_or(CarrierFailure::Protocol)?;
            let install = match prepared.selected_tab_id() {
                Some(tab)
                    if state
                        .create_membership_commit()
                        .is_some_and(|committed| committed.committed_key().tab_id() == tab) =>
                {
                    super::super::mailbox::DesiredInstall::ReuseReserved {
                        key: state
                            .create_membership_commit()
                            .ok_or(CarrierFailure::Protocol)?
                            .committed_key()
                            .clone(),
                        panel: prepared
                            .focus_panel_id()
                            .cloned()
                            .ok_or(CarrierFailure::Protocol)?,
                    }
                }
                Some(tab) => match cleanup.result() {
                    // A switch cleanup installs the marker's own pane — the
                    // candidate's default focus leaf may differ from the pane
                    // the switch target selected.
                    MembershipResultKey::Switch(marker) => {
                        super::super::mailbox::DesiredInstall::Fresh {
                            tab_id: tab.clone(),
                            panel: marker.panel().clone(),
                        }
                    }
                    _ => match prepared.focus_panel_id() {
                        Some(panel) => super::super::mailbox::DesiredInstall::Fresh {
                            tab_id: tab.clone(),
                            panel: panel.clone(),
                        },
                        None => super::super::mailbox::DesiredInstall::Empty,
                    },
                },
                None => super::super::mailbox::DesiredInstall::Empty,
            };
            let desired_plan = transaction.prepare_desired_install(install)?;
            let state_plan = state.prepare_cleaned_membership_install(
                &cleanup,
                prepared,
                desired_plan.desired().map(|key| key.anchor().clone()),
            )?;
            let decision = state_plan.commit();
            desired_plan.commit();
            let decision = match decision {
                MembershipInstallDecision::Attach(target) => {
                    RefreshDecision::AttachRecovered(target)
                }
                MembershipInstallDecision::SelectedWithoutAttachment
                | MembershipInstallDecision::Empty => RefreshDecision::SettledWithoutAttachment,
                MembershipInstallDecision::Preserved => RefreshDecision::Continue,
            };
            if let Some(marker) = cleanup.close_marker() {
                return Ok(UiReply::CloseReconciliation(Box::new(
                    super::super::mailbox::CloseReconciliationSettlement::new(
                        marker.clone(),
                        decision,
                        state.desired().cloned(),
                    ),
                )));
            }
            let settlement = RefreshSettlement::new(decision, state.desired().cloned());
            Ok(match cleanup.result() {
                MembershipResultKey::Refresh(_)
                | MembershipResultKey::Create(_)
                | MembershipResultKey::Move(_)
                | MembershipResultKey::Panel(_) => UiReply::Membership(settlement),
                MembershipResultKey::Switch(_) => UiReply::Switch(settlement),
                MembershipResultKey::Rename(id) => {
                    UiReply::Rename(super::super::mailbox::RenameSettlement::new(
                        id.clone(),
                        settlement.decision().clone(),
                        settlement.desired().cloned(),
                    ))
                }
                MembershipResultKey::Close(_) => return Err(CarrierFailure::Protocol),
            })
        });
        match result {
            Ok(MutationResult::Applied) => {
                self.clear_projections();
                if expired {
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
}
