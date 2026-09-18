use super::super::field_window::{NativeFieldWindow, field_window};
use super::super::line_editor::{LengthLimit, LineEditor};
use super::{
    NativeContextLocation, NativeContextTabs, NativeSessionState, SessionNotice, SessionPhase,
    WorkbenchLabel, WorkbenchProjectId,
};
use orchbee_workbench_protocol::{MAXIMUM_DIRECTORY_PATH_BYTES, MAXIMUM_LABEL_LENGTH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSetupToken(pub(super) u64);
impl NativeSetupToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSetupStage {
    ChooseProject,
    ProjectLabel,
    WorkspaceLabel,
    DirectoryPath,
    ProjectRename,
    WorkspaceRename,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSetupStatus {
    LocationLabel {
        target: NativeLocationLabelTarget,
        status: NativeLocationLabelStatus,
    },
    NotApplied,
    ProjectCreated,
    WorkspaceCreated,
    DirectoryAccepted,
    CommittedNeedRefresh,
    OutcomeUnknown,
}
/// The typed edit vocabulary the setup draft applies — the same contract
/// U5's typed-input bridge delivers. Text operations apply to the stage's
/// line editor and are inert while the picker owns the draft.
pub(crate) enum NativeSetupEdit<'a> {
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
    SelectPreviousProject,
    SelectNextProject,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeLocationLabelTarget {
    Project,
    Workspace,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeLocationLabelStatus {
    Unavailable,
    NotApplied,
    Unchanged,
    Renamed,
    CommittedNeedRefresh,
    OutcomeUnknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum NativeSetupIntent {
    CreateWorkspace,
    ConfigureDirectory,
    RenameLocation(NativeLocationLabelTarget),
}
#[derive(Clone, Debug)]
pub(super) struct SetupDraft {
    pub(super) token: NativeSetupToken,
    pub(super) baseline: NativeContextTabs,
    pub(super) stage: NativeSetupStage,
    pub(super) selected: usize,
    pub(super) project: Option<WorkbenchProjectId>,
    pub(super) known_project_created: bool,
    pub(super) editor: LineEditor,
    pub(super) shown: bool,
    pub(super) complete: bool,
    pub(super) faulted: bool,
}
pub(crate) struct NativeSetupDraftView<'a> {
    pub(super) draft: &'a SetupDraft,
}
pub(crate) struct NativeSetupProjectView<'a> {
    pub(super) position: usize,
    pub(super) selected: bool,
    pub(super) label: Option<&'a str>,
}
impl<'a> NativeSetupProjectView<'a> {
    pub(crate) fn position(&self) -> usize {
        self.position
    }
    pub(crate) fn selected(&self) -> bool {
        self.selected
    }
    pub(crate) fn label(&self) -> Option<&'a str> {
        self.label
    }
}
impl<'a> NativeSetupDraftView<'a> {
    pub(crate) fn token(&self) -> NativeSetupToken {
        self.draft.token
    }
    pub(crate) fn stage(&self) -> NativeSetupStage {
        self.draft.stage
    }
    /// The draft's exact raw text — what a submit sends and a cancel
    /// discards.
    pub(crate) fn text(&self) -> &'a str {
        self.draft.editor.text()
    }
    /// Pure display-space projection of the draft inside a
    /// `field_width`-cell field; `None` when the field has no drawable
    /// width.
    pub(crate) fn field_window(&self, field_width: usize) -> Option<NativeFieldWindow<'a>> {
        field_window(&self.draft.editor, field_width)
    }
    pub(crate) fn faulted(&self) -> bool {
        self.draft.faulted
    }
    pub(crate) fn known_project_created(&self) -> bool {
        self.draft.known_project_created
    }
    pub(crate) fn project_count(&self) -> usize {
        self.draft.baseline.projects().len() + 1
    }
    pub(crate) fn project_at(&self, index: usize) -> Option<NativeSetupProjectView<'a>> {
        if index >= self.project_count() {
            return None;
        }
        Some(NativeSetupProjectView {
            position: index + 1,
            selected: index == self.draft.selected,
            label: self
                .draft
                .baseline
                .projects()
                .get(index)
                .map(|p| p.label.as_str()),
        })
    }
    pub(crate) fn project_label(&self) -> Option<&'a str> {
        let id = self.draft.project.as_ref()?;
        self.draft
            .baseline
            .projects()
            .iter()
            .find(|p| &p.project_id == id)
            .map(|p| p.label.as_str())
    }
    pub(crate) fn workspace_label(&self) -> Option<&'a str> {
        self.draft
            .baseline
            .navigation()
            .iter()
            .find(|c| &c.location == self.draft.baseline.location())?
            .workspace_label
            .as_ref()
            .map(WorkbenchLabel::as_str)
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn open_setup_draft(
        &mut self,
        intent: NativeSetupIntent,
    ) -> Option<NativeSetupToken> {
        if self.setup_blocks_structure()
            || !self.setup_authority_available()
            || !self.invariant_holds()
        {
            return None;
        }
        if matches!(intent, NativeSetupIntent::RenameLocation(_)) {
            self.clear_notice();
        }
        if matches!(intent, NativeSetupIntent::ConfigureDirectory)
            && (!matches!(self.phase, SessionPhase::Empty)
                || !self.workspace_directory_unconfigured())
        {
            return None;
        }
        let project = match self.cache.location() {
            NativeContextLocation::Workspace { project_id, .. } => Some(project_id.clone()),
            _ => None,
        };
        let (stage, editor) = match intent {
            NativeSetupIntent::CreateWorkspace => (
                NativeSetupStage::ChooseProject,
                LineEditor::new(String::new(), LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH)),
            ),
            NativeSetupIntent::ConfigureDirectory => (
                NativeSetupStage::DirectoryPath,
                LineEditor::new(
                    String::new(),
                    LengthLimit::Bytes(MAXIMUM_DIRECTORY_PATH_BYTES),
                ),
            ),
            NativeSetupIntent::RenameLocation(target) => {
                let Some(label) = super::labels::location_label(&self.cache, target) else {
                    self.setup.status = Some(NativeSetupStatus::LocationLabel {
                        target,
                        status: NativeLocationLabelStatus::Unavailable,
                    });
                    return None;
                };
                (
                    match target {
                        NativeLocationLabelTarget::Project => NativeSetupStage::ProjectRename,
                        NativeLocationLabelTarget::Workspace => NativeSetupStage::WorkspaceRename,
                    },
                    // Prefill keeps the protocol-legal label byte-exact with
                    // the cursor at its end.
                    LineEditor::new(label.to_owned(), LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH)),
                )
            }
        };
        let selected = project
            .as_ref()
            .and_then(|id| {
                self.cache
                    .projects()
                    .iter()
                    .position(|p| &p.project_id == id)
            })
            .unwrap_or(self.cache.projects().len());
        let token = NativeSetupToken(self.setup.last_token.checked_add(1)?);
        self.setup.last_token = token.0;
        self.setup.status = None;
        self.setup.draft = Some(SetupDraft {
            token,
            baseline: self.cache.clone(),
            stage,
            selected,
            project,
            known_project_created: false,
            editor,
            shown: false,
            complete: true,
            faulted: false,
        });
        Some(token)
    }
    pub(in crate::terminal_session) fn setup_draft(&self) -> Option<NativeSetupDraftView<'_>> {
        let draft = self.setup.draft.as_ref()?;
        (self.setup_authority_available() && draft.baseline == self.cache)
            .then_some(NativeSetupDraftView { draft })
    }
    pub(in crate::terminal_session) fn clear_settled_setup_status(&mut self) {
        if !self.setup_blocks_structure() {
            self.setup.status = None;
        }
    }
    pub(in crate::terminal_session) fn setup_status(&self) -> Option<NativeSetupStatus> {
        self.setup.status
    }
    pub(in crate::terminal_session) fn acknowledge_setup_draw(
        &mut self,
        token: Option<NativeSetupToken>,
    ) {
        if let Some(draft) = self.setup.draft.as_mut() {
            draft.shown = token == Some(draft.token);
        }
    }
    pub(in crate::terminal_session) fn cancel_setup_on_retirement(&mut self) {
        self.setup.draft = None;
    }
    /// A draw receipt reporting that the draft could not be presented cancels
    /// the capture with a notice instead of leaving it open swallowing keys.
    pub(in crate::terminal_session) fn settle_setup_draft_not_presented(&mut self) {
        if self.setup.draft.is_some() {
            self.setup.draft = None;
            self.set_notice(SessionNotice::DraftCancelled);
        }
    }
    pub(in crate::terminal_session) fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        if self
            .setup
            .draft
            .as_ref()
            .is_some_and(|draft| draft.token == token)
        {
            self.setup.draft = None;
        }
    }
    pub(in crate::terminal_session) fn edit_setup_draft(
        &mut self,
        token: NativeSetupToken,
        edit: NativeSetupEdit<'_>,
    ) -> bool {
        if self.setup_draft().is_none() {
            return false;
        }
        let Some(draft) = self.setup.draft.as_mut().filter(|d| d.token == token) else {
            return false;
        };
        match edit {
            NativeSetupEdit::Completeness(value) => draft.complete = value,
            NativeSetupEdit::Fault => draft.faulted = true,
            NativeSetupEdit::SelectPreviousProject | NativeSetupEdit::SelectNextProject
                if draft.complete && draft.stage == NativeSetupStage::ChooseProject =>
            {
                let Some(next_token) = self.setup.last_token.checked_add(1) else {
                    return false;
                };
                let count = draft.baseline.projects().len() + 1;
                draft.selected = if matches!(edit, NativeSetupEdit::SelectNextProject) {
                    (draft.selected + 1) % count
                } else {
                    (draft.selected + count - 1) % count
                };
                draft.project = draft
                    .baseline
                    .projects()
                    .get(draft.selected)
                    .map(|p| p.project_id.clone());
                draft.token = NativeSetupToken(next_token);
                self.setup.last_token = next_token;
                draft.shown = false;
            }
            NativeSetupEdit::Append(value) | NativeSetupEdit::Paste(value)
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                // Typed input and paste share the editor's one atomic insert
                // path; a rejected insertion faults without touching text.
                if draft.editor.insert(value).is_err() {
                    draft.faulted = true;
                }
            }
            NativeSetupEdit::Backspace
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.backspace();
                draft.faulted = false;
            }
            NativeSetupEdit::Delete
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.delete();
                draft.faulted = false;
            }
            NativeSetupEdit::WordBackspace
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.word_back();
                draft.faulted = false;
            }
            NativeSetupEdit::Clear
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.clear();
                draft.faulted = false;
            }
            NativeSetupEdit::Left
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.left();
            }
            NativeSetupEdit::Right
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.right();
            }
            NativeSetupEdit::Home
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.home();
            }
            NativeSetupEdit::End
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                draft.editor.end();
            }
            _ => return false,
        }
        true
    }
    pub(in crate::terminal_session) fn advance_setup_picker(
        &mut self,
        token: NativeSetupToken,
    ) -> bool {
        if self.setup_draft().is_none() {
            return false;
        }
        let Some(draft) = self.setup.draft.as_mut().filter(|d| {
            d.token == token
                && d.shown
                && d.complete
                && !d.faulted
                && d.stage == NativeSetupStage::ChooseProject
        }) else {
            return false;
        };
        let Some(next) = self.setup.last_token.checked_add(1) else {
            return false;
        };
        draft.project = draft
            .baseline
            .projects()
            .get(draft.selected)
            .map(|p| p.project_id.clone());
        draft.stage = if draft.project.is_some() {
            NativeSetupStage::WorkspaceLabel
        } else {
            NativeSetupStage::ProjectLabel
        };
        draft.token = NativeSetupToken(next);
        self.setup.last_token = next;
        draft.shown = false;
        true
    }
}
impl<'a> crate::terminal_session::state::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn setup_draft(&self) -> Option<NativeSetupDraftView<'a>> {
        self.state.setup_draft()
    }
    pub(in crate::terminal_session) fn setup_status(&self) -> Option<NativeSetupStatus> {
        self.state.setup_status()
    }
    pub(in crate::terminal_session) fn workspace_directory_path(&self) -> Option<&'a str> {
        self.state.workspace_directory_path()
    }
    pub(in crate::terminal_session) fn workspace_directory_unconfigured(&self) -> bool {
        self.state.workspace_directory_unconfigured()
    }
}
