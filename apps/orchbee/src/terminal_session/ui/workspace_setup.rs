use super::{ActionBuildError, NativeSessionView, NativeUi, finish_action};
use crate::client_view::SessionActionDisposition;
pub(crate) use crate::terminal_session::state::workspace_setup::{
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupDraftView, NativeSetupEdit,
    NativeSetupStage, NativeSetupStatus, NativeSetupToken,
};
use crate::terminal_session::state::workspace_setup::{
    NativeSetupIntent, SetupOperation, SetupResult, SetupWorkerEvidence,
};
use crate::terminal_session::{
    mailbox::{CommandMailbox, MoveAdmissionError, MutationResult, RefreshSettlement, UiReply},
    state::{CarrierFailure, OperationDeadlines},
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSetupInputContext {
    token: NativeSetupToken,
    stage: NativeSetupStage,
    remaining_scalars: usize,
    remaining_bytes: usize,
}
impl NativeSetupInputContext {
    pub(crate) const fn token(self) -> NativeSetupToken {
        self.token
    }
    pub(crate) const fn stage(self) -> NativeSetupStage {
        self.stage
    }
    pub(crate) const fn remaining_scalars(self) -> usize {
        self.remaining_scalars
    }
    pub(crate) const fn remaining_bytes(self) -> usize {
        self.remaining_bytes
    }
    #[cfg(test)]
    pub(crate) const fn for_test(
        token: NativeSetupToken,
        stage: NativeSetupStage,
        remaining_scalars: usize,
        remaining_bytes: usize,
    ) -> Self {
        Self {
            token,
            stage,
            remaining_scalars,
            remaining_bytes,
        }
    }
}
impl<'a> NativeSessionView<'a> {
    pub(crate) fn setup_draft(&self) -> Option<NativeSetupDraftView<'a>> {
        self.state.as_ref()?.setup_draft()
    }
    pub(crate) fn setup_status(&self) -> Option<NativeSetupStatus> {
        self.state.as_ref()?.setup_status()
    }
    pub(crate) fn workspace_directory_path(&self) -> Option<&'a str> {
        self.state.as_ref()?.workspace_directory_path()
    }
    pub(crate) fn workspace_directory_unconfigured(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.workspace_directory_unconfigured())
    }
}
impl NativeUi {
    pub(in crate::terminal_session) fn setup_worker_loss(&self) -> Option<SetupWorkerEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.setup_worker_loss())
    }
    pub(in crate::terminal_session) fn finalize_setup_worker(
        &mut self,
        evidence: Option<SetupWorkerEvidence>,
    ) -> bool {
        evidence.is_none_or(|evidence| {
            self.state
                .as_mut()
                .is_some_and(|state| state.retain_setup_worker(evidence))
        })
    }
    pub(in crate::terminal_session) fn request_setup_draft(
        &mut self,
        intent: NativeSetupIntent,
    ) -> Option<NativeSetupToken> {
        if self.retirement_failure.is_some() {
            return None;
        }
        self.mailbox
            .as_ref()?
            .open_setup_draft_for(self.state.as_mut()?, intent)
    }
    pub(in crate::terminal_session) fn setup_input_context(
        &mut self,
    ) -> Option<NativeSetupInputContext> {
        let draft = self.state.as_ref()?.setup_draft()?;
        Some(NativeSetupInputContext {
            token: draft.token(),
            stage: draft.stage(),
            remaining_scalars: 512usize.saturating_sub(draft.text().chars().count()),
            remaining_bytes: 4096usize.saturating_sub(draft.text().len()),
        })
    }
    pub(in crate::terminal_session) fn edit_setup_draft(
        &mut self,
        token: NativeSetupToken,
        edit: NativeSetupEdit<'_>,
    ) -> bool {
        self.state
            .as_mut()
            .is_some_and(|state| state.edit_setup_draft(token, edit))
    }
    pub(in crate::terminal_session) fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        if let Some(state) = self.state.as_mut() {
            state.cancel_setup_draft(token);
        }
    }
    pub(in crate::terminal_session) fn after_setup_draw(
        &mut self,
        token: Option<NativeSetupToken>,
    ) {
        if let Some(state) = self.state.as_mut() {
            state.acknowledge_setup_draw(token);
        }
    }
    pub(in crate::terminal_session) fn invalidate_setup_display(&mut self) {
        self.after_setup_draw(None);
    }
    pub(in crate::terminal_session) fn submit_setup_draft(
        &mut self,
        token: NativeSetupToken,
    ) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        if state.advance_setup_picker(token) {
            return SessionActionDisposition::KeepNavigator;
        }
        match mailbox.try_enqueue_setup_for(state, token, deadlines, &self.clock) {
            Ok(()) => SessionActionDisposition::KeepNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            Err(_) => SessionActionDisposition::KeepNavigator,
        }
    }
    pub(super) fn refresh_workspace_setup(&mut self) -> SessionActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let result = mailbox.try_enqueue_context_selection_for(
            state,
            deadlines,
            &self.clock,
            |state, generation| {
                if !state.setup_recovery() || !state.start_pending_refresh(generation) {
                    return Err(ActionBuildError::Busy);
                }
                state.reset_setup_recovery_deadlines(deadlines);
                Ok(())
            },
            finish_action,
        );
        if let Err(error) = result {
            self.retire_for_admission(&error);
        }
        SessionActionDisposition::KeepNavigator
    }
    pub(super) fn apply_workspace_setup_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        operation: SetupOperation,
        outcome: SetupResult,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        match outcome {
            SetupResult::Committed { known, outcome } => {
                if known.operation != operation
                    || !state.retain_setup_worker(SetupWorkerEvidence::Known(*known))
                {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
                self.apply_refresh_event(mailbox, wait, operation.generation, *outcome)
            }
            SetupResult::NotApplied => {
                let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |_| {
                    if !state.settle_setup_not_applied(&operation) {
                        return Err(CarrierFailure::Protocol);
                    }
                    let prepared = super::super::state::PreparedRefresh::DefiniteFailure {
                        generation: operation.generation,
                        baseline_revision: state.cache().revision(),
                    };
                    let decision = state.apply_prepared_refresh(prepared);
                    Ok(UiReply::Refresh(RefreshSettlement::new(
                        decision,
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
    }
}
