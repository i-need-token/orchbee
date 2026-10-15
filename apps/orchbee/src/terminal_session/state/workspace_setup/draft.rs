use super::super::field_window::{NativeFieldWindow, field_window};
use super::super::line_editor::{LengthLimit, LineEditor};
use super::{NativeContextTabs, NativeSessionState, WorkbenchLabel, WorkbenchProjectId};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
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
    ChooseWorkspaceProject,
    WorkspaceProjectName,
    DissolveProject,
    TabGroupName,
    PreparingWorkspace,
    QuickProject,
    QuickName,
    QuickDirectory,
    DirectoryPath,
    ProjectRename,
    WorkspaceRename,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSetupStatus {
    PresentationUnavailable,
    WorkspaceOrganized,
    ProjectDissolved,
    TabsConfigured,
    NavigationConfigured,
    TabsNotApplied,
    TabsCommittedNeedRefresh,
    TabsOutcomeUnknown,
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
    NextField,
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
    ScrollProjects { up: bool, lines: usize },
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum NativeSetupIntent {
    Organize(Box<super::WorkspaceOrganizationRequest>),
    TabGroupName(Box<orchbee_workbench_protocol::TabConfiguration>),
    ConfigureDirectory,
    RenameLocation(NativeLocationLabelTarget),
    RenameProject(Box<super::ProjectRenameRequest>),
}
#[derive(Clone, Debug)]
pub(super) struct SetupDraft {
    pub(super) organization: Option<Box<super::WorkspaceOrganizationRequest>>,
    pub(super) tab_configuration: Option<orchbee_workbench_protocol::TabConfiguration>,
    pub(super) project_rename: Option<Box<super::ProjectRenameRequest>>,
    pub(super) continuation: Option<super::OperationDeadlines>,
    pub(super) quick: Option<super::quick::QuickWorkspaceFields>,
    pub(super) token: NativeSetupToken,
    pub(super) baseline: NativeContextTabs,
    pub(super) stage: NativeSetupStage,
    pub(super) selected: usize,
    pub(super) project: Option<WorkbenchProjectId>,
    pub(super) editor: LineEditor,
    pub(super) shown: bool,
    pub(super) complete: bool,
    pub(super) faulted: bool,
    pub(super) parent: Option<Box<SetupParent>>,
}
#[derive(Clone, Debug)]
pub(super) struct SetupParent {
    stage: NativeSetupStage,
    selected: usize,
    project: Option<WorkbenchProjectId>,
    editor: LineEditor,
    complete: bool,
    faulted: bool,
    quick: Option<super::quick::QuickWorkspaceFields>,
}
pub(crate) struct NativeSetupDraftView<'a> {
    pub(super) draft: &'a SetupDraft,
    presentation: PresentationPhase,
}
pub(crate) struct NativeSetupProjectView<'a> {
    pub(super) position: usize,
    pub(super) label: Option<&'a str>,
    ungrouped: bool,
}
impl SetupDraft {
    fn text_field(&self) -> bool {
        !matches!(
            self.stage,
            NativeSetupStage::ChooseWorkspaceProject
                | NativeSetupStage::DissolveProject
                | NativeSetupStage::QuickProject
                | NativeSetupStage::PreparingWorkspace
        )
    }
    pub(super) fn editor(&self) -> &LineEditor {
        self.quick.as_ref().map_or(&self.editor, |fields| {
            fields.field(self.stage == NativeSetupStage::QuickDirectory)
        })
    }
    fn editor_mut(&mut self) -> &mut LineEditor {
        match &mut self.quick {
            Some(fields) => fields.field_mut(self.stage == NativeSetupStage::QuickDirectory),
            None => &mut self.editor,
        }
    }
}
impl<'a> NativeSetupProjectView<'a> {
    pub(crate) fn ungrouped(&self) -> bool {
        self.ungrouped
    }
    pub(crate) fn position(&self) -> usize {
        self.position
    }
    pub(crate) fn label(&self) -> Option<&'a str> {
        self.label
    }
}
impl<'a> NativeSetupDraftView<'a> {
    #[cfg(windows)]
    pub(in crate::terminal_session) fn background(&self) -> bool {
        self.draft.stage == NativeSetupStage::PreparingWorkspace
            || self.draft.quick.is_some() && self.draft.continuation.is_some()
    }
    pub(crate) const fn interactive(&self) -> bool {
        self.draft.shown && self.presentation.interactive()
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.presentation
    }
    /// Index within the current token's Project choices, including synthetic rows.
    pub(crate) fn selected_project(&self) -> usize {
        self.draft.selected
    }
    pub(crate) fn quick_window(
        &self,
        directory: bool,
        width: usize,
    ) -> Option<NativeFieldWindow<'a>> {
        field_window(self.draft.quick.as_ref()?.field(directory), width)
    }
    pub(crate) fn token(&self) -> NativeSetupToken {
        self.draft.token
    }
    pub(crate) fn stage(&self) -> NativeSetupStage {
        self.draft.stage
    }
    /// The draft's exact raw text — what a submit sends and a cancel
    /// discards.
    pub(crate) fn text(&self) -> &'a str {
        self.draft.editor().text()
    }
    /// Pure display-space projection of the draft inside a
    /// `field_width`-cell field; `None` when the field has no drawable
    /// width.
    pub(crate) fn field_window(&self, field_width: usize) -> Option<NativeFieldWindow<'a>> {
        field_window(self.draft.editor(), field_width)
    }
    pub(crate) fn faulted(&self) -> bool {
        self.draft.faulted
    }
    pub(crate) fn project_count(&self) -> usize {
        self.draft.project_count()
    }
    pub(crate) fn project_at(&self, index: usize) -> Option<NativeSetupProjectView<'a>> {
        if index >= self.project_count() {
            return None;
        }
        Some(NativeSetupProjectView {
            ungrouped: self.draft.stage == NativeSetupStage::ChooseWorkspaceProject
                && index == self.draft.baseline.projects().len(),
            position: index + 1,
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
        if let Some(request) = &self.draft.organization
            && let super::organization::OrganizationTarget::Workspace(workspace) = &request.target
        {
            return Some(&workspace.label);
        }
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
    /// The caller submits synchronously: the painted row, rather than a newly drawn
    /// selection, authorizes this exact choice. No undrawn token is armed.
    pub(in crate::terminal_session) fn choose_setup_project(
        &mut self,
        token: NativeSetupToken,
        index: usize,
    ) -> bool {
        if self.setup_draft().is_none() || self.quick_continuation().is_some() {
            return false;
        }
        let interactive = self.setup.presentation.interactive();
        let Some(draft) = self.setup.draft.as_mut().filter(|draft| {
            draft.token == token
                && draft.shown
                && interactive
                && draft.complete
                && !draft.faulted
                && matches!(
                    draft.stage,
                    NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
                )
                && index < draft.project_count()
        }) else {
            return false;
        };
        draft.selected = index;
        draft.project = draft
            .baseline
            .projects()
            .get(index)
            .map(|project| project.project_id.clone());
        true
    }

    pub(in crate::terminal_session) fn scroll_setup_projects(
        &mut self,
        token: NativeSetupToken,
        up: bool,
        lines: usize,
    ) -> bool {
        if !self.setup.draft.as_ref().is_some_and(|draft| {
            draft.token == token && draft.shown && self.setup.presentation.interactive()
        }) {
            return false;
        }
        self.edit_setup_draft(token, NativeSetupEdit::ScrollProjects { up, lines })
    }

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
        if matches!(
            intent,
            NativeSetupIntent::RenameLocation(_) | NativeSetupIntent::RenameProject(_)
        ) {
            self.clear_notice();
        }
        if matches!(intent, NativeSetupIntent::ConfigureDirectory)
            && self.directory_configuration_unavailable().is_some()
        {
            return None;
        }
        let mut project = self.cache.project_id().cloned();
        let mut project_rename = None;
        let mut organization = None;
        let mut tab_configuration = None;
        let (stage, editor) = match intent {
            NativeSetupIntent::Organize(request) => {
                if !request.matches(self) {
                    return None;
                }
                let stage = match &request.target {
                    super::organization::OrganizationTarget::Workspace(workspace) => {
                        project = workspace.project_id.clone();
                        NativeSetupStage::ChooseWorkspaceProject
                    }
                    super::organization::OrganizationTarget::Project(target) => {
                        project = Some(target.project_id.clone());
                        NativeSetupStage::DissolveProject
                    }
                };
                organization = Some(request);
                (
                    stage,
                    LineEditor::new(String::new(), LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH)),
                )
            }
            NativeSetupIntent::TabGroupName(operation) => {
                let label = match &*operation {
                    orchbee_workbench_protocol::TabConfiguration::CreateGroup { label, .. }
                    | orchbee_workbench_protocol::TabConfiguration::CreateGroupForTabs {
                        label,
                        ..
                    }
                    | orchbee_workbench_protocol::TabConfiguration::UpdateGroup { label, .. } => {
                        label.as_str().to_owned()
                    }
                    _ => return None,
                };
                tab_configuration = Some(*operation);
                (
                    NativeSetupStage::TabGroupName,
                    LineEditor::new(label, LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH)),
                )
            }
            NativeSetupIntent::RenameProject(request) => {
                if !request.matches(self) {
                    return None;
                }
                project = Some(request.project_id().clone());
                let editor = LineEditor::new(
                    request.label().to_owned(),
                    LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH),
                );
                project_rename = Some(request);
                (NativeSetupStage::ProjectRename, editor)
            }
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
        self.setup.presentation = PresentationPhase::Pending;
        self.setup.draft = Some(SetupDraft {
            organization,
            tab_configuration,
            project_rename,
            token,
            baseline: self.cache.clone(),
            stage,
            selected,
            project,
            editor,
            shown: false,
            complete: true,
            faulted: false,
            quick: None,
            continuation: None,
            parent: None,
        });
        Some(token)
    }
    pub(in crate::terminal_session) fn setup_draft(&self) -> Option<NativeSetupDraftView<'_>> {
        let draft = self.setup.draft.as_ref()?;
        ((self.setup_authority_available() || self.setup.preparation.is_some())
            && draft.baseline == self.cache
            && draft
                .organization
                .as_ref()
                .is_none_or(|request| request.matches(self))
            && draft
                .project_rename
                .as_ref()
                .is_none_or(|request| request.matches(self))
            && draft
                .quick
                .as_ref()
                .is_none_or(|fields| fields.target_current(self)))
        .then_some(NativeSetupDraftView {
            draft,
            presentation: self.setup.presentation,
        })
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
        let presented = self
            .setup
            .draft
            .as_ref()
            .is_some_and(|draft| token == Some(draft.token));
        if let Some(draft) = self.setup.draft.as_mut() {
            draft.shown = presented;
        }
        if presented {
            self.setup.presentation.presented();
        }
    }
    pub(in crate::terminal_session) fn cancel_setup_on_retirement(&mut self) {
        #[cfg(any(windows, test))]
        self.cancel_project_quick_entry();
        self.setup.draft = None;
        self.setup.preparation = None;
    }
    /// Classifies an evaluated geometry block without treating an absent or
    /// covered surface as cancellation.
    pub(in crate::terminal_session) fn settle_setup_draft_not_presented(
        &mut self,
    ) -> Option<GeometrySettlement> {
        let draft = self.setup.draft.as_mut()?;
        draft.shown = false;
        let settlement = self.setup.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.setup.draft = None;
            self.setup.status = Some(NativeSetupStatus::PresentationUnavailable);
        }
        Some(settlement)
    }
    pub(in crate::terminal_session) fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        let Some(draft) = self
            .setup
            .draft
            .as_mut()
            .filter(|draft| draft.token == token)
        else {
            return;
        };
        if let Some(parent) = draft.parent.take()
            && let Some(next) = self.setup.last_token.checked_add(1)
        {
            draft.stage = parent.stage;
            draft.selected = parent.selected;
            draft.project = parent.project;
            draft.editor = parent.editor;
            draft.complete = parent.complete;
            draft.faulted = parent.faulted;
            draft.quick = parent.quick;
            draft.token = NativeSetupToken(next);
            draft.shown = false;
            self.setup.last_token = next;
        } else {
            self.setup.draft = None;
        }
    }
    pub(in crate::terminal_session) fn edit_setup_draft(
        &mut self,
        token: NativeSetupToken,
        edit: NativeSetupEdit<'_>,
    ) -> bool {
        if self.setup_draft().is_none() || self.quick_continuation().is_some() {
            return false;
        }
        let interactive = self.setup.presentation.interactive();
        let Some(draft) = self
            .setup
            .draft
            .as_mut()
            .filter(|d| d.token == token && d.shown && interactive)
        else {
            return false;
        };
        match edit {
            NativeSetupEdit::NextField
                if draft.complete
                    && draft.quick.is_some()
                    && matches!(
                        draft.stage,
                        NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory
                    ) =>
            {
                let Some(next) = self.setup.last_token.checked_add(1) else {
                    return false;
                };
                draft.stage = if draft.stage == NativeSetupStage::QuickName {
                    NativeSetupStage::QuickDirectory
                } else {
                    NativeSetupStage::QuickName
                };
                draft.token = NativeSetupToken(next);
                self.setup.last_token = next;
                draft.shown = false;
            }
            NativeSetupEdit::Completeness(value) => draft.complete = value,
            NativeSetupEdit::Fault => draft.faulted = true,
            NativeSetupEdit::SelectPreviousProject
            | NativeSetupEdit::SelectNextProject
            | NativeSetupEdit::ScrollProjects { .. }
                if draft.complete
                    && matches!(
                        draft.stage,
                        NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
                    ) =>
            {
                let Some(next_token) = self.setup.last_token.checked_add(1) else {
                    return false;
                };
                let count = draft.project_count();
                let selected = match edit {
                    NativeSetupEdit::ScrollProjects { up: true, lines } => {
                        draft.selected.saturating_sub(lines)
                    }
                    NativeSetupEdit::ScrollProjects { up: false, lines } => draft
                        .selected
                        .saturating_add(lines)
                        .min(count.saturating_sub(1)),
                    NativeSetupEdit::SelectNextProject => (draft.selected + 1) % count,
                    _ => (draft.selected + count - 1) % count,
                };
                if selected == draft.selected {
                    return true;
                }
                draft.selected = selected;
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
                if draft.complete && draft.text_field() =>
            {
                // Typed input and paste share the editor's one atomic insert
                // path; a rejected insertion faults without touching text.
                if draft.editor_mut().insert(value).is_err() {
                    draft.faulted = true;
                }
            }
            NativeSetupEdit::Backspace if draft.complete && draft.text_field() => {
                draft.editor_mut().backspace();
                draft.faulted = false;
            }
            NativeSetupEdit::Delete if draft.complete && draft.text_field() => {
                draft.editor_mut().delete();
                draft.faulted = false;
            }
            NativeSetupEdit::WordBackspace if draft.complete && draft.text_field() => {
                draft.editor_mut().word_back();
                draft.faulted = false;
            }
            NativeSetupEdit::Clear if draft.complete && draft.text_field() => {
                draft.editor_mut().clear();
                draft.faulted = false;
            }
            NativeSetupEdit::Left if draft.complete && draft.text_field() => {
                draft.editor_mut().left();
            }
            NativeSetupEdit::Right if draft.complete && draft.text_field() => {
                draft.editor_mut().right();
            }
            NativeSetupEdit::Home if draft.complete && draft.text_field() => {
                draft.editor_mut().home();
            }
            NativeSetupEdit::End if draft.complete && draft.text_field() => {
                draft.editor_mut().end();
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
        let interactive = self.setup.presentation.interactive();
        let Some(draft) = self.setup.draft.as_mut().filter(|d| {
            d.token == token
                && d.shown
                && interactive
                && d.complete
                && !d.faulted
                && matches!(
                    d.stage,
                    NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
                )
        }) else {
            return false;
        };
        if draft.stage == NativeSetupStage::ChooseWorkspaceProject
            && draft.selected <= draft.baseline.projects().len()
        {
            return false;
        }
        let Some(next) = self.setup.last_token.checked_add(1) else {
            return false;
        };
        let parent = SetupParent {
            stage: draft.stage,
            selected: draft.selected,
            project: draft.project.clone(),
            editor: draft.editor.clone(),
            complete: draft.complete,
            faulted: draft.faulted,
            quick: draft.quick.clone(),
        };
        draft.project = draft
            .baseline
            .projects()
            .get(draft.selected)
            .map(|p| p.project_id.clone());
        draft.stage = if draft.stage == NativeSetupStage::ChooseWorkspaceProject {
            NativeSetupStage::WorkspaceProjectName
        } else if let Some(fields) = &mut draft.quick {
            let Some(project) = &draft.project else {
                return false;
            };
            if !fields.select_project(project, &draft.baseline) {
                return false;
            }
            NativeSetupStage::QuickName
        } else {
            return false;
        };
        draft.token = NativeSetupToken(next);
        self.setup.last_token = next;
        draft.shown = false;
        draft.parent = Some(Box::new(parent));
        true
    }
}
impl SetupDraft {
    fn project_count(&self) -> usize {
        self.baseline.projects().len()
            + if self.stage == NativeSetupStage::ChooseWorkspaceProject {
                2
            } else {
                0
            }
    }
}
impl<'a> crate::terminal_session::state::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn quick_workspace_recovery(
        &self,
    ) -> Option<super::quick::QuickRecovery<'a>> {
        self.state.quick_workspace_recovery()
    }
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
