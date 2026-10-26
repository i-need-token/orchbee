use super::{ActionBuildError, ModalPresentation, NativeUi, WorkbenchClientView, finish_action};
use crate::client_view::ClientActionDisposition;
pub(crate) use crate::workbench_client::state::workspace_setup::{
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupDraftView, NativeSetupEdit,
    NativeSetupStage, NativeSetupStatus, NativeSetupToken,
};
use crate::workbench_client::state::workspace_setup::{
    NativeSetupIntent, SetupOperation, SetupResult, SetupWorkerEvidence,
};
pub(crate) use crate::workbench_client::state::workspace_setup::{
    WorkspaceCreationPhase, WorkspaceCreationPreview,
};
use crate::workbench_client::{
    mailbox::{CommandMailbox, MoveAdmissionError, MutationResult, RefreshSettlement, UiReply},
    state::{CarrierFailure, OperationDeadlines},
};
use orchbee_workbench_protocol::{MAXIMUM_DIRECTORY_PATH_BYTES, MAXIMUM_LABEL_LENGTH};
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
}
impl<'a> WorkbenchClientView<'a> {
    /// Creation feedback never exposes a retired or ambiguous operation as a live target.
    pub(crate) fn workspace_creation_preview(&self) -> Option<WorkspaceCreationPreview<'a>> {
        if self.retirement_requested || self.retirement.is_some() {
            return None;
        }
        self.state.as_ref()?.workspace_creation_preview()
    }
    pub(crate) fn quick_workspace_recovery(
        &self,
    ) -> Option<super::super::state::workspace_setup::quick::QuickRecovery<'a>> {
        self.state.as_ref()?.quick_workspace_recovery()
    }
    pub(crate) fn setup_draft(&self) -> Option<NativeSetupDraftView<'a>> {
        self.state.as_ref()?.setup_draft()
    }
    pub(crate) fn setup_capture_claim(&self) -> bool {
        let draft = self.setup_draft();
        let draft = draft.filter(|draft| !draft.background());
        draft.is_some()
    }
    pub(crate) fn setup_input_context(&self) -> Option<NativeSetupInputContext> {
        let draft = self.setup_draft()?;
        if draft.background() {
            return None;
        }
        draft.interactive().then_some(NativeSetupInputContext {
            token: draft.token(),
            stage: draft.stage(),
            remaining_scalars: MAXIMUM_LABEL_LENGTH.saturating_sub(draft.text().chars().count()),
            remaining_bytes: MAXIMUM_DIRECTORY_PATH_BYTES.saturating_sub(draft.text().len()),
        })
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
    /// The foreground creation projection has no live terminal target. Keep
    /// this admission fact independent of modal capture so navigation remains usable.
    pub(super) fn workspace_creation_replaces_content(&self) -> bool {
        {
            self.view()
                .workspace_creation_preview()
                .is_some_and(WorkspaceCreationPreview::replaces_content)
        }
    }

    pub(in crate::workbench_client) fn choose_setup_project(
        &mut self,
        token: NativeSetupToken,
        index: usize,
    ) {
        if self.view().setup_draft().is_some_and(|draft| {
            matches!(
                draft.stage(),
                NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory
            )
        }) {
            self.edit_setup_draft(token, NativeSetupEdit::SelectDirectory(index));
            return;
        }
        if self
            .state
            .as_mut()
            .is_some_and(|state| state.choose_setup_project(token, index))
        {
            self.submit_setup_draft(token);
        }
    }

    pub(in crate::workbench_client) fn scroll_setup_projects(
        &mut self,
        token: NativeSetupToken,
        up: bool,
        lines: usize,
    ) -> Option<NativeSetupToken> {
        let state = self.state.as_mut()?;
        state
            .scroll_setup_projects(token, up, lines)
            .then(|| state.setup_draft().map(|draft| draft.token()))
            .flatten()
    }

    pub(super) fn request_workspace_organization(
        &mut self,
        request: Box<crate::workbench_client::WorkspaceOrganizationRequest>,
    ) -> bool {
        if self.clock.now() >= request.deadline() {
            return false;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return false;
        };
        if request.direct() {
            if self.modal_owns_input() {
                return false;
            }
            let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
                return false;
            };
            return mailbox
                .try_enqueue_sidebar_organization_for(state, &request, deadlines, &self.clock)
                .is_ok();
        }
        let Some(token) = self.request_setup_draft(NativeSetupIntent::Organize(request)) else {
            return false;
        };
        if self
            .state
            .as_mut()
            .is_some_and(|state| state.arm_project_dissolve(token, deadlines))
        {
            self.submit_setup_draft(token);
        }
        true
    }
    pub(in crate::workbench_client) fn cancel_project_quick_terminal_entry(&mut self) {
        if let Some(state) = self.state.as_mut() {
            state.cancel_project_quick_entry();
        }
    }

    pub(in crate::workbench_client) fn take_project_quick_terminal_entry(&mut self) -> bool {
        // A newer native surface retires the return even if it closes before
        // attachment becomes Ready. The operation's own receipt is not a modal.
        if self.retirement_failure.is_some()
            || self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            self.cancel_project_quick_terminal_entry();
            return false;
        }
        self.state
            .as_mut()
            .is_some_and(|state| state.take_project_quick_terminal_entry())
    }

    pub(super) fn request_project_rename(
        &mut self,
        request: Box<crate::workbench_client::ProjectRenameRequest>,
    ) -> Option<NativeSetupToken> {
        if self.retirement_failure.is_some() {
            return None;
        }
        self.mailbox.as_ref()?.open_project_rename_draft_for(
            self.state.as_mut()?,
            request,
            &self.clock,
        )
    }
    pub(in crate::workbench_client) fn request_quick_workspace(&mut self) -> bool {
        self.request_quick_workspace_for(None)
    }
    pub(super) fn request_quick_workspace_for(
        &mut self,
        request: Option<crate::workbench_client::ProjectQuickWorkspaceRequest>,
    ) -> bool {
        // Quick creation submits its prepared defaults; only the explicit
        // at-directory request opens the directory review.
        self.request_quick_workspace_with_review(request, false)
    }

    pub(super) fn request_quick_workspace_with_review(
        &mut self,
        request: Option<crate::workbench_client::ProjectQuickWorkspaceRequest>,
        review_directory: bool,
    ) -> bool {
        if self.retirement_failure.is_some() {
            return false;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return false;
        };
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return false;
        };
        if review_directory {
            mailbox.try_enqueue_quick_preparation_with_review(
                state,
                deadlines,
                &self.clock,
                request,
                true,
            )
        } else {
            mailbox.try_enqueue_quick_preparation_for(state, deadlines, &self.clock, request)
        }
    }
    pub(super) fn apply_quick_preparation_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        preparation: super::super::state::workspace_setup::quick::QuickPreparation,
        cwd: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
        outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let Some(active) = state.take_quick_preparation(&preparation) else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let changed = self.apply_refresh_event(mailbox, wait, preparation.generation, outcome);
        if active
            && self.retirement_failure.is_none()
            && let Some(state) = self.state.as_mut()
        {
            state.finish_quick_preparation(
                &preparation,
                cwd,
                crate::localization::select_locale(|name| std::env::var_os(name)),
                ["USERPROFILE", "HOMEDRIVE", "HOMEPATH", "HOME"]
                    .into_iter()
                    .filter_map(|name| std::env::var_os(name).map(|value| (name.to_owned(), value)))
                    .collect(),
            );
        }
        if let Some((token, _)) = self
            .state
            .as_ref()
            .and_then(|state| state.quick_continuation())
        {
            self.submit_setup_draft(token);
        }
        changed || active
    }
    pub(in crate::workbench_client) fn setup_worker_loss(&self) -> Option<SetupWorkerEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.setup_worker_loss())
    }
    pub(in crate::workbench_client) fn finalize_setup_worker(
        &mut self,
        evidence: Option<SetupWorkerEvidence>,
    ) -> bool {
        evidence.is_none_or(|evidence| {
            self.state
                .as_mut()
                .is_some_and(|state| state.retain_setup_worker(evidence))
        })
    }
    pub(in crate::workbench_client) fn request_setup_draft(
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
    #[cfg(test)]
    pub(in crate::workbench_client) fn setup_capture_claim(&self) -> bool {
        self.view().setup_capture_claim()
    }
    pub(in crate::workbench_client) fn setup_input_context(
        &self,
    ) -> Option<NativeSetupInputContext> {
        self.view().setup_input_context()
    }
    pub(in crate::workbench_client) fn edit_setup_draft(
        &mut self,
        token: NativeSetupToken,
        edit: NativeSetupEdit<'_>,
    ) -> bool {
        let Some(draft) = self.state.as_ref().and_then(|state| state.setup_draft()) else {
            return false;
        };
        if draft.token() != token {
            return false;
        }
        if !draft.interactive() {
            return true;
        }
        self.state
            .as_mut()
            .is_some_and(|state| state.edit_setup_draft(token, edit))
    }
    pub(in crate::workbench_client) fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        if let Some(state) = self.state.as_mut() {
            state.cancel_setup_draft(token);
        }
    }
    pub(in crate::workbench_client) fn after_setup_draw(
        &mut self,
        token: Option<ModalPresentation<NativeSetupToken>>,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match token {
            Some(ModalPresentation::Presented(token)) => {
                state.acknowledge_setup_draw(Some(token));
            }
            Some(ModalPresentation::NotPresented) => {
                state.acknowledge_setup_draw(None);
                let _ = state.settle_setup_draft_not_presented();
            }
            None => state.acknowledge_setup_draw(None),
        }
    }
    pub(in crate::workbench_client) fn invalidate_setup_display(&mut self) {
        self.after_setup_draw(None);
    }
    pub(in crate::workbench_client) fn submit_setup_draft(
        &mut self,
        token: NativeSetupToken,
    ) -> ClientActionDisposition {
        let hidden_unaccepted = self.state.as_ref().is_some_and(|state| {
            state
                .setup_draft()
                .is_some_and(|draft| draft.token() == token && !draft.interactive())
                && state.quick_continuation().is_none()
        });
        if hidden_unaccepted {
            return ClientActionDisposition::KeepNavigator;
        }
        if self.state.as_mut().is_some_and(|state| {
            state.settle_stale_quick_workspace() | state.settle_stale_project_rename()
        }) {
            return ClientActionDisposition::KeepNavigator;
        }
        let continuation = self
            .state
            .as_ref()
            .and_then(|state| state.quick_continuation())
            .filter(|(current, _)| *current == token)
            .map(|(_, deadlines)| deadlines);
        let Some(deadlines) =
            continuation.or_else(|| OperationDeadlines::from_sample(self.clock.now()))
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        if state.advance_setup_picker(token) {
            return ClientActionDisposition::KeepNavigator;
        }
        match mailbox.try_enqueue_setup_for(state, token, deadlines, &self.clock) {
            Ok(()) => ClientActionDisposition::KeepNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                ClientActionDisposition::KeepNavigator
            }
            Err(_) => {
                if continuation.is_some() {
                    state.abort_quick_continuation(token);
                }
                ClientActionDisposition::KeepNavigator
            }
        }
    }
    pub(super) fn refresh_workspace_setup(&mut self) -> ClientActionDisposition {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
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
        ClientActionDisposition::KeepNavigator
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
                let changed =
                    self.apply_refresh_event(mailbox, wait, operation.generation, *outcome);
                if let Some((token, _)) = self
                    .state
                    .as_ref()
                    .and_then(|state| state.quick_continuation())
                {
                    self.submit_setup_draft(token);
                }
                changed
            }
            result @ (SetupResult::NotApplied
            | SetupResult::Stale
            | SetupResult::ConfirmationRequired) => {
                let review_delete = matches!(result, SetupResult::ConfirmationRequired);
                let stale = matches!(result, SetupResult::Stale);
                let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
                    if !state.settle_setup_refusal(&operation, stale) {
                        return Err(CarrierFailure::Protocol);
                    }
                    let prepared = super::super::state::PreparedRefresh::DefiniteFailure {
                        generation: operation.generation,
                        baseline_revision: state.cache().revision(),
                    };
                    let decision =
                        state.apply_prepared_refresh(prepared, transaction.observed_at());
                    Ok(UiReply::Refresh(RefreshSettlement::new(
                        decision,
                        state.desired().cloned(),
                    )))
                });
                match result {
                    Ok(MutationResult::Applied) => {
                        if review_delete {
                            // The conditional command definitely had no effect. This creates a new
                            // prompt/token; only its complete successful draw plus explicit Enter
                            // can authorize a distinct unconditional request.
                            let _ = state
                                .review_project_delete(&operation, operation.deadlines.total());
                        }
                        self.validate_state()
                    }
                    Ok(MutationResult::Stale) => false,
                    Err(error) => self.handle_event_error(error),
                }
            }
        }
    }
}
