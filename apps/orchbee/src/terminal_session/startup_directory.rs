//! Empty-Site startup intent and retained invocation evidence. Server owns all commits.
use super::{
    NativeContextLocation, NativeMemberTarget,
    state::CarrierFailure,
    workbench_projection::{NativeContextTabs, project_native_context_tabs},
};
use crate::{
    local_client::workbench::workspace_launch::WorkspaceLaunchCommit, localization::Locale,
};
use orchbee_workbench_protocol::{
    SiteId, Snapshot, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProjectId,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchTerminalOutcome,
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Parameters {
    pub(super) cwd: WorkbenchDirectoryPath,
    pub(super) project_label: WorkbenchLabel,
    pub(super) workspace_label: WorkbenchLabel,
}
impl Parameters {
    fn from_path(path: &Path, locale: Locale) -> Option<Self> {
        let cwd = WorkbenchDirectoryPath::parse(path.to_str()?).ok()?;
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(locale.cli_default_workspace_label());
        Some(Self {
            cwd,
            project_label: WorkbenchLabel::parse(locale.cli_default_project_label()).ok()?,
            workspace_label: WorkbenchLabel::parse(&label.chars().take(512).collect::<String>())
                .ok()?,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StartupDirectory {
    parameters: Option<Parameters>,
    allowed: bool,
    report: Arc<Mutex<Progress>>,
}
impl PartialEq for StartupDirectory {
    fn eq(&self, other: &Self) -> bool {
        self.parameters == other.parameters
            && self.allowed == other.allowed
            && Arc::ptr_eq(&self.report, &other.report)
    }
}
impl Eq for StartupDirectory {}

#[derive(Clone, Debug)]
pub(super) struct KnownProject {
    pub(super) site: SiteId,
    pub(super) revision: WorkbenchRevision,
    pub(super) id: WorkbenchProjectId,
}
#[derive(Debug)]
pub(super) enum Progress {
    Initial,
    ProjectUnknown(WorkbenchRequestId),
    Project(KnownProject),
    WorkspaceUnknown {
        project: KnownProject,
        request: WorkbenchRequestId,
    },
    Workspace {
        project: KnownProject,
        commit: Arc<WorkspaceLaunchCommit>,
    },
}

impl StartupDirectory {
    pub(crate) fn new(path: Option<&Path>, locale: Locale, allowed: bool) -> Self {
        Self {
            parameters: path.and_then(|path| Parameters::from_path(path, locale)),
            allowed,
            report: Arc::new(Mutex::new(Progress::Initial)),
        }
    }
    pub(super) fn admits(&self, snapshot: &Snapshot, has_position: bool) -> bool {
        self.allowed
            && !has_position
            && snapshot.navigation.projects.is_empty()
            && snapshot
                .terminal_contexts
                .iter()
                .all(|context| context.tabs.is_empty())
    }
    pub(super) fn parameters(&self) -> Result<Parameters, CarrierFailure> {
        self.parameters.clone().ok_or(CarrierFailure::Transport)
    }
    pub(super) fn retain(&self, progress: Progress) -> Result<(), CarrierFailure> {
        *self.report.lock().map_err(|_| CarrierFailure::Protocol)? = progress;
        Ok(())
    }
    pub(crate) fn failure_note(&self, locale: Locale) -> std::io::Result<Option<String>> {
        let report = self
            .report
            .lock()
            .map_err(|_| std::io::Error::other("startup report poisoned"))?;
        let (project, workspace, unknown) = match &*report {
            Progress::Initial => return Ok(None),
            Progress::ProjectUnknown(request) => (None, None, Some(request.as_str())),
            Progress::Project(project) => (Some(project.id.as_str()), None, None),
            Progress::WorkspaceUnknown { project, request } => {
                (Some(project.id.as_str()), None, Some(request.as_str()))
            }
            Progress::Workspace { project, commit } => (
                Some(project.id.as_str()),
                Some(commit.workspace_id.as_str()),
                None,
            ),
        };
        Ok(Some(
            locale.directory_startup_failure(project, workspace, unknown),
        ))
    }
}

pub(super) fn validate_project(
    snapshot: &Snapshot,
    project: &KnownProject,
) -> Result<(), CarrierFailure> {
    if snapshot.site_id != project.site
        || snapshot.revision < project.revision
        || !snapshot
            .navigation
            .projects
            .iter()
            .any(|entry| entry.project_id == project.id)
    {
        return Err(CarrierFailure::AuthorityDivergence);
    }
    Ok(())
}

pub(super) fn validate_workspace(
    snapshot: &Snapshot,
    project: &KnownProject,
    commit: &WorkspaceLaunchCommit,
    parameters: &Parameters,
) -> Result<(NativeContextTabs, NativeMemberTarget), CarrierFailure> {
    if !matches!(
        commit.terminal_outcome,
        Some(WorkbenchTerminalOutcome::Started { .. })
    ) {
        return Err(CarrierFailure::AuthorityDivergence);
    }
    validate_project(snapshot, project)?;
    if snapshot.revision < commit.revision || commit.project_id != project.id {
        return Err(CarrierFailure::AuthorityDivergence);
    }
    let location = NativeContextLocation::Workspace {
        project_id: commit.project_id.clone(),
        workspace_id: commit.workspace_id.clone(),
        terminal_context_id: commit.terminal_context_id.clone(),
    };
    let cache = project_native_context_tabs(snapshot, &location)
        .map_err(|_| CarrierFailure::AuthorityDivergence)?;
    let directory = cache
        .directory()
        .ok_or(CarrierFailure::AuthorityDivergence)?;
    let tab = cache
        .tab_index(&commit.tab_id)
        .and_then(|index| cache.tabs().get(index))
        .ok_or(CarrierFailure::AuthorityDivergence)?;
    let panel = tab
        .panel_index(&commit.panel_id)
        .and_then(|index| tab.panel(index))
        .ok_or(CarrierFailure::AuthorityDivergence)?;
    let binding = panel
        .primary()
        .binding()
        .ok_or(CarrierFailure::AuthorityDivergence)?;
    if directory.directory_id != commit.directory_id
        || directory.path != parameters.cwd
        || &directory.worker_id != binding.worker_id()
        || panel.primary().presentation_id() != &commit.presentation_id
    {
        return Err(CarrierFailure::AuthorityDivergence);
    }
    let member = NativeMemberTarget {
        tab: commit.tab_id.clone(),
        panel: commit.panel_id.clone(),
        presentation: commit.presentation_id.clone(),
    };
    Ok((cache, member))
}

#[cfg(test)]
pub(in crate::terminal_session) mod tests;
