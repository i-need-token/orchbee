//! The confirmed quick-launch fields and exact Project policy; I/O stays in the native executor.
use super::super::line_editor::{LengthLimit, LineEditor};
use super::{NativeContextLocation, NativeContextTabs};
use crate::localization::Locale;
use orchbee_workbench_protocol::{
    Project, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProjectId,
};
use std::time::Instant;

/// One accepted targeted launch's local return; neither status nor a later visit can rearm it.
#[cfg(any(windows, test))]
pub(super) enum ProjectQuickEntry {
    Submitted(u64),
    Entering {
        generation: u64,
        key: super::super::PaneKey,
    },
}

#[cfg(any(windows, test))]
impl NativeSessionState {
    pub(in crate::terminal_session) fn cancel_project_quick_entry(&mut self) {
        self.setup.project_quick_entry = None;
    }

    pub(in crate::terminal_session) fn take_project_quick_terminal_entry(&mut self) -> bool {
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
pub(in crate::terminal_session) struct QuickWorkspaceTarget {
    project: Project,
    source: NativeContextTabs,
    desired: Option<super::super::PaneKey>,
    focus: Option<super::super::PaneRef>,
}

impl QuickWorkspaceTarget {
    fn matches(&self, state: &NativeSessionState) -> bool {
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
    pub(in crate::terminal_session) fn capture(
        state: &NativeSessionState,
        project: &Project,
        deadline: Instant,
    ) -> Option<Self> {
        let target = QuickWorkspaceTarget {
            project: project.clone(),
            source: state.cache().clone(),
            desired: state.desired().cloned(),
            focus: state.focus_pane_ref(),
        };
        target.matches(state).then(|| Self {
            target: Box::new(target),
            deadline,
        })
    }

    pub(in crate::terminal_session) fn deadline(&self) -> Instant {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum QuickProject {
    Existing(WorkbenchProjectId),
    Targeted(Box<QuickWorkspaceTarget>),
    CreateDefault,
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
pub(in crate::terminal_session) struct QuickWorkspaceFields {
    base: WorkbenchDirectoryPath,
    name: LineEditor,
    directory: LineEditor,
    project: QuickProject,
    default_project_label: WorkbenchLabel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct QuickLaunchPlan {
    pub(in crate::terminal_session) cwd: WorkbenchDirectoryPath,
    pub(in crate::terminal_session) label: WorkbenchLabel,
    pub(in crate::terminal_session) project: QuickProject,
    pub(in crate::terminal_session) default_project_label: WorkbenchLabel,
}

impl QuickWorkspaceFields {
    pub(in crate::terminal_session) fn new(
        cache: &NativeContextTabs,
        cwd: WorkbenchDirectoryPath,
        locale: Locale,
    ) -> Self {
        let project = match cache.location() {
            NativeContextLocation::Workspace { project_id, .. } => {
                QuickProject::Existing(project_id.clone())
            }
            _ => match cache.projects() {
                [] => QuickProject::CreateDefault,
                [project] => QuickProject::Existing(project.project_id.clone()),
                _ => QuickProject::Choose,
            },
        };
        Self::with_project(project, cwd, locale)
    }
    fn with_project(project: QuickProject, cwd: WorkbenchDirectoryPath, locale: Locale) -> Self {
        let name = std::path::Path::new(cwd.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(locale.cli_default_workspace_label());
        let label = name.chars().take(512).collect::<String>();
        Self {
            name: LineEditor::new(label, LengthLimit::Scalars(512)),
            directory: LineEditor::new(cwd.as_str().to_owned(), LengthLimit::Bytes(4096)),
            base: cwd,
            project,
            default_project_label: WorkbenchLabel::parse(locale.cli_default_project_label())
                .expect("localized Default Project label"),
        }
    }
    pub(in crate::terminal_session) fn name(&self) -> &str {
        self.name.text()
    }
    pub(in crate::terminal_session) fn directory(&self) -> &str {
        self.directory.text()
    }
    #[cfg(test)]
    pub(in crate::terminal_session) fn name_editor(&mut self) -> &mut LineEditor {
        &mut self.name
    }
    #[cfg(test)]
    pub(in crate::terminal_session) fn directory_editor(&mut self) -> &mut LineEditor {
        &mut self.directory
    }
    pub(in crate::terminal_session) fn project(&self) -> &QuickProject {
        &self.project
    }
    pub(in crate::terminal_session) fn select_project(
        &mut self,
        id: &WorkbenchProjectId,
        cache: &NativeContextTabs,
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
    pub(in crate::terminal_session) fn field(&self, directory: bool) -> &LineEditor {
        if directory {
            &self.directory
        } else {
            &self.name
        }
    }
    pub(in crate::terminal_session) fn field_mut(&mut self, directory: bool) -> &mut LineEditor {
        if directory {
            &mut self.directory
        } else {
            &mut self.name
        }
    }
    pub(super) fn continuation(plan: &QuickLaunchPlan, project: WorkbenchProjectId) -> Self {
        Self {
            base: plan.cwd.clone(),
            name: LineEditor::new(plan.label.as_str().to_owned(), LengthLimit::Scalars(512)),
            directory: LineEditor::new(plan.cwd.as_str().to_owned(), LengthLimit::Bytes(4096)),
            project: QuickProject::Existing(project),
            default_project_label: plan.default_project_label.clone(),
        }
    }
    pub(in crate::terminal_session) fn plan(&self) -> Option<QuickLaunchPlan> {
        if self.project == QuickProject::Choose {
            return None;
        }
        let path = self.normalized_path()?;
        Some(QuickLaunchPlan {
            cwd: WorkbenchDirectoryPath::parse(&path).ok()?,
            label: WorkbenchLabel::parse(self.name()).ok()?,
            project: self.project.clone(),
            default_project_label: self.default_project_label.clone(),
        })
    }
    pub(super) fn target_current(&self, state: &NativeSessionState) -> bool {
        match &self.project {
            QuickProject::Targeted(target) => target.matches(state),
            _ => true,
        }
    }
    fn normalized_path(&self) -> Option<String> {
        #[cfg(windows)]
        {
            orchbee_local_site_runtime::normalize_windows_user_path(
                self.directory(),
                |name| std::env::var_os(name),
                || Ok(std::path::PathBuf::from(self.base.as_str())),
            )
            .ok()
        }
        #[cfg(not(windows))]
        {
            let _ = &self.base;
            None
        }
    }
}

use super::{NativeSessionState, NativeSetupStage, NativeSetupToken, SetupDraft};
use crate::terminal_session::{
    state::OperationDeadlines, workbench_projection::NativeTerminalBinding,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct QuickPreparation {
    pub(in crate::terminal_session) target: Option<Box<QuickWorkspaceTarget>>,
    pub(in crate::terminal_session) generation: u64,
    pub(in crate::terminal_session) token: NativeSetupToken,
    pub(in crate::terminal_session) baseline: NativeContextTabs,
    pub(in crate::terminal_session) primary: Option<NativeTerminalBinding>,
    pub(in crate::terminal_session) primary_pane: Option<super::super::PaneRef>,
    pub(in crate::terminal_session) deadlines: OperationDeadlines,
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn quick_workspace_available(&self) -> bool {
        !self.setup_blocks_structure() && self.setup_authority_available() && self.invariant_holds()
    }

    pub(in crate::terminal_session) fn settle_stale_quick_workspace(&mut self) -> bool {
        let stale = self.setup.draft.as_ref().is_some_and(|draft| {
            draft
                .quick
                .as_ref()
                .is_some_and(|fields| !fields.target_current(self))
        });
        if stale {
            self.setup.draft = None;
            self.setup.status = Some(super::NativeSetupStatus::NotApplied);
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
                    project: Some(commit.project_id.as_str()),
                    workspace: Some(commit.workspace_id.as_str()),
                    unknown: None,
                }),
                super::WorkbenchSetupCommit::ProjectCreated { project_id, .. } => {
                    Some(QuickRecovery {
                        project: Some(project_id.as_str()),
                        workspace: None,
                        unknown: None,
                    })
                }
                _ => None,
            };
        }
        if let Some(operation) = &self.setup.uncertain {
            operation.quick.as_ref()?;
            let project = match &operation.command.action {
                orchbee_workbench_protocol::WorkbenchAction::WorkspaceLaunch {
                    project_id, ..
                } => Some(project_id.as_str()),
                _ => None,
            };
            return Some(QuickRecovery {
                project,
                workspace: None,
                unknown: Some(operation.command.request_id.as_str()),
            });
        }
        if matches!(
            self.setup.status,
            Some(super::NativeSetupStatus::NotApplied | super::NativeSetupStatus::ProjectCreated)
        ) {
            let known = self.setup.settled.as_ref()?;
            known.operation.quick.as_ref()?;
            if let super::WorkbenchSetupCommit::ProjectCreated { project_id, .. } = &known.commit {
                return Some(QuickRecovery {
                    project: Some(project_id.as_str()),
                    workspace: None,
                    unknown: None,
                });
            }
        }
        None
    }
    pub(in crate::terminal_session) fn begin_quick_preparation(
        &mut self,
        generation: u64,
        deadlines: OperationDeadlines,
        request: Option<ProjectQuickWorkspaceRequest>,
    ) -> Option<QuickPreparation> {
        if !self.quick_workspace_available()
            || request
                .as_ref()
                .is_some_and(|request| !request.target.matches(self))
        {
            return None;
        }
        let token = NativeSetupToken(self.setup.last_token.checked_add(1)?);
        let primary = self
            .active()
            .and_then(|selected| {
                self.cache
                    .tabs()
                    .get(selected.index())
                    .and_then(|tab| tab.panel(selected.panel_index()))
            })
            .and_then(|panel| panel.primary().binding())
            .cloned();
        let primary_pane = self
            .focus_record()
            .filter(|record| Some(&record.binding) == primary.as_ref())
            .map(super::super::PaneRecord::pane_ref);
        let preparation = QuickPreparation {
            target: request.map(|request| request.target),
            generation,
            token,
            baseline: self.cache.clone(),
            primary,
            primary_pane,
            deadlines,
        };
        if !self.start_pending_refresh(generation) {
            return None;
        }
        self.setup.last_token = token.0;
        self.setup.status = None;
        self.setup.preparation = Some(preparation.clone());
        self.setup.draft = Some(SetupDraft {
            tab_configuration: None,
            project_rename: None,
            token,
            baseline: self.cache.clone(),
            stage: NativeSetupStage::PreparingWorkspace,
            selected: 0,
            project: None,
            known_project_created: false,
            editor: LineEditor::new(String::new(), LengthLimit::Scalars(512)),
            shown: false,
            complete: true,
            faulted: false,
            quick: None,
            continuation: None,
        });
        Some(preparation)
    }
    pub(in crate::terminal_session) fn take_quick_preparation(
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
    pub(in crate::terminal_session) fn finish_quick_preparation(
        &mut self,
        preparation: &QuickPreparation,
        cwd: Option<WorkbenchDirectoryPath>,
        locale: Locale,
    ) {
        if self.cache != preparation.baseline
            || !self.setup_authority_available()
            || preparation
                .target
                .as_ref()
                .is_some_and(|target| !target.matches(self))
        {
            self.setup.status = Some(super::NativeSetupStatus::NotApplied);
            return;
        }
        let Some(cwd) = cwd else {
            self.setup.status = Some(super::NativeSetupStatus::NotApplied);
            return;
        };
        let Some(next) = self.setup.last_token.checked_add(1) else {
            return;
        };
        let fields = match &preparation.target {
            Some(target) => QuickWorkspaceFields::with_project(
                QuickProject::Targeted(target.clone()),
                cwd,
                locale,
            ),
            None => QuickWorkspaceFields::new(&self.cache, cwd, locale),
        };
        let stage = if fields.project() == &QuickProject::Choose {
            NativeSetupStage::QuickProject
        } else {
            NativeSetupStage::QuickName
        };
        let project = match fields.project() {
            QuickProject::Existing(id) => Some(id.clone()),
            QuickProject::Targeted(target) => Some(target.project.project_id.clone()),
            _ => None,
        };
        self.setup.last_token = next;
        self.setup.draft = Some(SetupDraft {
            tab_configuration: None,
            project_rename: None,
            token: NativeSetupToken(next),
            baseline: self.cache.clone(),
            stage,
            selected: 0,
            project,
            known_project_created: false,
            editor: LineEditor::new(String::new(), LengthLimit::Scalars(512)),
            shown: false,
            complete: true,
            faulted: false,
            quick: Some(fields),
            continuation: None,
        });
    }
    pub(in crate::terminal_session) fn quick_continuation(
        &self,
    ) -> Option<(NativeSetupToken, OperationDeadlines)> {
        let draft = self.setup.draft.as_ref()?;
        Some((draft.token, draft.continuation?))
    }
    pub(in crate::terminal_session) fn abort_quick_continuation(
        &mut self,
        token: NativeSetupToken,
    ) {
        if self
            .setup
            .draft
            .as_ref()
            .is_some_and(|draft| draft.token == token && draft.continuation.is_some())
        {
            self.setup.draft = None;
            self.setup.status = Some(super::NativeSetupStatus::ProjectCreated);
        }
    }
}

pub(crate) struct QuickRecovery<'a> {
    pub(crate) project: Option<&'a str>,
    pub(crate) workspace: Option<&'a str>,
    pub(crate) unknown: Option<&'a str>,
}
