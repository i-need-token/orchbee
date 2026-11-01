//! The confirmed quick-launch fields and exact Project policy; I/O stays in the native executor.
use super::super::line_editor::{LengthLimit, LineEditor};
use super::ClientContextTabs;
use crate::localization::Locale;
use orchbee_workbench_protocol::{
    Project, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProjectId,
};
use std::time::Instant;

/// One accepted launch's local return; neither status nor a later visit can rearm it.
pub(super) enum ProjectQuickEntry {
    Preparing(SetupToken),
    Submitted(u64),
    Entering {
        generation: u64,
        key: super::super::PaneKey,
    },
}

impl WorkbenchClientState {
    pub(crate) fn cancel_project_quick_entry(&mut self) {
        self.setup.project_quick_entry = None;
    }

    pub(crate) fn take_project_quick_terminal_entry(&mut self) -> bool {
        let Some(ProjectQuickEntry::Entering { generation, key }) =
            self.setup.project_quick_entry.as_ref()
        else {
            return false;
        };
        let exact = !self.setup_blocks_structure()
            && self.desired() == Some(key)
            && self.setup.settled.as_ref().is_some_and(|known| {
                let super::WorkbenchSetupCommit::WorkspaceLaunched(receipt) = &known.commit else {
                    return false;
                };
                known.operation.generation == *generation
                    && known.target() == *self.cache.location()
                    && key.anchor().tab_id() == &receipt.tab_id
                    && key.panel() == &receipt.panel_id
                    && self.active().is_some_and(|selected| {
                        selected.key() == key
                            && self.cache.tabs().get(selected.index()).is_some_and(|tab| {
                                tab.panel(selected.panel_index()).is_some_and(|panel| {
                                    self.selected_presentation(panel).presentation_id()
                                        == &receipt.presentation_id
                                })
                            })
                    })
            });
        if !exact {
            self.cancel_project_quick_entry();
            return false;
        }
        if self.ready().is_none() {
            return false;
        }
        self.cancel_project_quick_entry();
        true
    }
}

/// A menu handoff to the existing quick-launch owner, with a fixed Project and source proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectQuickWorkspaceRequest {
    target: Box<QuickWorkspaceTarget>,
    deadline: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuickWorkspaceTarget {
    project: Project,
    /// A folder chosen in the menu replaces the focused Workspace's directory as the default.
    directory: Option<WorkbenchDirectoryPath>,
    source: ClientContextTabs,
    desired: Option<super::super::PaneKey>,
    focus: Option<super::super::PaneRef>,
}

impl QuickWorkspaceTarget {
    pub(crate) fn directory(&self) -> Option<&WorkbenchDirectoryPath> {
        self.directory.as_ref()
    }

    fn matches(&self, state: &WorkbenchClientState) -> bool {
        self.source == *state.cache()
            && self.desired.as_ref() == state.desired()
            && self.focus == state.focus_pane_ref()
            && state
                .cache()
                .projects()
                .iter()
                .find(|p| p.project_id == self.project.project_id)
                == Some(&self.project)
    }
}

impl ProjectQuickWorkspaceRequest {
    pub(crate) fn capture(
        state: &WorkbenchClientState,
        project: &Project,
        directory: Option<WorkbenchDirectoryPath>,
        deadline: Instant,
    ) -> Option<Self> {
        let target = QuickWorkspaceTarget {
            project: project.clone(),
            directory,
            source: state.cache().clone(),
            desired: state.desired().cloned(),
            focus: state.focus_pane_ref(),
        };
        target.matches(state).then(|| Self {
            target: Box::new(target),
            deadline,
        })
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QuickProject {
    Existing(WorkbenchProjectId),
    Targeted(Box<QuickWorkspaceTarget>),
    Ungrouped,
    Choose,
}

impl QuickProject {
    pub(super) fn existing_id(&self) -> Option<&WorkbenchProjectId> {
        match self {
            Self::Existing(id) => Some(id),
            Self::Targeted(target) => Some(&target.project.project_id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QuickWorkspaceFields {
    base: WorkbenchDirectoryPath,
    environment: Vec<(String, std::ffi::OsString)>,
    name: LineEditor,
    directory: LineEditor,
    project: QuickProject,
    directories: Vec<WorkbenchDirectoryPath>,
    automatic_name: bool,
    locale: Locale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuickLaunchPlan {
    pub(crate) cwd: WorkbenchDirectoryPath,
    pub(crate) label: WorkbenchLabel,
    pub(crate) project: QuickProject,
}

impl QuickWorkspaceFields {
    pub(crate) fn new(
        cache: &ClientContextTabs,
        cwd: WorkbenchDirectoryPath,
        locale: Locale,
    ) -> Self {
        Self::with_project(cache, QuickProject::Ungrouped, cwd, locale)
    }
    fn with_project(
        cache: &ClientContextTabs,
        project: QuickProject,
        cwd: WorkbenchDirectoryPath,
        locale: Locale,
    ) -> Self {
        let name = std::path::Path::new(cwd.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(locale.cli_default_workspace_label());
        let mut label = name.chars().take(512).collect::<String>();
        let mut ordinal = 2;
        // Membership is optional, so generated names avoid collisions across all
        // Workspaces in this snapshot. Concurrent creation still uses stable IDs.
        while cache
            .canonical_snapshot()
            .navigation
            .workspaces
            .iter()
            .any(|workspace| workspace.label == label)
        {
            let suffix = format!(" ({ordinal})");
            label = name.chars().take(512 - suffix.len()).collect::<String>();
            label.push_str(&suffix);
            ordinal += 1;
        }
        Self {
            name: LineEditor::new(label, LengthLimit::Scalars(512)),
            directory: LineEditor::new(cwd.as_str().to_owned(), LengthLimit::Bytes(4096)),
            directories: vec![cwd.clone()],
            base: cwd,
            environment: Vec::new(),
            project,
            automatic_name: true,
            locale,
        }
    }
    pub(crate) fn name(&self) -> &str {
        self.name.text()
    }
    pub(crate) fn directory(&self) -> &str {
        self.directory.text()
    }
    #[cfg(test)]
    pub(crate) fn name_editor(&mut self) -> &mut LineEditor {
        &mut self.name
    }
    #[cfg(test)]
    pub(crate) fn directory_editor(&mut self) -> &mut LineEditor {
        &mut self.directory
    }
    #[cfg(test)]
    pub(crate) fn set_environment(&mut self, environment: Vec<(String, std::ffi::OsString)>) {
        self.environment = environment;
    }
    pub(crate) fn project(&self) -> &QuickProject {
        &self.project
    }
    pub(crate) fn select_project(
        &mut self,
        id: &WorkbenchProjectId,
        cache: &ClientContextTabs,
    ) -> bool {
        if matches!(self.project, QuickProject::Targeted(_))
            || !cache
                .projects()
                .iter()
                .any(|project| &project.project_id == id)
        {
            return false;
        }
        self.project = QuickProject::Existing(id.clone());
        true
    }
    pub(crate) fn field(&self, directory: bool) -> &LineEditor {
        if directory {
            &self.directory
        } else {
            &self.name
        }
    }
    pub(super) fn directory_choices(&self) -> &[WorkbenchDirectoryPath] {
        &self.directories
    }
    pub(super) fn selected_directory(&self) -> Option<usize> {
        self.directories
            .iter()
            .position(|path| path.as_str().eq_ignore_ascii_case(self.directory()))
    }
    pub(super) fn select_directory(&mut self, index: usize, cache: &ClientContextTabs) -> bool {
        let Some(path) = self.directories.get(index) else {
            return false;
        };
        self.directory = LineEditor::new(path.as_str().to_owned(), LengthLimit::Bytes(4096));
        self.refresh_name(cache);
        true
    }
    pub(super) fn refresh_name(&mut self, cache: &ClientContextTabs) {
        if self.automatic_name
            && let Some(path) = self
                .normalized_path()
                .and_then(|path| WorkbenchDirectoryPath::parse(&path).ok())
        {
            self.name = Self::with_project(cache, self.project.clone(), path, self.locale).name;
        }
    }
    pub(crate) fn field_mut(&mut self, directory: bool) -> &mut LineEditor {
        if directory {
            &mut self.directory
        } else {
            self.automatic_name = false;
            &mut self.name
        }
    }
    pub(crate) fn name_valid(&self) -> bool {
        WorkbenchLabel::parse_workspace(self.name()).is_ok()
    }
    pub(crate) fn directory_valid(&self) -> bool {
        self.normalized_path()
            .is_some_and(|path| WorkbenchDirectoryPath::parse(&path).is_ok())
    }
    pub(crate) fn plan(&self) -> Option<QuickLaunchPlan> {
        if self.project == QuickProject::Choose {
            return None;
        }
        let path = self.normalized_path()?;
        Some(QuickLaunchPlan {
            cwd: WorkbenchDirectoryPath::parse(&path).ok()?,
            label: WorkbenchLabel::parse_workspace(self.name()).ok()?,
            project: self.project.clone(),
        })
    }
    pub(super) fn target_current(&self, state: &WorkbenchClientState) -> bool {
        match &self.project {
            QuickProject::Targeted(target) => target.matches(state),
            _ => true,
        }
    }
    fn normalized_path(&self) -> Option<String> {
        {
            orchbee_local_runtime::normalize_user_path(
                self.directory(),
                |name| {
                    self.environment
                        .iter()
                        .find(|(key, _)| key == name)
                        .map(|(_, value)| value.clone())
                },
                || Ok(std::path::PathBuf::from(self.base.as_str())),
            )
            .ok()
        }
    }
}

use super::{SetupDraft, SetupStage, SetupToken, WorkbenchClientState};
use crate::workbench_projection::ClientTerminalBinding;
use crate::workbench_state::OperationDeadlines;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QuickPreparation {
    pub(crate) review_directory: bool,
    pub(crate) target: Option<Box<QuickWorkspaceTarget>>,
    pub(crate) generation: u64,
    pub(crate) token: SetupToken,
    pub(crate) baseline: ClientContextTabs,
    /// The focused Workspace's first terminal: the first Tab's leading Panel primary binding,
    /// with this Client's live pane for it when one is attached.
    pub(crate) first_terminal: Option<ClientTerminalBinding>,
    pub(crate) first_terminal_pane: Option<super::super::PaneRef>,
    pub(crate) deadlines: OperationDeadlines,
}

impl WorkbenchClientState {
    pub(crate) fn quick_workspace_available(&self) -> bool {
        !self.setup_blocks_structure() && self.setup_authority_available() && self.invariant_holds()
    }

    pub(crate) fn settle_stale_quick_workspace(&mut self) -> bool {
        let stale = self.setup.draft.as_ref().is_some_and(|draft| {
            draft
                .quick
                .as_ref()
                .is_some_and(|fields| !fields.target_current(self))
        });
        if stale {
            self.setup.draft = None;
            self.setup.status = Some(super::SetupStatus::NotApplied);
        }
        stale
    }

    pub(crate) fn quick_workspace_recovery(&self) -> Option<QuickRecovery<'_>> {
        if self.setup.pending.is_some() && self.setup.uncertain.is_none() {
            return None;
        }
        if let Some(known) = &self.setup.known {
            known.operation.quick.as_ref()?;
            return match &known.commit {
                super::WorkbenchSetupCommit::WorkspaceLaunched(commit) => Some(QuickRecovery {
                    project: commit.project_id.as_ref().map(WorkbenchProjectId::as_str),
                    workspace: Some(commit.workspace_id.as_str()),
                    unknown: None,
                }),
                _ => None,
            };
        }
        if let Some(operation) = &self.setup.uncertain {
            operation.quick.as_ref()?;
            let project = match &operation.command.action {
                orchbee_workbench_protocol::WorkbenchAction::WorkspaceLaunch {
                    placement:
                        orchbee_workbench_protocol::WorkspacePlacement::Project { project_id },
                    ..
                } => Some(project_id.as_str()),
                // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
                _ => None,
            };
            return Some(QuickRecovery {
                project,
                workspace: None,
                unknown: Some(operation.command.request_id.as_str()),
            });
        }
        None
    }
    pub(crate) fn begin_quick_preparation(
        &mut self,
        generation: u64,
        deadlines: OperationDeadlines,
        request: Option<ProjectQuickWorkspaceRequest>,
    ) -> Option<QuickPreparation> {
        self.begin_quick_preparation_with_review(generation, deadlines, request, false)
    }

    pub(crate) fn begin_quick_preparation_with_review(
        &mut self,
        generation: u64,
        deadlines: OperationDeadlines,
        request: Option<ProjectQuickWorkspaceRequest>,
        review_directory: bool,
    ) -> Option<QuickPreparation> {
        if !self.quick_workspace_available()
            || request
                .as_ref()
                .is_some_and(|request| !request.target.matches(self))
        {
            return None;
        }
        let token = SetupToken(self.setup.last_token.checked_add(1)?);
        let first_terminal = self
            .cache
            .tabs()
            .first()
            .and_then(|tab| tab.panel(0)?.primary().binding())
            .cloned();
        let first_terminal_pane = first_terminal.as_ref().and_then(|binding| {
            self.panes
                .iter()
                .find(|record| {
                    !record.departing && record.attached() && record.binding() == binding
                })
                .map(super::super::PaneRecord::pane_ref)
        });
        let preparation = QuickPreparation {
            review_directory,
            target: request.map(|request| request.target),
            generation,
            token,
            baseline: self.cache.clone(),
            first_terminal,
            first_terminal_pane,
            deadlines,
        };
        if !self.start_pending_refresh(generation) {
            return None;
        }
        self.setup.last_token = token.0;
        self.setup.status = None;
        self.setup.presentation = crate::tui_presentation::PresentationPhase::Pending;
        self.setup.preparation = Some(preparation.clone());
        {
            self.setup.project_quick_entry = Some(ProjectQuickEntry::Preparing(token));
        }
        self.setup.draft = Some(SetupDraft {
            organization: None,
            tab_configuration: None,
            project_rename: None,
            token,
            baseline: self.cache.clone(),
            stage: SetupStage::PreparingWorkspace,
            selected: 0,
            project: preparation
                .target
                .as_ref()
                .map(|target| target.project.project_id.clone()),
            editor: LineEditor::new(String::new(), LengthLimit::Scalars(512)),
            shown: false,
            complete: true,
            faulted: false,
            touched: [false; 2],
            quick: None,
            continuation: None,
            parent: None,
        });
        Some(preparation)
    }
    pub(crate) fn take_quick_preparation(
        &mut self,
        preparation: &QuickPreparation,
    ) -> Option<bool> {
        if self.setup.preparation.as_ref() != Some(preparation) {
            return None;
        }
        self.setup.preparation = None;
        let shown = self
            .setup
            .draft
            .as_ref()
            .is_some_and(|draft| draft.token == preparation.token);
        self.setup.draft = None;
        Some(shown)
    }
    pub(crate) fn finish_quick_preparation(
        &mut self,
        preparation: &QuickPreparation,
        cwd: Option<WorkbenchDirectoryPath>,
        locale: Locale,
        environment: Vec<(String, std::ffi::OsString)>,
    ) {
        if self.cache != preparation.baseline
            || !self.setup_authority_available()
            || preparation
                .target
                .as_ref()
                .is_some_and(|target| !target.matches(self))
        {
            self.setup.status = Some(super::SetupStatus::NotApplied);
            return;
        }
        let Some(cwd) = cwd else {
            self.setup.status = Some(super::SetupStatus::NotApplied);
            return;
        };
        let Some(next) = self.setup.last_token.checked_add(1) else {
            return;
        };
        let mut fields = match &preparation.target {
            Some(target) => QuickWorkspaceFields::with_project(
                &self.cache,
                QuickProject::Targeted(target.clone()),
                cwd,
                locale,
            ),
            None => QuickWorkspaceFields::new(&self.cache, cwd, locale),
        };
        fields.environment = environment;
        // Current directory first, then recent selections, then other known Workspaces.
        // Choices are frozen with the draft and bounded independently of navigation size.
        let locations = self.navigation_memory.recent_locations().chain(
            self.cache
                .navigation()
                .iter()
                .map(|context| &context.location),
        );
        for location in locations {
            let crate::workbench_projection::ClientContextLocation::Workspace {
                workspace_id, ..
            } = location
            else {
                continue;
            };
            let Some(directory) = self
                .cache
                .canonical_snapshot()
                .navigation
                .workspace_directory(workspace_id)
            else {
                continue;
            };
            let path = directory.path.clone();
            if !fields
                .directories
                .iter()
                .any(|prior| prior.as_str().eq_ignore_ascii_case(path.as_str()))
            {
                fields.directories.push(path);
            }
            if fields.directories.len() >= 8 {
                break;
            }
        }
        let stage = if preparation.review_directory {
            SetupStage::QuickDirectory
        } else if fields.project() == &QuickProject::Choose {
            SetupStage::QuickProject
        } else {
            SetupStage::QuickName
        };
        let project = match fields.project() {
            QuickProject::Existing(id) => Some(id.clone()),
            QuickProject::Targeted(target) => Some(target.project.project_id.clone()),
            _ => None,
        };
        self.setup.last_token = next;
        if matches!(self.setup.project_quick_entry, Some(ProjectQuickEntry::Preparing(token)) if token == preparation.token)
        {
            self.setup.project_quick_entry = Some(ProjectQuickEntry::Preparing(SetupToken(next)));
        }
        self.setup.draft = Some(SetupDraft {
            organization: None,
            tab_configuration: None,
            project_rename: None,
            token: SetupToken(next),
            baseline: self.cache.clone(),
            stage,
            selected: 0,
            project,
            editor: LineEditor::new(String::new(), LengthLimit::Scalars(512)),
            shown: false,
            complete: true,
            faulted: false,
            touched: [false; 2],
            quick: Some(fields),
            continuation: (!preparation.review_directory).then_some(preparation.deadlines),
            parent: None,
        });
    }
    pub(crate) fn quick_continuation(&self) -> Option<(SetupToken, OperationDeadlines)> {
        let draft = self.setup.draft.as_ref()?;
        Some((draft.token, draft.continuation?))
    }
    pub(crate) fn abort_quick_continuation(&mut self, token: SetupToken) {
        if self
            .setup
            .draft
            .as_ref()
            .is_some_and(|draft| draft.token == token && draft.continuation.is_some())
        {
            self.setup.draft = None;
            self.setup.status = Some(super::SetupStatus::NotApplied);
        }
    }
}

pub(crate) struct QuickRecovery<'a> {
    pub(crate) project: Option<&'a str>,
    pub(crate) workspace: Option<&'a str>,
    pub(crate) unknown: Option<&'a str>,
}
