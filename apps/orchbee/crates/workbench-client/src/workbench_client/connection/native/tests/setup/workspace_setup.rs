use super::*;
use crate::workbench_client::mailbox::DesiredInstall;
use crate::workbench_state::workspace_setup::{
    SetupEdit, SetupResult, SetupStatus, SetupToken, SetupWorkerEvidence,
};
use crate::workbench_state::{MembershipInstallDecision, PreparedRefresh};

impl UiDriver {
    pub(in crate::workbench_client::connection::native::tests) fn setup_group_draft(
        &mut self,
        label: &str,
        drawn: bool,
    ) -> TestResult<SetupToken> {
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        let intent = crate::workbench_state::workspace_setup::SetupIntent::TabGroupName(Box::new(
            orchbee_workbench_protocol::TabConfiguration::CreateGroup {
                tab_id: state.cache().tabs()[0].tab_id().clone(),
                label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse(label).unwrap(),
                color: orchbee_workbench_protocol::TabGroupColor::Blue,
            },
        ));
        let token = mailbox
            .open_setup_draft_for(state, intent)
            .ok_or_else(|| test_error("group draft admission"))?;
        if drawn {
            state.acknowledge_setup_draw(Some(token));
        }
        Ok(token)
    }

    pub(in crate::workbench_client::connection::native::tests) fn enqueue_setup_token(
        &mut self,
        token: SetupToken,
    ) -> TestResult {
        let clock = self.clock.clone();
        let deadlines = OperationDeadlines::from_sample(clock.now())
            .ok_or_else(|| test_error("setup budget"))?;
        let mailbox = Arc::clone(&self.mailbox);
        mailbox
            .try_enqueue_setup_for(self.state_mut(), token, deadlines, &clock)
            .map_err(|error| test_error(&format!("setup admission: {error:?}")))
    }

    pub(in crate::workbench_client::connection::native::tests) fn draw_setup(
        &mut self,
        token: SetupToken,
    ) {
        self.state_mut().acknowledge_setup_draw(Some(token));
    }

    // The explicit recovery action uses the same admission and frozen-state checks as WorkbenchClientUi.
    pub(in crate::workbench_client::connection::native::tests) fn refresh_known_setup(
        &mut self,
    ) -> TestResult {
        let clock = self.clock.clone();
        let deadlines = OperationDeadlines::from_sample(clock.now())
            .ok_or_else(|| test_error("recovery budget"))?;
        let mailbox = Arc::clone(&self.mailbox);
        mailbox
            .try_enqueue_context_selection_for(
                self.state_mut(),
                deadlines,
                &clock,
                |state, generation| {
                    if !state.setup_recovery() || !state.start_pending_refresh(generation) {
                        return Err("setup recovery admission");
                    }
                    state.reset_setup_recovery_deadlines(deadlines);
                    Ok(())
                },
                |_, _| {},
            )
            .map_err(|error| test_error(&format!("setup recovery: {error:?}")))?;
        Ok(())
    }

    // This driver uses the production state reducer and mailbox reply transaction. These traces
    // preserve the current selection; context-changing cleanup remains covered by its own driver.
    pub(in crate::workbench_client::connection::native::tests) fn settle_setup_next(
        &mut self,
    ) -> TestResult {
        let (wait, event) = self.take_next()?.into_parts();
        let wait = wait.ok_or_else(|| test_error("setup refresh wait"))?;
        let (generation, prepared) = match event {
            WorkerEvent::WorkspaceSetupResult { operation, outcome } => {
                let prepared = match outcome {
                    SetupResult::Committed { known, outcome } => {
                        assert_eq!(known.operation, operation);
                        assert!(
                            self.state_mut()
                                .retain_setup_worker(SetupWorkerEvidence::Known(*known))
                        );
                        self.state().prepare_refresh(operation.generation, *outcome)
                    }
                    result @ (SetupResult::NotApplied
                    | SetupResult::LaunchRequestTooLarge
                    | SetupResult::Stale
                    | SetupResult::ConfirmationRequired) => {
                        let stale = matches!(result, SetupResult::Stale);
                        assert!(self.state_mut().settle_setup_refusal(&operation, stale));
                        if matches!(result, SetupResult::LaunchRequestTooLarge) {
                            self.state_mut().setup_launch_too_large();
                        }
                        PreparedRefresh::DefiniteFailure {
                            generation: operation.generation,
                            baseline_revision: self.state().cache().revision(),
                        }
                    }
                };
                (operation.generation, prepared)
            }
            WorkerEvent::RefreshSnapshotResult {
                generation,
                outcome,
            } => (
                generation,
                self.state().prepare_refresh(generation, outcome),
            ),
            _ => {
                return Err(test_error(
                    "expected setup result or explicit recovery snapshot",
                ));
            }
        };
        let mailbox = Arc::clone(&self.mailbox);
        let clock = self.clock.clone();
        let state = self.state_mut();
        let deadlines = state.setup_deadlines();
        require_applied(
            mailbox.deposit_ui_reply_before(&wait, &clock, |transaction| {
                let decision = if let PreparedRefresh::Membership {
                    generation: actual,
                    prepared,
                } = prepared
                {
                    assert_eq!(actual, generation);
                    assert!(prepared.preserves_selection());
                    if deadlines
                        .is_some_and(|value| transaction.observed_at() >= value.forward_cutoff())
                    {
                        return Err(CarrierFailure::Deadline);
                    }
                    let desired = transaction.prepare_desired_install(DesiredInstall::Preserve)?;
                    let plan = state.prepare_refresh_membership_install(
                        generation,
                        prepared,
                        desired.desired().map(|pane| pane.anchor().clone()),
                        deadlines,
                    )?;
                    assert!(matches!(
                        plan.commit(),
                        MembershipInstallDecision::Preserved
                    ));
                    desired.commit();
                    RefreshDecision::Continue
                } else {
                    state.apply_prepared_refresh(prepared, transaction.observed_at())
                };
                Ok(UiReply::Refresh(RefreshSettlement::new(
                    decision,
                    state.desired().cloned(),
                )))
            }),
        )?;
        assert!(self.state().invariant_holds());
        Ok(())
    }
}

impl Harness {
    pub(in crate::workbench_client::connection::native::tests) fn finish_with_setup_not_applied(
        mut self,
        expected: CarrierFailure,
    ) -> TestResult<WorkbenchClientState> {
        self.worker_done.recv_timeout(WALL_GUARD)?;
        let mut exit = self
            .worker
            .take()
            .ok_or_else(|| test_error("setup worker already joined"))?
            .join()
            .map_err(|_| test_error("setup worker panicked"))?;
        assert_eq!(self.mailbox.is_closed(), Some(expected));
        let evidence = exit
            .take_setup_result()
            .ok_or_else(|| test_error("definite setup result was lost"))?;
        assert!(matches!(&evidence, SetupWorkerEvidence::NotApplied(_)));
        assert_eq!(self.mailbox.setup_worker_loss(), Some(evidence.clone()));
        assert!(self.ui.state_mut().retain_setup_worker(evidence));
        assert_eq!(
            self.ui.state().setup_status(),
            Some(SetupStatus::TabsNotApplied)
        );
        assert!(!self.ui.state().setup_pending_or_draft());
        let state = self
            .ui
            .state
            .take()
            .ok_or_else(|| test_error("canonical setup state"))?;
        self.shutdown.close()?;
        self.carrier.finish()?;
        Ok(state)
    }
}

impl UiDriver {
    pub(in crate::workbench_client::connection::native::tests) fn location_label_draft(
        &mut self,
        target: crate::workbench_state::workspace_setup::LocationLabelTarget,
        replacement: Option<&str>,
    ) -> TestResult<SetupToken> {
        use crate::workbench_state::workspace_setup::SetupIntent;
        let mailbox = Arc::clone(&self.mailbox);
        let state = self.state_mut();
        let token = mailbox
            .open_setup_draft_for(state, SetupIntent::RenameLocation(target))
            .ok_or_else(|| test_error("location label draft admission"))?;
        state.acknowledge_setup_draw(Some(token));
        if let Some(label) = replacement {
            assert!(state.edit_setup_draft(token, SetupEdit::Clear));
            assert!(state.edit_setup_draft(token, SetupEdit::Append(label)));
        }
        Ok(token)
    }
}
