use super::field_window::{NativeFieldWindow, field_window};
use super::line_editor::{LengthLimit, LineEditor};
use super::{AttachmentKey, NativeContextTabs, NativeSessionState, PaneKey, SessionPhase};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use orchbee_workbench_protocol::{MAXIMUM_LABEL_LENGTH, WorkbenchLabel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RenameDraftToken(u64);

#[derive(Default)]
pub(super) struct RenameDraftState {
    last_token: u64,
    draft: Option<RenameDraft>,
}

struct RenameDraft {
    token: RenameDraftToken,
    baseline: NativeContextTabs,
    target: AttachmentKey,
    target_index: usize,
    editor: LineEditor,
    presentation: PresentationPhase,
    shown: bool,
    input_fault: bool,
    complete: bool,
}

/// The typed edit vocabulary the rename draft applies to its line editor —
/// the same contract U5's typed-input bridge delivers for text fields.
pub(in crate::terminal_session) enum RenameDraftEdit<'a> {
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

pub(in crate::terminal_session) struct RenameDraftView<'a> {
    draft: &'a RenameDraft,
}

impl<'a> RenameDraftView<'a> {
    pub(in crate::terminal_session) const fn token(&self) -> RenameDraftToken {
        self.draft.token
    }
    /// The draft's exact raw text — what a submit sends and a cancel discards.
    pub(in crate::terminal_session) fn text(&self) -> &'a str {
        self.draft.editor.text()
    }
    /// Pure display-space projection of the draft inside a `field_width`-cell
    /// field; `None` when the field has no drawable width.
    pub(in crate::terminal_session) fn field_window(
        &self,
        field_width: usize,
    ) -> Option<NativeFieldWindow<'a>> {
        field_window(&self.draft.editor, field_width)
    }
    pub(in crate::terminal_session) fn target_label(&self) -> std::borrow::Cow<'a, str> {
        self.draft.baseline.tabs()[self.draft.target_index].display_label(self.draft.target_index)
    }
    pub(in crate::terminal_session) fn automatic_label(&self) -> bool {
        self.draft.baseline.tabs()[self.draft.target_index].automatic_label()
    }
    pub(in crate::terminal_session) fn position(&self) -> usize {
        self.draft.target_index + 1
    }
    pub(in crate::terminal_session) fn label_valid(&self) -> bool {
        WorkbenchLabel::parse(self.draft.editor.text()).is_ok()
    }
    pub(in crate::terminal_session) const fn input_fault(&self) -> bool {
        self.draft.input_fault
    }
    pub(in crate::terminal_session) const fn interactive(&self) -> bool {
        self.draft.shown && self.draft.presentation.interactive()
    }
    pub(in crate::terminal_session) const fn presentation(&self) -> PresentationPhase {
        self.draft.presentation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameDraftIntent {
    token: RenameDraftToken,
    baseline: NativeContextTabs,
    target: AttachmentKey,
    label: WorkbenchLabel,
}

impl RenameDraftIntent {
    pub(in crate::terminal_session) const fn token(&self) -> RenameDraftToken {
        self.token
    }
    pub(in crate::terminal_session) const fn baseline(&self) -> &NativeContextTabs {
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
        self.pending_refresh.is_none()
            && !self.mutation_blocks_structure()
            && self.pending_membership.is_none()
            && self.close.confirmation().is_none()
            && self.panel_close.confirmation().is_none()
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
            && self.desired.as_ref().map(PaneKey::anchor) == Some(&draft.target)
            && self
                .active()
                .is_some_and(|selected| selected.key().anchor() == &draft.target))
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
        let target = self.desired.as_ref()?.anchor().clone();
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
            // Automatic positions are presentation-only, so keep the editor
            // empty and let the modal show the localized display name as a
            // placeholder. Fixed labels remain exact and start at the end.
            editor: LineEditor::new(
                if tab.automatic_label() {
                    String::new()
                } else {
                    tab.display_label(target_index).into_owned()
                },
                LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH),
            ),
            presentation: PresentationPhase::Pending,
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

    /// Refuses a new draft whose first draw is geometry-blocked, or suspends an
    /// admitted draft while preserving its editor, cursor, and target.
    pub(in crate::terminal_session) fn settle_rename_draft_not_presented(
        &mut self,
    ) -> Option<GeometrySettlement> {
        let draft = self.rename.draft.as_mut()?;
        draft.shown = false;
        let settlement = draft.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.rename.draft = None;
        }
        Some(settlement)
    }

    pub(in crate::terminal_session) fn acknowledge_rename_draw(
        &mut self,
        token: Option<RenameDraftToken>,
    ) {
        if self.current_rename_draft().is_none() {
            self.cancel_rename_draft();
        } else if let Some(draft) = self.rename.draft.as_mut() {
            draft.shown = token == Some(draft.token);
            if draft.shown {
                draft.presentation.presented();
            }
        }
    }

    pub(in crate::terminal_session) fn rename_draft_input_ready(
        &self,
        token: RenameDraftToken,
    ) -> bool {
        self.current_rename_draft().is_some_and(|draft| {
            draft.token == token && draft.shown && draft.presentation.interactive()
        })
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
            // Typed input and paste share the editor's one atomic insert path;
            // a rejected insertion faults the draft without touching its text.
            RenameDraftEdit::Append(text) | RenameDraftEdit::Paste(text) if draft.complete => {
                if draft.editor.insert(text).is_err() {
                    draft.input_fault = true;
                }
            }
            RenameDraftEdit::Backspace if draft.complete => {
                draft.editor.backspace();
                draft.input_fault = false;
            }
            RenameDraftEdit::Delete if draft.complete => {
                draft.editor.delete();
                draft.input_fault = false;
            }
            RenameDraftEdit::WordBackspace if draft.complete => {
                draft.editor.word_back();
                draft.input_fault = false;
            }
            RenameDraftEdit::Clear if draft.complete => {
                draft.editor.clear();
                draft.input_fault = false;
            }
            RenameDraftEdit::Left if draft.complete => {
                draft.editor.left();
            }
            RenameDraftEdit::Right if draft.complete => {
                draft.editor.right();
            }
            RenameDraftEdit::Home if draft.complete => {
                draft.editor.home();
            }
            RenameDraftEdit::End if draft.complete => {
                draft.editor.end();
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
            label: WorkbenchLabel::parse(draft.editor.text()).ok()?,
        })
    }
}

impl<'a> super::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn rename_draft(&self) -> Option<RenameDraftView<'a>> {
        self.state.rename_draft()
    }
}
