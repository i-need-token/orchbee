use super::{
    Arc, CarrierFailure, CommandMailbox, CreateOperationKey, CreateProjectionDecision,
    MutationResult, NativeUi, OperationDeadlines, RefreshDecision, RefreshSettlement, UiReply,
};

impl NativeUi {
    #[expect(clippy::too_many_arguments)]
    pub(super) fn apply_create_recovery_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        generation: u64,
        operation: CreateOperationKey,
        prepared: super::super::state::PreparedMembershipCandidate,
        deadlines: Option<OperationDeadlines>,
        index_snapshot: Option<orchbee_workbench_protocol::Snapshot>,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut installed = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            let deadlines = deadlines.ok_or(CarrierFailure::Deadline)?;
            if transaction.observed_at() >= deadlines.forward_cutoff() {
                return Err(CarrierFailure::Deadline);
            }
            if state.has_attachments() {
                let cleanup = state
                    .stage_create_refresh_cleanup(generation, &operation, prepared, deadlines)?;
                return Ok(UiReply::Refresh(RefreshSettlement::new(
                    RefreshDecision::Cleanup(cleanup),
                    state.desired().cloned(),
                )));
            }
            let committed = state
                .create_membership_commit()
                .ok_or(CarrierFailure::Protocol)?;
            let install = match prepared.selected_tab_id() {
                Some(tab) if tab == committed.committed_key().tab_id() => {
                    super::super::mailbox::DesiredInstall::ReuseReserved {
                        key: committed.committed_key().clone(),
                        panel: prepared
                            .focus_panel_id()
                            .cloned()
                            .ok_or(CarrierFailure::Protocol)?,
                    }
                }
                Some(tab) => match prepared.focus_panel_id() {
                    Some(panel) => super::super::mailbox::DesiredInstall::Fresh {
                        tab_id: tab.clone(),
                        panel: panel.clone(),
                    },
                    None => super::super::mailbox::DesiredInstall::Empty,
                },
                None => super::super::mailbox::DesiredInstall::Empty,
            };
            let desired = transaction.prepare_desired_install(install)?;
            let plan = state.prepare_create_refresh_install(
                generation,
                &operation,
                prepared,
                desired.desired().map(|key| key.anchor().clone()),
                deadlines,
            )?;
            let decision = plan.commit();
            desired.commit();
            installed = true;
            let decision = match decision {
                super::super::state::MembershipInstallDecision::Attach(target) => {
                    RefreshDecision::AttachRecovered(target)
                }
                super::super::state::MembershipInstallDecision::Empty
                | super::super::state::MembershipInstallDecision::SelectedWithoutAttachment => {
                    RefreshDecision::SettledWithoutAttachment
                }
                super::super::state::MembershipInstallDecision::Preserved => {
                    unreachable!("Create recovery always installs")
                }
            };
            Ok(UiReply::Refresh(RefreshSettlement::new(
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                // The snapshot that drove this accepted recovery is the
                // freshest canonical feed the board's index may see.
                if let Some(snapshot) = &index_snapshot {
                    self.agent_index =
                        super::super::agent_board::PresentationIndex::rebuild(snapshot);
                }
                if installed {
                    self.clear_projections();
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }

    pub(super) fn apply_create_snapshot_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        operation: CreateOperationKey,
        outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // The board's presentation index consumes the snapshot only when
        // the accepted reply used it — a refused or divergent create
        // outcome never feeds it.
        let index_snapshot = match &outcome {
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available { snapshot } => {
                Some(snapshot.clone())
            }
            _ => None,
        };
        let prepared = state.prepare_create_snapshot(&operation, outcome);
        if let super::super::state::PreparedCreateSnapshot::Available {
            baseline,
            committed,
            projection,
        } = prepared
        {
            let candidate =
                state.prepare_create_membership_candidate(&baseline, &committed, projection);
            let candidate = match candidate {
                Ok(candidate) => candidate,
                Err(failure) => {
                    self.retire(failure);
                    return true;
                }
            };
            let deadlines = committed.operation().deadlines();
            let mut installed = false;
            let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
                if transaction.observed_at() >= deadlines.total() {
                    return Err(CarrierFailure::Deadline);
                }
                if state.has_attachments() {
                    let cleanup =
                        state.stage_create_membership_cleanup(&operation, candidate, deadlines)?;
                    return Ok(UiReply::CreateProjection(
                        CreateProjectionDecision::Cleanup(cleanup),
                    ));
                }
                if transaction.observed_at() >= deadlines.forward_cutoff() {
                    return Err(CarrierFailure::Deadline);
                }
                let install = match candidate.selected_tab_id() {
                    Some(tab) if tab == committed.committed_key().tab_id() => {
                        super::super::mailbox::DesiredInstall::ReuseReserved {
                            key: committed.committed_key().clone(),
                            panel: candidate
                                .focus_panel_id()
                                .cloned()
                                .ok_or(CarrierFailure::Protocol)?,
                        }
                    }
                    Some(tab) => match candidate.focus_panel_id() {
                        Some(panel) => super::super::mailbox::DesiredInstall::Fresh {
                            tab_id: tab.clone(),
                            panel: panel.clone(),
                        },
                        None => super::super::mailbox::DesiredInstall::Empty,
                    },
                    None => super::super::mailbox::DesiredInstall::Empty,
                };
                let desired = transaction.prepare_desired_install(install)?;
                let plan = state.prepare_create_membership_install(
                    &operation,
                    candidate,
                    desired.desired().map(|key| key.anchor().clone()),
                    deadlines,
                )?;
                let decision = plan.commit();
                desired.commit();
                installed = true;
                Ok(UiReply::CreateProjection(match decision {
                    super::super::state::MembershipInstallDecision::Attach(target) => {
                        CreateProjectionDecision::Attach(target)
                    }
                    super::super::state::MembershipInstallDecision::SelectedWithoutAttachment
                    | super::super::state::MembershipInstallDecision::Empty => {
                        CreateProjectionDecision::SettledWithoutAttachment
                    }
                    super::super::state::MembershipInstallDecision::Preserved => {
                        unreachable!("Create always installs a new selection")
                    }
                }))
            });
            return match result {
                Ok(MutationResult::Applied) => {
                    if let Some(snapshot) = &index_snapshot {
                        self.agent_index =
                            super::super::agent_board::PresentationIndex::rebuild(snapshot);
                    }
                    if installed {
                        self.clear_projections();
                    }
                    self.validate_state()
                }
                Ok(MutationResult::Stale) => false,
                Err(error) => self.handle_event_error(error),
            };
        }
        let mut failure = None;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |_| {
            let decision = state.apply_prepared_create_snapshot(prepared);
            if decision == CreateProjectionDecision::Retire {
                failure = state.retired_failure();
            }
            Ok(UiReply::CreateProjection(decision))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if let Some(failure) = failure {
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
}
