use super::{
    CommandMailbox, NativeUi, OperationDeadlines, RefreshDecision, RefreshSettlement, UiReply,
};
use crate::{
    local_client::LocalOperationClock,
    terminal_session::{
        mailbox::{DesiredInstall, MutationResult, UiWaitKey},
        state::{
            CarrierFailure, MembershipInstallDecision,
            retired_membership::{RetiredMembershipOrigin, UnavailablePoll},
        },
    },
};
use orchbee_workbench_protocol::WorkbenchSnapshotOutcome;
use std::{sync::Arc, time::Instant};

pub(super) struct RetiredMembershipAttempt {
    origin: RetiredMembershipOrigin,
    generation: u64,
    deadlines: OperationDeadlines,
}

impl NativeUi {
    pub(in crate::terminal_session) fn finish_retired_peer_join(
        &mut self,
        evidence: Option<UnavailablePoll>,
        clean_return: bool,
    ) {
        self.retired_peer_attempt = None;
        if !clean_return {
            self.retired_peer = None;
            return;
        }
        if let Some(evidence) = evidence {
            self.retired_peer = if self.retirement_poll_serial == Some(evidence.poll.serial()) {
                self.state
                    .as_ref()
                    .and_then(|state| state.capture_retired_membership_origin(evidence))
            } else {
                None
            };
        }
    }

    pub(in crate::terminal_session) fn begin_retired_membership_recovery(
        &mut self,
    ) -> Option<(Arc<CommandMailbox>, LocalOperationClock, Instant)> {
        if !self.retirement_finalized
            || self.mailbox.is_some()
            || self.retired_peer_attempt.is_some()
        {
            return None;
        }
        let origin = self.retired_peer.clone()?;
        let deadlines = OperationDeadlines::from_sample(self.clock.now())?;
        let state = self.state.as_mut()?;
        if !state.retired_membership_origin_matches(&origin) {
            return None;
        }
        let mailbox = Arc::new(CommandMailbox::for_status_recovery(
            self.retired_mailbox_seed?,
            state.latest_geometry(),
        ));
        let generation = mailbox
            .try_enqueue_refresh_for(
                state,
                deadlines.forward_cutoff(),
                |state, generation| state.admit_retired_membership_refresh(&origin, generation),
                |_, _| {},
            )
            .ok()?;
        self.retired_peer_attempt = Some(RetiredMembershipAttempt {
            origin,
            generation,
            deadlines,
        });
        self.mailbox = Some(Arc::clone(&mailbox));
        self.retirement_failure = None;
        self.retirement_finalized = false;
        self.pending_draws.clear();
        self.ordinary_poll_serial = None;
        Some((mailbox, self.clock.clone(), deadlines.forward_cutoff()))
    }

    pub(super) fn apply_retired_membership_result(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: UiWaitKey,
        generation: u64,
        outcome: WorkbenchSnapshotOutcome,
    ) -> bool {
        let Some(attempt) = &self.retired_peer_attempt else {
            return false;
        };
        if attempt.generation != generation {
            return false;
        }
        let origin = attempt.origin.clone();
        let deadlines = attempt.deadlines;
        // The board's index consumes the snapshot only when the install
        // below actually commits it — a failed recovery's Retire verdict
        // never feeds it.
        let index_snapshot = match &outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => Some(snapshot.clone()),
            _ => None,
        };
        let prepared = match outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => {
                let state = self.state.as_ref().ok_or(CarrierFailure::Protocol);
                state.and_then(|state| {
                    match state.cache().project_snapshot(&snapshot) {
                        Ok(candidate) => {
                            state.prepare_retired_membership_candidate(&origin, candidate)
                        }
                        // The origin context no longer projects: classify a peer
                        // workspace-close removal before treating the reply as
                        // divergent — the same permit discipline authorizes it.
                        Err(_) => {
                            state.prepare_retired_context_removal(&origin, generation, &snapshot)
                        }
                    }
                })
            }
            _ => Err(CarrierFailure::AuthorityDivergence),
        };
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut installed = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            // This exact Refresh wait is the attempt marker; the old origin never confers cleanup.
            if transaction.observed_at() >= deadlines.forward_cutoff()
                || !state.retired_membership_origin_matches(&origin)
                || prepared.is_err()
            {
                return Ok(UiReply::Refresh(RefreshSettlement::new(
                    RefreshDecision::Retire,
                    None,
                )));
            }
            let prepared = prepared?;
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
            let desired_plan = transaction.prepare_desired_install(install)?;
            let state_plan = state.prepare_retired_membership_install(
                &origin,
                generation,
                prepared,
                desired_plan.desired().map(|key| key.anchor().clone()),
                deadlines,
            )?;
            let decision = state_plan.commit();
            desired_plan.commit();
            installed = true;
            let decision = match decision {
                MembershipInstallDecision::Attach(target) => {
                    RefreshDecision::AttachRecovered(target)
                }
                MembershipInstallDecision::Empty
                | MembershipInstallDecision::SelectedWithoutAttachment => {
                    RefreshDecision::SettledWithoutAttachment
                }
                MembershipInstallDecision::Preserved => return Err(CarrierFailure::Protocol),
            };
            Ok(UiReply::Refresh(RefreshSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) if installed => {
                if let Some(snapshot) = &index_snapshot {
                    self.agent_index =
                        super::super::agent_board::PresentationIndex::rebuild(snapshot);
                }
                self.retired_peer = None;
                self.retired_peer_attempt = None;
                self.retirement = None;
                self.clear_projections();
                self.validate_state()
            }
            Ok(MutationResult::Applied) => {
                self.retire(CarrierFailure::AuthorityDivergence);
                true
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}
