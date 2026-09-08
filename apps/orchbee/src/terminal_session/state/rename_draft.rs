use super::rename_text::RenameText;
use super::{AttachmentKey, NativeSessionState, NativeStandaloneTabs, SessionPhase};
use orchbee_workbench_protocol::WorkbenchLabel;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenameDraftToken(u64);

#[derive(Default)]
pub(super) struct RenameDraftState {
    last_token: u64,
    draft: Option<RenameDraft>,
}

struct RenameDraft {
    token: RenameDraftToken,
    baseline: NativeStandaloneTabs,
    target: AttachmentKey,
    target_index: usize,
    text: RenameText,
    shown: bool,
    input_fault: bool,
    complete: bool,
}

pub(in crate::terminal_session) enum RenameDraftEdit<'a> {
    Append(&'a str),
    Backspace,
    Clear,
    Fault,
    Completeness(bool),
}

pub(in crate::terminal_session) struct RenameDraftView<'a> {
    draft: &'a RenameDraft,
}

impl<'a> RenameDraftView<'a> {
    pub(in crate::terminal_session) const fn token(&self) -> RenameDraftToken {
        self.draft.token
    }
    pub(in crate::terminal_session) fn text(&self) -> &'a str {
        self.draft.text.as_str()
    }
    pub(in crate::terminal_session) fn target_label(&self) -> &'a str {
        self.draft.baseline.tabs()[self.draft.target_index]
            .tab_label()
            .as_str()
    }
    pub(in crate::terminal_session) fn label_valid(&self) -> bool {
        self.draft.text.label().is_ok()
    }
    pub(in crate::terminal_session) const fn input_fault(&self) -> bool {
        self.draft.input_fault
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameDraftIntent {
    token: RenameDraftToken,
    baseline: NativeStandaloneTabs,
    target: AttachmentKey,
    label: WorkbenchLabel,
}

impl RenameDraftIntent {
    pub(in crate::terminal_session) const fn token(&self) -> RenameDraftToken {
        self.token
    }
    pub(in crate::terminal_session) const fn baseline(&self) -> &NativeStandaloneTabs {
        &self.baseline
    }
    pub(in crate::terminal_session) const fn target(&self) -> &AttachmentKey {
        &self.target
    }
    pub(in crate::terminal_session) const fn label(&self) -> &WorkbenchLabel {
        &self.label
    }
}

impl NativeSessionState {
    fn rename_authority_available(&self) -> bool {
        self.pending_refresh_generation.is_none()
            && !self.mutation_blocks_structure()
            && self.pending_membership.is_none()
            && self.close.confirmation().is_none()
            && matches!(
                self.phase,
                SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }

    fn current_rename_draft(&self) -> Option<&RenameDraft> {
        let draft = self.rename.draft.as_ref()?;
        (self.rename_authority_available()
            && self.cache == draft.baseline
            && self.desired.as_ref() == Some(&draft.target)
            && self
                .active()
                .is_some_and(|selected| selected.key() == &draft.target))
        .then_some(draft)
    }

    pub(in crate::terminal_session) fn rename_draft(&self) -> Option<RenameDraftView<'_>> {
        self.current_rename_draft()
            .map(|draft| RenameDraftView { draft })
    }

    pub(in crate::terminal_session) fn rename_draft_blocks_structure(&self) -> bool {
        self.rename.draft.is_some()
    }

    pub(in crate::terminal_session) fn open_rename_draft(&mut self) -> Option<RenameDraftToken> {
        if self.rename.draft.is_some()
            || !self.rename_authority_available()
            || !self.invariant_holds()
        {
            return None;
        }
        let target = self.desired.clone()?;
        let target_index = self.active()?.index();
        let tab = self.cache.tabs().get(target_index)?;
        if tab.tab_id() != target.tab_id() {
            return None;
        }
        let next = self.rename.last_token.checked_add(1)?;
        let token = RenameDraftToken(next);
        self.rename.draft = Some(RenameDraft {
            token,
            baseline: self.cache.clone(),
            target,
            target_index,
            text: RenameText::from_label(tab.tab_label()),
            shown: false,
            input_fault: false,
            complete: true,
        });
        self.rename.last_token = next;
        Some(token)
    }

    pub(in crate::terminal_session) fn cancel_rename_draft(&mut self) {
        self.rename.draft = None;
    }

    pub(in crate::terminal_session) fn acknowledge_rename_draw(
        &mut self,
        token: Option<RenameDraftToken>,
    ) {
        if self.current_rename_draft().is_none() {
            self.cancel_rename_draft();
        } else if let Some(draft) = self.rename.draft.as_mut() {
            draft.shown = token == Some(draft.token);
        }
    }

    pub(in crate::terminal_session) fn edit_rename_draft(
        &mut self,
        token: RenameDraftToken,
        edit: RenameDraftEdit<'_>,
    ) -> bool {
        if self.current_rename_draft().is_none() {
            self.cancel_rename_draft();
            return false;
        }
        let Some(draft) = self
            .rename
            .draft
            .as_mut()
            .filter(|draft| draft.token == token)
        else {
            return false;
        };
        match edit {
            RenameDraftEdit::Append(text) if draft.complete => {
                if draft.text.append(text).is_err() {
                    draft.input_fault = true;
                }
            }
            RenameDraftEdit::Backspace if draft.complete => {
                draft.text.remove_last_scalar();
                draft.input_fault = false;
            }
            RenameDraftEdit::Clear if draft.complete => {
                draft.text.clear();
                draft.input_fault = false;
            }
            RenameDraftEdit::Fault => draft.input_fault = true,
            RenameDraftEdit::Completeness(complete) => draft.complete = complete,
            _ => return false,
        }
        true
    }

    pub(in crate::terminal_session) fn plan_rename_draft(
        &self,
        token: RenameDraftToken,
    ) -> Option<RenameDraftIntent> {
        let draft = self.current_rename_draft()?;
        if draft.token != token
            || !draft.shown
            || draft.input_fault
            || !draft.complete
            || !self.invariant_holds()
        {
            return None;
        }
        Some(RenameDraftIntent {
            token,
            baseline: draft.baseline.clone(),
            target: draft.target.clone(),
            label: draft.text.label().ok()?,
        })
    }
}

impl<'a> super::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn rename_draft(&self) -> Option<RenameDraftView<'a>> {
        self.state.rename_draft()
    }
}
