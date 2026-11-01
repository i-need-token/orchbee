//! Owns one identity-bound rename draft, bounded edits, and draw acknowledgement before
//! producing an admissible rename intent. It does not persist a label or let a hidden or stale
//! draft authorize submission.

use super::field_window::{FieldWindow, field_window};
use super::line_editor::{LengthLimit, LineEditor};
use super::{AttachmentKey, ClientContextTabs, ClientPhase, PaneKey, WorkbenchClientState};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use orchbee_workbench_protocol::{MAXIMUM_LABEL_LENGTH, WorkbenchLabel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DraftToken(u64);

#[derive(Default)]
pub(super) struct RenameDraftState {
    last_token: u64,
    draft: Option<RenameDraft>,
}

struct RenameDraft {
    token: DraftToken,
    baseline: ClientContextTabs,
    target: AttachmentKey,
    target_index: usize,
    editor: LineEditor,
    presentation: PresentationPhase,
    shown: bool,
    input_fault: bool,
    /// Whether an edit or a refused submission has exposed the field's error.
    touched: bool,
    complete: bool,
}

/// The typed edit vocabulary the rename draft applies to its line editor —
/// the same contract U5's typed-input bridge delivers for text fields.
pub(crate) enum RenameDraftEdit<'a> {
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

pub(crate) struct RenameDraftView<'a> {
    draft: &'a RenameDraft,
}

impl<'a> RenameDraftView<'a> {
    pub(crate) fn tab_id(&self) -> &'a orchbee_workbench_protocol::WorkbenchTabId {
        self.draft.target.tab_id()
    }
    pub(crate) const fn token(&self) -> DraftToken {
        self.draft.token
    }
    /// The draft's exact raw text — what a submit sends and a cancel discards.
    pub(crate) fn text(&self) -> &'a str {
        self.draft.editor.text()
    }
    /// Pure display-space projection of the draft inside a `field_width`-cell
    /// field; `None` when the field has no drawable width.
    pub(crate) fn field_window(&self, field_width: usize) -> Option<FieldWindow<'a>> {
        field_window(&self.draft.editor, field_width)
    }
    pub(crate) fn target_label(&self) -> std::borrow::Cow<'a, str> {
        self.draft.baseline.tabs()[self.draft.target_index].display_label(self.draft.target_index)
    }
    pub(crate) fn automatic_label(&self) -> bool {
        self.draft.baseline.tabs()[self.draft.target_index].automatic_label()
    }
    pub(crate) fn position(&self) -> usize {
        self.draft.target_index + 1
    }
    pub(crate) fn label_valid(&self) -> bool {
        WorkbenchLabel::parse(self.draft.editor.text()).is_ok()
    }
    pub(crate) const fn input_fault(&self) -> bool {
        self.draft.input_fault
    }
    pub(crate) const fn touched(&self) -> bool {
        self.draft.touched
    }
    /// The field's current error, exposed or not; a rejected insertion reports the limit.
    pub(crate) fn field_error(&self) -> Option<crate::localization::FieldError> {
        use crate::localization::FieldError;
        if self.draft.input_fault {
            Some(FieldError::NameTooLong)
        } else if self.label_valid() {
            None
        } else if self.draft.editor.text().trim().is_empty() {
            Some(FieldError::NameRequired)
        } else {
            Some(FieldError::NameTooLong)
        }
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.draft.shown && self.draft.presentation.interactive()
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.draft.presentation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenameDraftIntent {
    token: DraftToken,
    baseline: ClientContextTabs,
    target: AttachmentKey,
    label: WorkbenchLabel,
}

impl RenameDraftIntent {
    pub(crate) const fn token(&self) -> DraftToken {
        self.token
    }
    pub(crate) const fn baseline(&self) -> &ClientContextTabs {
        &self.baseline
    }
    pub(crate) const fn target(&self) -> &AttachmentKey {
        &self.target
    }
    pub(crate) const fn label(&self) -> &WorkbenchLabel {
        &self.label
    }
}

impl WorkbenchClientState {
    fn rename_authority_available(&self) -> bool {
        self.pending_refresh.is_none()
            && !self.mutation_blocks_structure()
            && self.pending_membership.is_none()
            && self.close.confirmation().is_none()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
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

    pub(crate) fn rename_draft(&self) -> Option<RenameDraftView<'_>> {
        self.current_rename_draft()
            .map(|draft| RenameDraftView { draft })
    }

    pub(crate) fn rename_draft_blocks_structure(&self) -> bool {
        self.rename.draft.is_some()
    }

    pub(crate) fn open_rename_draft(&mut self) -> Option<DraftToken> {
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
        let token = DraftToken(next);
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
            touched: false,
            complete: true,
        });
        self.rename.last_token = next;
        Some(token)
    }

    pub(crate) fn cancel_rename_draft(&mut self) {
        self.rename.draft = None;
    }

    /// Refuses a new draft whose first draw is geometry-blocked, or suspends an
    /// admitted draft while preserving its editor, cursor, and target.
    pub(crate) fn settle_rename_draft_not_presented(&mut self) -> Option<GeometrySettlement> {
        let draft = self.rename.draft.as_mut()?;
        draft.shown = false;
        let settlement = draft.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.rename.draft = None;
            self.set_notice(super::ClientNotice::DialogRefused);
        }
        Some(settlement)
    }

    pub(crate) fn acknowledge_rename_draw(&mut self, token: Option<DraftToken>) {
        if self.current_rename_draft().is_none() {
            self.cancel_rename_draft();
        } else if let Some(draft) = self.rename.draft.as_mut() {
            draft.shown = token == Some(draft.token);
            if draft.shown {
                draft.presentation.presented();
            }
        }
    }

    /// A submission refused for an invalid name exposes its error. Returns whether it was invalid.
    pub(crate) fn reveal_rename_errors(&mut self, token: DraftToken) -> bool {
        if self
            .rename_draft()
            .is_none_or(|draft| draft.token() != token || draft.field_error().is_none())
        {
            return false;
        }
        if let Some(draft) = self.rename.draft.as_mut() {
            draft.touched = true;
        }
        true
    }

    pub(crate) fn rename_draft_input_ready(&self, token: DraftToken) -> bool {
        self.current_rename_draft().is_some_and(|draft| {
            draft.token == token && draft.shown && draft.presentation.interactive()
        })
    }

    pub(crate) fn edit_rename_draft(
        &mut self,
        token: DraftToken,
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
        if matches!(
            edit,
            RenameDraftEdit::Append(_)
                | RenameDraftEdit::Paste(_)
                | RenameDraftEdit::Backspace
                | RenameDraftEdit::Delete
                | RenameDraftEdit::WordBackspace
                | RenameDraftEdit::Clear
        ) && draft.complete
        {
            draft.touched = true;
        }
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

    pub(crate) fn plan_rename_draft(&self, token: DraftToken) -> Option<RenameDraftIntent> {
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

impl<'a> super::WorkbenchClientView<'a> {
    pub(crate) fn rename_draft(&self) -> Option<RenameDraftView<'a>> {
        self.state.rename_draft()
    }
}

/// The rename draft token that leaves the Workbench Client for paint and input dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenameDraftToken(pub(crate) DraftToken);
