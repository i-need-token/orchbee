use super::*;
use crate::terminal_session::mailbox::UiWaitKey;
use crate::terminal_session::{
    mailbox::DesiredInstall,
    state::{MembershipCleanup, MembershipInstallDecision, MembershipResultKey},
};

impl UiDriver {
    pub(super) fn settle_switch_snapshot(
        &mut self,
        wait: UiWaitKey,
        marker: AttachmentKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> TestResult<UiEvent> {
        let prepared = if self.state().switch_marker_is_current_k(&marker)
            && matches!(outcome, WorkbenchSnapshotOutcome::Available { .. })
        {
            Some(
                self.state()
                    .prepare_switch_candidate_k(&marker, outcome)
                    .map_err(|_| test_error("Switch candidate validation"))?,
            )
        } else {
            None
        };
        let retained = self
            .projection
            .as_ref()
            .and_then(|_| self.state().attached_key().cloned());
        let mailbox = Arc::clone(&self.mailbox);
        let clock = self.clock.clone();
        let state = self.state_mut();
        let mut decision = None;
        require_applied(
            mailbox.deposit_ui_reply_before(&wait, &clock, |transaction| {
                if let Some(discarded) = transaction.take_switch_anchor_invalidation(
                    &state
                        .pane_key_for_anchor(&marker)
                        .unwrap_or_else(|| pane_key(&marker)),
                ) {
                    state.invalidate_switch_anchor_for_k(&discarded);
                }
                let settled = if !state.switch_marker_is_current_k(&marker) {
                    RefreshDecision::Continue
                } else if let Some(prepared) = prepared {
                    let deadlines = state
                        .switch_deadlines_k(&marker)
                        .ok_or(CarrierFailure::Protocol)?;
                    if state.has_attachments() {
                        RefreshDecision::Cleanup(
                            state
                                .stage_switch_membership_cleanup_k(&marker, prepared, deadlines)?,
                        )
                    } else if transaction.observed_at() >= deadlines.forward_cutoff() {
                        state.retire(CarrierFailure::Deadline);
                        RefreshDecision::Retire
                    } else {
                        let desired = transaction.prepare_desired_install(
                            prepared
                                .selected_tab_id()
                                .map_or(DesiredInstall::Empty, |tab| DesiredInstall::Fresh {
                                    tab_id: tab.clone(),
                                    panel: prepared
                                        .focus_panel_id()
                                        .expect("selected Tab has a Panel")
                                        .clone(),
                                }),
                        )?;
                        let plan = state.prepare_switch_membership_install_k(
                            &marker,
                            prepared,
                            desired.desired().map(|pane| pane.anchor().clone()),
                        )?;
                        let installed = plan.commit();
                        desired.commit();
                        install_decision(installed)
                    }
                } else {
                    match state.prepare_switch_restore_k(&marker, retained.as_ref()) {
                        Ok(plan) => {
                            let desired = transaction.prepare_desired_install(
                                DesiredInstall::RestoreSwitch(plan.restoration().clone()),
                            )?;
                            plan.commit();
                            desired.commit();
                            RefreshDecision::Continue
                        }
                        Err(_) => {
                            state.retire(CarrierFailure::AuthorityDivergence);
                            RefreshDecision::Retire
                        }
                    }
                };
                decision = Some(settled.clone());
                Ok(UiReply::Switch(RefreshSettlement::new(
                    settled,
                    state.desired().cloned(),
                )))
            }),
        )?;
        Ok(UiEvent::SwitchSnapshot {
            work: marker,
            decision: decision.ok_or_else(|| test_error("Switch decision"))?,
        })
    }

    pub(super) fn settle_switch_forward_expired(
        &mut self,
        wait: UiWaitKey,
        target: crate::terminal_session::state::AttachTarget,
    ) -> TestResult<UiEvent> {
        let marker = target.selected().key().clone();
        let mailbox = Arc::clone(&self.mailbox);
        let clock = self.clock.clone();
        let state = self.state_mut();
        let mut decision = None;
        require_applied(
            mailbox.deposit_ui_reply_before(&wait, &clock, |transaction| {
                let settled = if !state.switch_marker_is_current_k(&marker) {
                    RefreshDecision::Continue
                } else if transaction.observed_at() < target.deadlines().forward_cutoff() {
                    return Err(CarrierFailure::Protocol);
                } else if state.has_attachments() {
                    RefreshDecision::Cleanup(state.stage_switch_expiry_cleanup(&marker)?)
                } else {
                    state.retire(CarrierFailure::Deadline);
                    RefreshDecision::Retire
                };
                decision = Some(settled.clone());
                Ok(UiReply::SwitchExpired(RefreshSettlement::new(
                    settled,
                    state.desired().cloned(),
                )))
            }),
        )?;
        Ok(UiEvent::SwitchSnapshot {
            work: marker.anchor().clone(),
            decision: decision.ok_or_else(|| test_error("Switch expiry decision"))?,
        })
    }

    pub(super) fn settle_switch_membership_cleanup(
        &mut self,
        wait: UiWaitKey,
        cleanup: MembershipCleanup,
        detached: bool,
    ) -> TestResult<UiEvent> {
        let MembershipResultKey::Switch(marker) = cleanup.result() else {
            return Err(test_error("Switch cleanup token"));
        };
        let marker = marker.clone();
        let prepared = cleanup
            .installs_candidate()
            .then(|| {
                self.state()
                    .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            })
            .transpose()
            .map_err(|_| test_error("exact Switch pending cleanup"))?;
        let mailbox = Arc::clone(&self.mailbox);
        let clock = self.clock.clone();
        let state = self.state_mut();
        let mut decision = None;
        require_applied(
            mailbox.deposit_ui_reply_before(&wait, &clock, |transaction| {
                let settled = if !detached || !state.switch_marker_is_current_k(&marker) {
                    state.settle_switch_membership_cleanup(&cleanup, detached)?;
                    RefreshDecision::Continue
                } else if transaction.observed_at() >= cleanup.deadlines().forward_cutoff() {
                    state.settle_switch_membership_cleanup(&cleanup, true)?;
                    state.retire(CarrierFailure::Deadline);
                    RefreshDecision::Retire
                } else {
                    let prepared = prepared.ok_or(CarrierFailure::Protocol)?;
                    let desired = transaction.prepare_desired_install(
                        prepared
                            .selected_tab_id()
                            .map_or(DesiredInstall::Empty, |tab| DesiredInstall::Fresh {
                                tab_id: tab.clone(),
                                panel: prepared
                                    .focus_panel_id()
                                    .expect("selected Tab has a Panel")
                                    .clone(),
                            }),
                    )?;
                    let plan = state.prepare_cleaned_membership_install(
                        &cleanup,
                        prepared,
                        desired.desired().map(|pane| pane.anchor().clone()),
                    )?;
                    let installed = plan.commit();
                    desired.commit();
                    install_decision(installed)
                };
                decision = Some(settled.clone());
                Ok(UiReply::Switch(RefreshSettlement::new(
                    settled,
                    state.desired().cloned(),
                )))
            }),
        )?;
        if detached {
            self.projection = None;
        }
        let decision = decision.ok_or_else(|| test_error("Switch cleanup decision"))?;
        if !detached {
            return Ok(UiEvent::SwitchSnapshot {
                work: marker.anchor().clone(),
                decision,
            });
        }
        let continuation = match decision {
            RefreshDecision::AttachRecovered(target) => SwitchDetachContinuation::Attach(target),
            RefreshDecision::Continue => SwitchDetachContinuation::Superseded {
                desired: self
                    .state()
                    .desired()
                    .map(|pane| pane.anchor().clone())
                    .ok_or_else(|| test_error("superseded desired"))?,
            },
            RefreshDecision::SettledWithoutAttachment | RefreshDecision::Retire => {
                SwitchDetachContinuation::SettledWithoutAttachment
            }
            RefreshDecision::Cleanup(_) => return Err(test_error("recursive Switch cleanup")),
        };
        Ok(UiEvent::SwitchOldDetached {
            work: marker.anchor().clone(),
            detached: cleanup.old().clone(),
            continuation,
        })
    }
}

fn install_decision(decision: MembershipInstallDecision) -> RefreshDecision {
    match decision {
        MembershipInstallDecision::Attach(target) => RefreshDecision::AttachRecovered(target),
        MembershipInstallDecision::Empty | MembershipInstallDecision::SelectedWithoutAttachment => {
            RefreshDecision::SettledWithoutAttachment
        }
        MembershipInstallDecision::Preserved => RefreshDecision::Continue,
    }
}

pub(in crate::terminal_session::connection::native::tests) fn drive_switch_snapshot(
    harness: &mut Harness,
) -> TestResult<UiEvent> {
    let request = harness.carrier.take_read()?;
    let candidate = snapshot(
        harness.ui.state().cache().tabs().len(),
        harness.ui.state().cache().revision().value(),
    );
    harness.carrier.respond_read(
        &request,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    )?;
    harness.ui.settle_next(FrameCut::None)
}
