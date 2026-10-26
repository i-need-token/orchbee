#[cfg(test)]
use super::WorkbenchClientState;
use super::{
    Arc, CarrierFailure, ClientActionDisposition, CommandMailbox, ModalPresentation,
    MoveAdmissionError, MutationResult, OperationDeadlines, RefreshDecision, UiReply,
    WorkbenchClientUi, WorkbenchClientView,
};
use crate::workbench_client::state::FieldWindow;
use crate::workbench_client::state::rename_draft::{
    RenameDraftEdit, RenameDraftToken, RenameDraftView,
};
use orchbee_workbench_protocol::MAXIMUM_LABEL_LENGTH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenameDraftToken(RenameDraftToken);

pub(crate) struct RenameDraftView<'a>(RenameDraftView<'a>, Option<&'a str>);
impl<'a> RenameDraftView<'a> {
    pub(crate) fn tab_id(&self) -> &'a orchbee_workbench_protocol::WorkbenchTabId {
        self.0.tab_id()
    }
    pub(crate) fn token(&self) -> RenameDraftToken {
        RenameDraftToken(self.0.token())
    }
    pub(crate) fn text(&self) -> &'a str {
        self.0.text()
    }
    pub(crate) fn target_label(
        &self,
        locale: crate::localization::Locale,
    ) -> std::borrow::Cow<'a, str> {
        if self.0.automatic_label() {
            self.1.map_or_else(
                || locale.automatic_tab_label(self.0.position()).into(),
                std::borrow::Cow::Borrowed,
            )
        } else {
            self.0.target_label()
        }
    }
    pub(crate) fn automatic_label(&self) -> bool {
        self.0.automatic_label()
    }
    /// Pure display-space projection of the draft inside a
    /// `field_width`-cell field; `None` when the field has no drawable
    /// width.
    pub(crate) fn field_window(&self, field_width: usize) -> Option<FieldWindow<'a>> {
        self.0.field_window(field_width)
    }
    pub(crate) fn input_fault(&self) -> bool {
        self.0.input_fault()
    }
    pub(crate) fn label_valid(&self) -> bool {
        self.0.label_valid()
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.0.interactive()
    }
    pub(crate) const fn presentation(&self) -> crate::tui_presentation::PresentationPhase {
        self.0.presentation()
    }
    pub(crate) fn remaining_scalar_capacity(&self) -> usize {
        MAXIMUM_LABEL_LENGTH - self.0.text().chars().count()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenameInputContext {
    token: RenameDraftToken,
}
impl RenameInputContext {
    #[cfg(test)]
    pub(crate) const fn token(self) -> RenameDraftToken {
        self.token
    }
}

/// The typed edit vocabulary forwarded to the rename draft — the same
/// contract U5's typed-input bridge emits for text fields.
pub(crate) enum RenameEdit<'a> {
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

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn rename_draft(&self) -> Option<RenameDraftView<'a>> {
        self.state.as_ref()?.rename_draft().map(|draft| {
            let title = self
                .tab(draft.position() - 1)
                .and_then(|tab| tab.observed_title);
            RenameDraftView(draft, title)
        })
    }
}

impl WorkbenchClientUi {
    #[cfg(test)]
    pub(in crate::workbench_client) fn rename_receipts_for_test(
        &self,
    ) -> (
        Option<&crate::workbench_client::state::tab_rename::RenameOperation>,
        Option<&crate::workbench_client::state::tab_rename::PendingRenameCommit>,
        Option<&crate::workbench_client::state::tab_rename::RenameOperation>,
    ) {
        self.state.as_ref().map_or(
            (None, None, None),
            WorkbenchClientState::rename_receipts_for_test,
        )
    }
    pub(in crate::workbench_client) fn rename_worker_loss(
        &self,
    ) -> Option<crate::workbench_client::state::tab_rename::RenameWorkerEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.rename_worker_loss())
    }
    pub(in crate::workbench_client) fn finalize_rename_worker(
        &mut self,
        evidence: Option<crate::workbench_client::state::tab_rename::RenameWorkerEvidence>,
    ) -> bool {
        evidence.is_none_or(|evidence| {
            self.state
                .as_mut()
                .is_some_and(|state| state.retain_rename_worker(evidence))
        })
    }
    pub(in crate::workbench_client) fn request_rename_draft(&mut self) -> Option<RenameDraftToken> {
        if self.retirement_failure.is_some() {
            return None;
        }
        let state = self.state.as_mut()?;
        self.mailbox
            .as_ref()?
            .open_rename_draft_for(state)
            .map(RenameDraftToken)
    }
    pub(in crate::workbench_client) fn rename_input_context(
        &mut self,
    ) -> Option<RenameInputContext> {
        let state = self.state.as_mut()?;
        if state.rename_draft().is_none() {
            state.cancel_rename_draft();
            return None;
        }
        let draft = state.rename_draft()?;
        Some(RenameInputContext {
            token: RenameDraftToken(draft.token()),
        })
    }
    pub(in crate::workbench_client) fn edit_rename_draft(
        &mut self,
        token: RenameDraftToken,
        edit: RenameEdit<'_>,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        if !state.rename_draft_input_ready(token.0) {
            return false;
        }
        state.edit_rename_draft(
            token.0,
            match edit {
                RenameEdit::Append(text) => RenameDraftEdit::Append(text),
                RenameEdit::Paste(text) => RenameDraftEdit::Paste(text),
                RenameEdit::Backspace => RenameDraftEdit::Backspace,
                RenameEdit::Clear => RenameDraftEdit::Clear,
                RenameEdit::Left => RenameDraftEdit::Left,
                RenameEdit::Right => RenameDraftEdit::Right,
                RenameEdit::Home => RenameDraftEdit::Home,
                RenameEdit::End => RenameDraftEdit::End,
                RenameEdit::Delete => RenameDraftEdit::Delete,
                RenameEdit::WordBackspace => RenameDraftEdit::WordBackspace,
                RenameEdit::Fault => RenameDraftEdit::Fault,
                RenameEdit::Completeness(value) => RenameDraftEdit::Completeness(value),
            },
        )
    }
    pub(in crate::workbench_client) fn cancel_rename_draft(&mut self, token: RenameDraftToken) {
        if let Some(state) = self.state.as_mut()
            && state
                .rename_draft()
                .is_some_and(|draft| draft.token() == token.0)
        {
            state.cancel_rename_draft();
        }
    }
    pub(in crate::workbench_client) fn after_rename_draw(
        &mut self,
        token: Option<ModalPresentation<RenameDraftToken>>,
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
                let _ = state.settle_rename_draft_not_presented();
            }
            None => state.acknowledge_rename_draw(None),
        }
    }
    pub(in crate::workbench_client) fn invalidate_rename_display(&mut self) {
        self.after_rename_draw(None);
    }
    pub(in crate::workbench_client) fn submit_rename_draft(
        &mut self,
        token: RenameDraftToken,
    ) -> ClientActionDisposition {
        if self
            .state
            .as_ref()
            .is_none_or(|state| !state.rename_draft_input_ready(token.0))
        {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(deadline) = self
            .clock
            .now()
            .checked_add(super::super::state::OPERATION_TOTAL)
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        match mailbox.try_enqueue_rename_for(state, token.0, deadline, &self.clock) {
            Ok(_) => ClientActionDisposition::DismissNavigator,
            Err(MoveAdmissionError::Closed(closed)) => {
                self.retire(closed.failure());
                ClientActionDisposition::KeepNavigator
            }
            Err(_) => ClientActionDisposition::KeepNavigator,
        }
    }
}
use crate::workbench_client::{
    mailbox::{DesiredInstall, RenameSettlement},
    state::{
        MembershipInstallDecision,
        tab_rename::{RenameOperation, RenamePreparation, RenameResult},
    },
};

impl WorkbenchClientUi {
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
                if !preserve && state.has_attachments() {
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
                            Some(tab) => match candidate.focus_panel_id() {
                                Some(panel) => DesiredInstall::Fresh {
                                    tab_id: tab.clone(),
                                    panel: panel.clone(),
                                },
                                None => DesiredInstall::Empty,
                            },
                            None => DesiredInstall::Empty,
                        }
                    };
                    let desired = transaction.prepare_desired_install(install)?;
                    let canonical = state.prepare_rename_membership_install(
                        operation.request_id(),
                        candidate,
                        desired.desired().map(|key| key.anchor().clone()),
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
                    self.clear_projections();
                }
                self.validate_state()
            }
            Ok(MutationResult::Stale) => false,
            Err(error) => self.handle_event_error(error),
        }
    }
}
