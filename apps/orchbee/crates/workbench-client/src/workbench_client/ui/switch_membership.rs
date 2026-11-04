//! Consumes exact switch snapshot, cleanup, and expiry events into prepared UI membership and
//! mailbox decisions. It does not execute attachment I/O or let a stale switch reply change
//! the current selection.

use super::{
    Arc, CarrierFailure, CommandMailbox, MutationResult, PaneKey, RefreshDecision,
    RefreshSettlement, UiReply, WorkbenchClientUi,
};
use crate::workbench_client::mailbox::{DesiredInstall, UiReplyTransaction};
use crate::workbench_state::{MembershipCleanup, MembershipInstallDecision, WorkbenchClientState};
use orchbee_workbench_protocol::WorkbenchSnapshotOutcome;

#[cfg(test)]
mod tests;

impl WorkbenchClientUi {
    pub(super) fn apply_switch_forward_expired(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        target: crate::workbench_state::AttachTarget,
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
            } else if state.has_attachments() {
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
        marker: PaneKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> bool {
        let Some(state) = self.state.as_ref() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // The board's index consumes the snapshot only when this reply
        // actually installs it — a dead marker or a refused outcome is
        // stale for this purpose and never feeds it.
        let index_snapshot = match &outcome {
            WorkbenchSnapshotOutcome::Available { snapshot }
                if state.switch_marker_is_current(&marker) =>
            {
                Some(snapshot.clone())
            }
            WorkbenchSnapshotOutcome::Available { .. }
            | WorkbenchSnapshotOutcome::Denied { .. }
            | WorkbenchSnapshotOutcome::Unavailable { .. } => None,
        };
        // Canonical validation and all bounded candidate allocations precede the reply mutex.
        let mut context_removed = false;
        let prepared = if state.switch_marker_is_current(&marker)
            && matches!(outcome, WorkbenchSnapshotOutcome::Available { .. })
        {
            match state.prepare_switch_candidate(&marker, outcome) {
                Ok(prepared) => {
                    if !self.menu_switch_candidate_valid(marker.anchor().generation(), &prepared) {
                        self.fail_action_menu();
                        None
                    } else {
                        Some(prepared)
                    }
                }
                // A peer closed this context before the read. A Switch cannot land outside its
                // context: it restores its selection and owes the publication read whose
                // refresh classifies the removal.
                Err(_)
                    if index_snapshot
                        .as_ref()
                        .is_some_and(|snapshot| state.snapshot_removes_context(snapshot)) =>
                {
                    context_removed = true;
                    None
                }
                Err(failure) => {
                    self.retire(failure);
                    return true;
                }
            }
        } else {
            None
        };
        let retained_projections = self
            .projections
            .iter()
            .map(|projection| projection.key.clone())
            .collect::<Vec<_>>();
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut failure_after_reply = None;
        let mut installed = false;
        let mut snapshot_consumed = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            if let Some(discarded) = transaction.take_switch_anchor_invalidation(&marker) {
                state.invalidate_switch_anchor_for(&discarded);
            }
            let decision = if !state.switch_marker_is_current(&marker) {
                RefreshDecision::Continue
            } else if let Some(prepared) = prepared {
                snapshot_consumed = true;
                if state.has_attachments() {
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
                    // The switch's focus pane is the marker's own pane: the
                    // desired install must name the marker's panel or the
                    // mailbox's focus would diverge from the state's target.
                    let install = match prepared.selected_tab_id() {
                        Some(tab) => DesiredInstall::Fresh {
                            tab_id: tab.clone(),
                            panel: prepared
                                .focus_panel_id()
                                .cloned()
                                .ok_or(CarrierFailure::Protocol)?,
                        },
                        None => DesiredInstall::Empty,
                    };
                    let desired = transaction.prepare_desired_install(install)?;
                    let canonical = state.prepare_switch_membership_install(
                        &marker,
                        prepared,
                        desired.desired().map(|key| key.anchor().clone()),
                    )?;
                    let decision = canonical.commit();
                    desired.commit();
                    installed = true;
                    membership_decision(decision)
                }
            } else {
                let restored = match state.prepare_switch_restore(&marker, &retained_projections) {
                    Ok(canonical) => {
                        let desired = transaction.prepare_desired_install(
                            DesiredInstall::RestoreSwitch(canonical.restoration().clone()),
                        )?;
                        canonical.commit();
                        desired.commit();
                        true
                    }
                    Err(_) => false,
                };
                if context_removed {
                    state.owe_publication_read();
                }
                // A refused Switch never retires the Client. Without a usable restore point, the
                // viewers under its origin's anchor are detached first, and the origin then parks.
                if restored {
                    RefreshDecision::Continue
                } else if state.has_attachments() {
                    RefreshDecision::Cleanup(state.stage_switch_refusal_cleanup(&marker)?)
                } else {
                    park_refused_switch(state, transaction, &marker)?;
                    RefreshDecision::Continue
                }
            };
            Ok(UiReply::Switch(RefreshSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                {
                    self.menu_navigation_prepared(
                        super::action_menus::NavigationKey::Tab(marker.anchor().generation()),
                        snapshot_consumed,
                    );
                    if installed {
                        self.menu_navigation_installed(super::action_menus::NavigationKey::Tab(
                            marker.anchor().generation(),
                        ));
                    }
                }
                if snapshot_consumed && let Some(snapshot) = &index_snapshot {
                    self.agent_index = crate::agent_board::PresentationIndex::rebuild(snapshot);
                }
                if installed {
                    self.clear_projections();
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

/// Parks the origin of a refused Switch that cannot be restored, in the same mailbox transaction
/// that returns the desired key to it.
pub(super) fn park_refused_switch(
    state: &mut WorkbenchClientState,
    transaction: &mut UiReplyTransaction<'_>,
    marker: &PaneKey,
) -> Result<(), CarrierFailure> {
    let park = state.prepare_switch_park(marker)?;
    let desired = transaction
        .prepare_desired_install(DesiredInstall::RestoreSwitch(park.restoration().clone()))?;
    park.commit();
    desired.commit();
    Ok(())
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
