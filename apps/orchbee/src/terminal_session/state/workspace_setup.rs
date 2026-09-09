use super::{
    AttachmentKey, NativeContextTabs, NativeSessionState, NavigatorRefreshStatus,
    OperationDeadlines, PreparedRefresh, SelectedTab, SessionPhase, validate_canonical_refresh,
};
use crate::local_client::workbench::workspace_setup::{
    WorkbenchSetupCommit, WorkbenchSetupRequest,
};
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, project_native_context_tabs,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchDirectoryPath,
    WorkbenchLabel, WorkbenchProjectId, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSetupToken(u64);
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
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSetupStatus {
    NotApplied,
    ProjectCreated,
    WorkspaceCreated,
    DirectoryAccepted,
    CommittedNeedRefresh,
    OutcomeUnknown,
}
pub(crate) enum NativeSetupEdit<'a> {
    Append(&'a str),
    Backspace,
    Clear,
    Fault,
    Completeness(bool),
    SelectPreviousProject,
    SelectNextProject,
}
#[derive(Default)]
pub(super) struct SetupState {
    last_token: u64,
    draft: Option<SetupDraft>,
    pending: Option<SetupOperation>,
    known: Option<SetupKnownCommit>,
    settled: Option<SetupKnownCommit>,
    not_applied: Option<SetupOperation>,
    uncertain: Option<SetupOperation>,
    status: Option<NativeSetupStatus>,
    deadlines: Option<OperationDeadlines>,
}
#[derive(Clone, Debug)]
struct SetupDraft {
    token: NativeSetupToken,
    baseline: NativeContextTabs,
    stage: NativeSetupStage,
    selected: usize,
    project: Option<WorkbenchProjectId>,
    known_project_created: bool,
    text: String,
    shown: bool,
    complete: bool,
    faulted: bool,
}
pub(crate) struct NativeSetupDraftView<'a> {
    draft: &'a SetupDraft,
}
pub(crate) struct NativeSetupProjectView<'a> {
    position: usize,
    selected: bool,
    label: Option<&'a str>,
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
    pub(crate) fn text(&self) -> &'a str {
        &self.draft.text
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct SetupOperation {
    pub(in crate::terminal_session) generation: u64,
    pub(in crate::terminal_session) command: WorkbenchCommand,
    pub(in crate::terminal_session) request: WorkbenchSetupRequest,
    baseline: NativeContextTabs,
    pub(in crate::terminal_session) deadlines: OperationDeadlines,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct SetupKnownCommit {
    pub(in crate::terminal_session) operation: SetupOperation,
    pub(in crate::terminal_session) commit: WorkbenchSetupCommit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum SetupWorkerEvidence {
    Known(SetupKnownCommit),
    Unknown(SetupOperation),
    NotApplied(SetupOperation),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum SetupResult {
    NotApplied,
    Committed {
        known: Box<SetupKnownCommit>,
        outcome: Box<WorkbenchSnapshotOutcome>,
    },
}
impl SetupOperation {
    pub(in crate::terminal_session) fn known_commit(
        &self,
        commit: WorkbenchSetupCommit,
    ) -> Option<SetupKnownCommit> {
        if self.command.expected_revision.value().checked_add(1) != Some(commit.revision().value())
        {
            return None;
        }
        let compatible = matches!(
            (&self.command.action, &commit),
            (
                WorkbenchAction::ProjectCreate { .. },
                WorkbenchSetupCommit::ProjectCreated { .. }
            ) | (
                WorkbenchAction::WorkspaceCreate { .. },
                WorkbenchSetupCommit::WorkspaceCreated { .. }
            ) | (
                WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. },
                WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted { .. }
            )
        );
        compatible.then(|| SetupKnownCommit {
            operation: self.clone(),
            commit,
        })
    }
}
impl SetupKnownCommit {
    fn target(&self) -> NativeContextLocation {
        match (&self.operation.command.action, &self.commit) {
            (
                WorkbenchAction::WorkspaceCreate { project_id, .. },
                WorkbenchSetupCommit::WorkspaceCreated {
                    workspace_id,
                    terminal_context_id,
                    ..
                },
            ) => NativeContextLocation::Workspace {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                terminal_context_id: terminal_context_id.clone(),
            },
            _ => self.operation.baseline.location().clone(),
        }
    }
    fn validate(&self, snapshot: &Snapshot) -> Option<NativeContextTabs> {
        let baseline = &self.operation.baseline;
        if snapshot.site_id != *baseline.site_id() || snapshot.revision < self.commit.revision() {
            return None;
        }
        let active =
            validate_canonical_refresh(baseline, baseline.project_snapshot(snapshot).ok()?).ok()?;
        let exact = snapshot.revision == self.commit.revision();
        match (&self.operation.command.action, &self.commit) {
            (
                WorkbenchAction::ProjectCreate { label },
                WorkbenchSetupCommit::ProjectCreated { project_id, .. },
            ) => {
                if baseline
                    .projects()
                    .iter()
                    .any(|p| &p.project_id == project_id)
                {
                    return None;
                }
                let project = snapshot
                    .navigation
                    .projects
                    .iter()
                    .find(|p| &p.project_id == project_id)?;
                if exact
                    && (project.label != label.as_str()
                        || !project.workspaces.is_empty()
                        || !project.directories.is_empty())
                {
                    return None;
                }
            }
            (
                WorkbenchAction::WorkspaceCreate { project_id, label },
                WorkbenchSetupCommit::WorkspaceCreated {
                    workspace_id,
                    terminal_context_id,
                    ..
                },
            ) => {
                if baseline.navigation().iter().any(|c| c.location.terminal_context_id() == terminal_context_id || matches!(&c.location, NativeContextLocation::Workspace {workspace_id: old, ..} if old == workspace_id)) { return None; }
                let project = snapshot
                    .navigation
                    .projects
                    .iter()
                    .find(|p| &p.project_id == project_id)?;
                let workspace = project
                    .workspaces
                    .iter()
                    .find(|w| &w.workspace_id == workspace_id)?;
                if &workspace.terminal_context_id != terminal_context_id
                    || exact
                        && (workspace.label != label.as_str()
                            || workspace.primary_directory_id.is_some())
                {
                    return None;
                }
                let context = snapshot
                    .terminal_contexts
                    .iter()
                    .find(|c| &c.terminal_context_id == terminal_context_id)?;
                if exact && (!context.tabs.is_empty() || !context.groups.is_empty()) {
                    return None;
                }
            }
            (
                WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    project_id,
                    workspace_id,
                    path,
                },
                WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
                    directory_id,
                    worker_id,
                    ..
                },
            ) => {
                if baseline.projects().iter().any(|project| {
                    project
                        .directories
                        .iter()
                        .any(|directory| &directory.directory_id == directory_id)
                }) {
                    return None;
                }
                let project = snapshot
                    .navigation
                    .projects
                    .iter()
                    .find(|p| &p.project_id == project_id)?;
                let workspace = project
                    .workspaces
                    .iter()
                    .find(|w| &w.workspace_id == workspace_id)?;
                if workspace.primary_directory_id.as_ref() != Some(directory_id) {
                    return None;
                }
                let directory = project
                    .directories
                    .iter()
                    .find(|d| &d.directory_id == directory_id)?;
                if &directory.worker_id != worker_id || &directory.path != path {
                    return None;
                }
            }
            _ => return None,
        }
        if exact {
            if active.tabs() != baseline.tabs() {
                return None;
            }
            let mut projects = snapshot.navigation.projects.clone();
            match (&self.operation.command.action, &self.commit) {
                (
                    WorkbenchAction::ProjectCreate { .. },
                    WorkbenchSetupCommit::ProjectCreated { project_id, .. },
                ) => {
                    let index = projects
                        .iter()
                        .position(|project| &project.project_id == project_id)?;
                    projects.remove(index);
                }
                (
                    WorkbenchAction::WorkspaceCreate { project_id, .. },
                    WorkbenchSetupCommit::WorkspaceCreated { workspace_id, .. },
                ) => {
                    let project = projects
                        .iter_mut()
                        .find(|project| &project.project_id == project_id)?;
                    let index = project
                        .workspaces
                        .iter()
                        .position(|workspace| &workspace.workspace_id == workspace_id)?;
                    project.workspaces.remove(index);
                }
                (
                    WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                        project_id,
                        workspace_id,
                        ..
                    },
                    WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
                        directory_id, ..
                    },
                ) => {
                    let project = projects
                        .iter_mut()
                        .find(|project| &project.project_id == project_id)?;
                    let index = project
                        .directories
                        .iter()
                        .position(|directory| &directory.directory_id == directory_id)?;
                    project.directories.remove(index);
                    project
                        .workspaces
                        .iter_mut()
                        .find(|workspace| &workspace.workspace_id == workspace_id)?
                        .primary_directory_id = None;
                }
                _ => return None,
            }
            if projects != baseline.projects() {
                return None;
            }
        }
        project_native_context_tabs(snapshot, &self.target()).ok()
    }
}
impl NativeSessionState {
    pub(super) fn setup_invariant_holds(&self) -> bool {
        let pending = self.setup.pending.as_ref();
        let known = self.setup.known.as_ref();
        if self.setup.draft.is_some()
            && (pending.is_some() || known.is_some() || self.setup.uncertain.is_some())
        {
            return false;
        }
        if pending.is_some() && known.is_some() {
            return false;
        }
        if self.setup_blocks_structure() && self.pending_mutation.is_some() {
            return false;
        }
        if self
            .setup
            .uncertain
            .as_ref()
            .is_some_and(|operation| pending != Some(operation))
        {
            return false;
        }
        if self.setup.draft.as_ref().is_some_and(|draft| {
            draft.baseline != self.cache || draft.token.0 != self.setup.last_token
        }) {
            return false;
        }
        pending
            .or_else(|| known.map(|known| &known.operation))
            .is_none_or(|operation| {
                operation.baseline == self.cache
                    && operation.command.site_id == *self.cache.site_id()
                    && operation.command.expected_revision == self.cache.revision()
                    && operation.request.request_id() == &operation.command.request_id
            })
    }
    fn setup_authority_available(&self) -> bool {
        self.pending_refresh_generation.is_none()
            && self.pending_mutation.is_none()
            && self.pending_membership.is_none()
            && self.move_recovery.is_none()
            && self.rename_recovery.is_none()
            && !self.rename_draft_blocks_structure()
            && self.close.permits_retired_membership()
            && matches!(
                self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }
    pub(in crate::terminal_session) fn setup_blocks_structure(&self) -> bool {
        self.setup.draft.is_some()
            || self.setup.pending.is_some()
            || self.setup.known.is_some()
            || self.setup.uncertain.is_some()
    }
    pub(in crate::terminal_session) fn setup_recovery(&self) -> bool {
        self.setup.known.is_some() && self.setup.pending.is_none()
    }
    pub(in crate::terminal_session) fn setup_pending_or_draft(&self) -> bool {
        self.setup.pending.is_some() || self.setup.draft.is_some() || self.setup.uncertain.is_some()
    }
    pub(in crate::terminal_session) fn workspace_directory_unconfigured(&self) -> bool {
        matches!(
            self.cache.location(),
            NativeContextLocation::Workspace { .. }
        ) && self.cache.directory().is_none()
    }
    pub(in crate::terminal_session) fn workspace_directory_path(&self) -> Option<&str> {
        self.cache
            .directory()
            .map(|directory| directory.path.as_str())
    }
    pub(in crate::terminal_session) fn open_setup_draft(
        &mut self,
        directory: bool,
    ) -> Option<NativeSetupToken> {
        if self.setup_blocks_structure()
            || !self.setup_authority_available()
            || !self.invariant_holds()
        {
            return None;
        }
        if directory
            && (!matches!(self.phase, SessionPhase::Empty)
                || !self.workspace_directory_unconfigured())
        {
            return None;
        }
        let project = match self.cache.location() {
            NativeContextLocation::Workspace { project_id, .. } => Some(project_id.clone()),
            _ => None,
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
            stage: if directory {
                NativeSetupStage::DirectoryPath
            } else {
                NativeSetupStage::ChooseProject
            },
            selected,
            project,
            known_project_created: false,
            text: String::new(),
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
    pub(super) fn cancel_setup_on_retirement(&mut self) {
        self.setup.draft = None;
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
            NativeSetupEdit::Append(value)
                if draft.complete && draft.stage != NativeSetupStage::ChooseProject =>
            {
                let fits = if draft.stage == NativeSetupStage::DirectoryPath {
                    draft.text.len().saturating_add(value.len()) <= 4096
                } else {
                    draft
                        .text
                        .chars()
                        .count()
                        .saturating_add(value.chars().count())
                        <= 512
                };
                if fits && !value.contains('\0') {
                    draft.text.push_str(value);
                } else {
                    draft.faulted = true;
                }
            }
            NativeSetupEdit::Backspace if draft.complete => {
                draft.text.pop();
                draft.faulted = false;
            }
            NativeSetupEdit::Clear if draft.complete => {
                draft.text.clear();
                draft.faulted = false;
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
    pub(in crate::terminal_session) fn plan_setup(
        &self,
        token: NativeSetupToken,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<SetupOperation> {
        let draft = self.setup_draft()?.draft;
        if draft.token != token || !draft.shown || !draft.complete || draft.faulted {
            return None;
        }
        let action = match draft.stage {
            NativeSetupStage::ProjectLabel => WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse(&draft.text).ok()?,
            },
            NativeSetupStage::WorkspaceLabel => WorkbenchAction::WorkspaceCreate {
                project_id: draft.project.clone()?,
                label: WorkbenchLabel::parse(&draft.text).ok()?,
            },
            NativeSetupStage::DirectoryPath => {
                let NativeContextLocation::Workspace {
                    project_id,
                    workspace_id,
                    ..
                } = self.cache.location()
                else {
                    return None;
                };
                if !matches!(self.phase, SessionPhase::Empty)
                    || !self.workspace_directory_unconfigured()
                {
                    return None;
                }
                WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                    path: WorkbenchDirectoryPath::parse(&draft.text).ok()?,
                }
            }
            NativeSetupStage::ChooseProject => return None,
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            site_id: self.cache.site_id().clone(),
            expected_revision: self.cache.revision(),
            action,
        };
        Some(SetupOperation {
            generation,
            request: WorkbenchSetupRequest::try_from(command.clone()).ok()?,
            command,
            baseline: self.cache.clone(),
            deadlines,
        })
    }
    pub(in crate::terminal_session) fn begin_setup(&mut self, operation: SetupOperation) {
        self.setup.draft = None;
        self.setup.deadlines = Some(operation.deadlines);
        self.pending_refresh_generation = Some(operation.generation);
        self.refresh_status = Some(NavigatorRefreshStatus::Refreshing);
        self.setup.pending = Some(operation);
    }
    pub(in crate::terminal_session) fn retain_setup_worker(
        &mut self,
        evidence: SetupWorkerEvidence,
    ) -> bool {
        match evidence {
            SetupWorkerEvidence::NotApplied(operation) => self.settle_setup_not_applied(&operation),
            SetupWorkerEvidence::Known(known) => {
                if self.setup.settled.as_ref() == Some(&known) {
                    return true;
                }
                if self.setup.pending.as_ref() != Some(&known.operation)
                    && self.setup.known.as_ref() != Some(&known)
                {
                    return false;
                }
                self.setup.pending = None;
                self.setup.known = Some(known);
                self.setup.status = Some(NativeSetupStatus::CommittedNeedRefresh);
                true
            }
            SetupWorkerEvidence::Unknown(operation) => {
                if self.setup.pending.as_ref() != Some(&operation) {
                    return false;
                }
                self.setup.uncertain = Some(operation);
                self.setup.status = Some(NativeSetupStatus::OutcomeUnknown);
                true
            }
        }
    }
    pub(in crate::terminal_session) fn settle_setup_not_applied(
        &mut self,
        operation: &SetupOperation,
    ) -> bool {
        if self.setup.not_applied.as_ref() == Some(operation) {
            return true;
        }
        if self.setup.pending.as_ref() != Some(operation) {
            return false;
        }
        self.setup.not_applied = self.setup.pending.take();
        self.setup.deadlines = None;
        self.setup.status = Some(NativeSetupStatus::NotApplied);
        true
    }
    pub(super) fn setup_target_matches(&self, target: &NativeContextLocation) -> bool {
        self.setup
            .known
            .as_ref()
            .is_some_and(|known| &known.target() == target)
    }
    pub(in crate::terminal_session) fn setup_deadlines(&self) -> Option<OperationDeadlines> {
        self.setup.deadlines
    }
    pub(super) fn prepare_setup_refresh(
        &self,
        generation: u64,
        snapshot: &Snapshot,
    ) -> PreparedRefresh {
        let prepared = self
            .setup
            .known
            .as_ref()
            .and_then(|known| known.validate(snapshot))
            .and_then(|candidate| {
                let same = candidate.location() == self.cache.location();
                self.prepare_membership_candidate(
                    candidate,
                    same.then(|| self.desired.as_ref().map(AttachmentKey::tab_id))
                        .flatten(),
                    self.active().filter(|_| same).map(SelectedTab::index),
                    None,
                )
                .ok()
            });
        match prepared {
            Some(prepared) => PreparedRefresh::Membership {
                generation,
                prepared,
            },
            None => PreparedRefresh::Divergent { generation },
        }
    }
    pub(super) fn prepare_setup_completion(
        &self,
        candidate: &NativeContextTabs,
    ) -> Option<Box<PreparedSetupCompletion>> {
        let known = self.setup.known.as_ref()?;
        let status = match &known.commit {
            WorkbenchSetupCommit::ProjectCreated { .. } => NativeSetupStatus::ProjectCreated,
            WorkbenchSetupCommit::WorkspaceCreated { .. } => NativeSetupStatus::WorkspaceCreated,
            WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted { .. } => {
                NativeSetupStatus::DirectoryAccepted
            }
        };
        let draft = match &known.commit {
            WorkbenchSetupCommit::ProjectCreated { project_id, .. } => {
                self.setup.last_token.checked_add(1).map(|next| SetupDraft {
                    token: NativeSetupToken(next),
                    baseline: candidate.clone(),
                    stage: NativeSetupStage::WorkspaceLabel,
                    selected: candidate
                        .projects()
                        .iter()
                        .position(|p| &p.project_id == project_id)
                        .unwrap_or(0),
                    project: Some(project_id.clone()),
                    known_project_created: true,
                    text: String::new(),
                    shown: false,
                    complete: true,
                    faulted: false,
                })
            }
            _ => None,
        };
        Some(Box::new(PreparedSetupCompletion {
            known: known.clone(),
            status,
            draft,
        }))
    }
    pub(super) fn complete_setup_refresh(&mut self, prepared: PreparedSetupCompletion) {
        let PreparedSetupCompletion {
            known,
            status,
            draft,
        } = prepared;
        debug_assert_eq!(self.setup.known.as_ref(), Some(&known));
        self.setup.known = None;
        self.setup.deadlines = None;
        self.setup.status = Some(status);
        if let Some(draft) = &draft {
            self.setup.last_token = draft.token.0;
        }
        self.setup.draft = draft;
        self.setup.settled = Some(known);
    }
    pub(in crate::terminal_session) fn reset_setup_recovery_deadlines(
        &mut self,
        deadlines: OperationDeadlines,
    ) {
        self.setup.deadlines = Some(deadlines);
    }
}

#[derive(Clone, Debug)]
pub(super) struct PreparedSetupCompletion {
    known: SetupKnownCommit,
    status: NativeSetupStatus,
    draft: Option<SetupDraft>,
}
impl<'a> super::NativeSessionView<'a> {
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
