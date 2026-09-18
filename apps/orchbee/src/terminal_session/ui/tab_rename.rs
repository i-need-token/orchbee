use super::*;
use crate::terminal_session::state::rename_draft::{
    RenameDraftEdit, RenameDraftToken, RenameDraftView,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeRenameDraftToken(RenameDraftToken);

pub(crate) struct NativeRenameDraftView<'a>(RenameDraftView<'a>);
impl<'a> NativeRenameDraftView<'a> {
    pub(crate) fn token(&self) -> NativeRenameDraftToken {
        NativeRenameDraftToken(self.0.token())
    }
    pub(crate) fn target_label(&self) -> &'a str {
        self.0.target_label()
    }
    pub(crate) fn text(&self) -> &'a str {
        self.0.text()
    }
    pub(crate) fn input_fault(&self) -> bool {
        self.0.input_fault()
    }
    pub(crate) fn label_valid(&self) -> bool {
        self.0.label_valid()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeRenameInputContext {
    token: NativeRenameDraftToken,
    remaining_scalar_capacity: usize,
}
impl NativeRenameInputContext {
    pub(crate) const fn token(self) -> NativeRenameDraftToken {
        self.token
    }
    pub(crate) const fn remaining_scalar_capacity(self) -> usize {
        self.remaining_scalar_capacity
    }
}

pub(crate) enum NativeRenameEdit<'a> {
    Append(&'a str),
    Paste(&'a str),
    Backspace,
    Clear,
    Left,
    Right,
    Home,
    End,
    Delete,
    WordBackspace,
    Fault,
    Completeness(bool),
}

impl<'a> NativeSessionView<'a> {
    pub(crate) fn rename_draft(&self) -> Option<NativeRenameDraftView<'a>> {
        self.state
            .as_ref()?
            .rename_draft()
            .map(NativeRenameDraftView)
    }
}

impl NativeUi {
    #[cfg(test)]
    pub(in crate::terminal_session) fn rename_receipts_for_test(
        &self,
    ) -> (
        Option<&crate::terminal_session::state::tab_rename::RenameOperation>,
        Option<&crate::terminal_session::state::tab_rename::PendingRenameCommit>,
        Option<&crate::terminal_session::state::tab_rename::RenameOperation>,
    ) {
        self.state.as_ref().map_or(
            (None, None, None),
            NativeSessionState::rename_receipts_for_test,
        )
    }
    pub(in crate::terminal_session) fn rename_worker_loss(
        &self,
    ) -> Option<crate::terminal_session::state::tab_rename::RenameWorkerEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.rename_worker_loss())
    }
    pub(in crate::terminal_session) fn finalize_rename_worker(
        &mut self,
        evidence: Option<crate::terminal_session::state::tab_rename::RenameWorkerEvidence>,
    ) -> bool {
        evidence.is_none_or(|evidence| {
            self.state
                .as_mut()
                .is_some_and(|state| state.retain_rename_worker(evidence))
        })
    }
    pub(in crate::terminal_session) fn request_rename_draft(
        &mut self,
    ) -> Option<NativeRenameDraftToken> {
        if self.retirement_failure.is_some() {
            return None;
        }
        let state = self.state.as_mut()?;
        self.mailbox
            .as_ref()?
            .open_rename_draft_for(state)
            .map(NativeRenameDraftToken)
    }
    pub(in crate::terminal_session) fn rename_input_context(
        &mut self,
    ) -> Option<NativeRenameInputContext> {
        let state = self.state.as_mut()?;
        if state.rename_draft().is_none() {
            state.cancel_rename_draft();
            return None;
        }
        let draft = state.rename_draft()?;
        Some(NativeRenameInputContext {
            token: NativeRenameDraftToken(draft.token()),
            remaining_scalar_capacity: 512 - draft.text().chars().count(),
        })
    }
    pub(in crate::terminal_session) fn edit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
        edit: NativeRenameEdit<'_>,
    ) -> bool {
        self.state.as_mut().is_some_and(|state| {
            state.edit_rename_draft(
                token.0,
                match edit {
                    NativeRenameEdit::Append(text) => RenameDraftEdit::Append(text),
                    NativeRenameEdit::Paste(text) => RenameDraftEdit::Paste(text),
                    NativeRenameEdit::Backspace => RenameDraftEdit::Backspace,
                    NativeRenameEdit::Clear => RenameDraftEdit::Clear,
                    NativeRenameEdit::Left => RenameDraftEdit::Left,
                    NativeRenameEdit::Right => RenameDraftEdit::Right,
                    NativeRenameEdit::Home => RenameDraftEdit::Home,
                    NativeRenameEdit::End => RenameDraftEdit::End,
                    NativeRenameEdit::Delete => RenameDraftEdit::Delete,
                    NativeRenameEdit::WordBackspace => RenameDraftEdit::WordBackspace,
                    NativeRenameEdit::Fault => RenameDraftEdit::Fault,
                    NativeRenameEdit::Completeness(value) => RenameDraftEdit::Completeness(value),
                },
            )
        })
    }
    pub(in crate::terminal_session) fn cancel_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) {
        if let Some(state) = self.state.as_mut()
            && state
                .rename_draft()
                .is_some_and(|draft| draft.token() == token.0)
        {
            state.cancel_rename_draft();
        }
    }
    pub(in crate::terminal_session) fn after_rename_draw(
        &mut self,
        token: Option<ModalPresentation<NativeRenameDraftToken>>,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match token {
            Some(ModalPresentation::Presented(token)) => {
                state.acknowledge_rename_draw(Some(token.0));
            }
            Some(ModalPresentation::NotPresented) => {
                state.acknowledge_rename_draw(None);
                state.settle_rename_draft_not_presented();
            }
            None => state.acknowledge_rename_draw(None),
        }
    }
    pub(in crate::terminal_session) fn invalidate_rename_display(&mut self) {
        self.after_rename_draw(None);
    }
    pub(in crate::terminal_session) fn submit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) -> SessionActionDisposition {
        let Some(deadline) = self
            .clock
            .now()
            .checked_add(super::super::state::OPERATION_TOTAL)
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        match mailbox.try_enqueue_rename_for(state, token.0, deadline, &self.clock) {
            Ok(_) => SessionActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                SessionActionDisposition::KeepNavigator
            }
            Err(_) => SessionActionDisposition::KeepNavigator,
        }
    }
}
use crate::terminal_session::{
    mailbox::{DesiredInstall, RenameSettlement},
    state::{
        MembershipInstallDecision,
        tab_rename::{RenameOperation, RenamePreparation, RenameResult},
    },
};

impl NativeUi {
    pub(super) fn apply_rename_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        operation: RenameOperation,
        outcome: RenameResult,
    ) -> bool {
        let prepared = operation.prepare(outcome);
        let (_, result) = prepared.into_parts();
        let (result, candidate) = match result {
            RenamePreparation::Committed {
                known,
                projection: Some(projection),
            } => {
                let candidate = self
                    .state
                    .as_ref()
                    .ok_or(CarrierFailure::Protocol)
                    .and_then(|state| {
                        state.prepare_membership_candidate(
                            projection.candidate,
                            Some(operation.target().tab_id()),
                            operation.baseline().tab_index(operation.target().tab_id()),
                            None,
                        )
                    });
                (
                    RenamePreparation::Committed {
                        known,
                        projection: None,
                    },
                    Some(candidate),
                )
            }
            result => (result, None),
        };
        let needs_budget = candidate.as_ref().is_some_and(|candidate| {
            candidate
                .as_ref()
                .is_ok_and(|candidate| !candidate.preserves_selection())
        });
        let deadlines = needs_budget
            .then(|| OperationDeadlines::from_sample(self.clock.now()))
            .flatten();
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        let mut changed_selection = false;
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            if !state.rename_is_current(&operation) {
                return Err(CarrierFailure::Protocol);
            }
            let decision = if let Some(candidate) = candidate {
                let RenamePreparation::Committed { known, .. } = result else {
                    return Err(CarrierFailure::Protocol);
                };
                if !state.retain_pending_rename(known) {
                    return Err(CarrierFailure::Protocol);
                }
                let candidate = candidate?;
                let preserve = candidate.preserves_selection();
                if !preserve && deadlines.is_none() {
                    return Err(CarrierFailure::Deadline);
                }
                if deadlines.is_some_and(|deadlines| {
                    transaction.observed_at() >= deadlines.forward_cutoff()
                }) {
                    return Err(CarrierFailure::Deadline);
                }
                changed_selection = !preserve;
                if !preserve && state.attached_key().is_some() {
                    RefreshDecision::Cleanup(state.stage_rename_membership_cleanup(
                        operation.request_id(),
                        candidate,
                        deadlines.ok_or(CarrierFailure::Deadline)?,
                    )?)
                } else {
                    let install = if preserve {
                        DesiredInstall::Preserve
                    } else {
                        match candidate.selected_tab_id() {
                            Some(tab) => DesiredInstall::Fresh(tab.clone()),
                            None => DesiredInstall::Empty,
                        }
                    };
                    let desired = transaction.prepare_desired_install(install)?;
                    let canonical = state.prepare_rename_membership_install(
                        operation.request_id(),
                        candidate,
                        desired.desired().cloned(),
                        deadlines,
                    )?;
                    let decision = canonical.commit();
                    desired.commit();
                    match decision {
                        MembershipInstallDecision::Preserved => RefreshDecision::Continue,
                        MembershipInstallDecision::Attach(target) => {
                            RefreshDecision::AttachRecovered(target)
                        }
                        MembershipInstallDecision::SelectedWithoutAttachment
                        | MembershipInstallDecision::Empty => {
                            RefreshDecision::SettledWithoutAttachment
                        }
                    }
                }
            } else {
                state.settle_rename_without_membership(&operation, result)?;
                RefreshDecision::Continue
            };
            Ok(UiReply::Rename(RenameSettlement::new(
                operation.request_id().clone(),
                decision,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => {
                if changed_selection {
                    self.clear_projection();
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}
