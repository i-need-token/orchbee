use super::{
    NativeContextLocation, NativeContextTabs, SetupKnownCommit, Snapshot, WorkbenchAction,
    WorkbenchSetupCommit, project_native_context_tabs, validate_canonical_refresh,
};

impl SetupKnownCommit {
    pub(super) fn target(&self) -> NativeContextLocation {
        match (&self.operation.command.action, &self.commit) {
            (
                WorkbenchAction::WorkspaceLaunch { project_id, .. },
                WorkbenchSetupCommit::WorkspaceLaunched(commit),
            ) => NativeContextLocation::Workspace {
                project_id: project_id.clone(),
                workspace_id: commit.workspace_id.clone(),
                terminal_context_id: commit.terminal_context_id.clone(),
            },
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
    pub(super) fn validate(&self, snapshot: &Snapshot) -> Option<NativeContextTabs> {
        let baseline = &self.operation.baseline;
        if snapshot.site_id != *baseline.site_id() || snapshot.revision < self.commit.revision() {
            return None;
        }
        if let WorkbenchSetupCommit::WorkspaceLaunched(commit) = &self.commit {
            let WorkbenchAction::WorkspaceLaunch {
                project_id,
                options,
                ..
            } = &self.operation.command.action
            else {
                return None;
            };
            if project_id != &commit.project_id
                || !matches!(
                    commit.terminal_outcome,
                    Some(orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. })
                )
            {
                return None;
            }
            if baseline.navigation().iter().any(|context| {
                context.location.terminal_context_id() == &commit.terminal_context_id
                    || matches!(
                        &context.location,
                        NativeContextLocation::Workspace { workspace_id, .. }
                            if workspace_id == &commit.workspace_id
                    )
            }) {
                return None;
            }
            let target = project_native_context_tabs(snapshot, &self.target()).ok()?;
            if snapshot.revision == commit.revision {
                let WorkbenchAction::WorkspaceLaunch { label, .. } = &self.operation.command.action
                else {
                    return None;
                };
                let workspace = snapshot
                    .navigation
                    .projects
                    .iter()
                    .find(|project| &project.project_id == project_id)?
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.workspace_id == commit.workspace_id)?;
                if workspace.label != label.as_str()
                    || target.tabs().len() != 1
                    || target.tabs()[0].panel_count() != 1
                {
                    return None;
                }
            }
            let directory = target.directory()?;
            if directory.directory_id != commit.directory_id
                || options.cwd.as_ref() != Some(&directory.path)
            {
                return None;
            }
            let tab = target
                .tabs()
                .iter()
                .find(|tab| tab.tab_id() == &commit.tab_id)?;
            let panel = tab.panel(tab.panel_index(&commit.panel_id)?)?;
            let primary = panel.primary();
            if primary.presentation_id() != &commit.presentation_id
                || primary.binding()?.worker_id() != &directory.worker_id
            {
                return None;
            }
            return Some(target);
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
            (
                WorkbenchAction::ProjectRename { project_id, label },
                WorkbenchSetupCommit::ProjectRenamed { .. },
            ) => {
                self.operation.validate_label_receipt(&self.commit)?;
                let project = snapshot
                    .navigation
                    .projects
                    .iter()
                    .find(|project| &project.project_id == project_id)?;
                if exact && project.label != label.as_str() {
                    return None;
                }
            }
            (
                WorkbenchAction::WorkspaceRename {
                    project_id,
                    workspace_id,
                    label,
                },
                WorkbenchSetupCommit::WorkspaceRenamed { .. },
            ) => {
                self.operation.validate_label_receipt(&self.commit)?;
                let project = snapshot
                    .navigation
                    .projects
                    .iter()
                    .find(|project| &project.project_id == project_id)?;
                let workspace = project
                    .workspaces
                    .iter()
                    .find(|workspace| &workspace.workspace_id == workspace_id)?;
                if &workspace.terminal_context_id != baseline.terminal_context_id()
                    || exact && workspace.label != label.as_str()
                {
                    return None;
                }
            }
            _ => return None,
        }
        if exact {
            if active.tabs() != baseline.tabs()
                || active.navigation().first().map(|entry| &entry.location)
                    != baseline.navigation().first().map(|entry| &entry.location)
            {
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
                (
                    WorkbenchAction::ProjectRename { project_id, .. },
                    WorkbenchSetupCommit::ProjectRenamed { .. },
                ) => {
                    projects
                        .iter_mut()
                        .find(|project| &project.project_id == project_id)?
                        .label = self.operation.original_location_label()?.to_owned();
                }
                (
                    WorkbenchAction::WorkspaceRename {
                        project_id,
                        workspace_id,
                        ..
                    },
                    WorkbenchSetupCommit::WorkspaceRenamed { .. },
                ) => {
                    projects
                        .iter_mut()
                        .find(|project| &project.project_id == project_id)?
                        .workspaces
                        .iter_mut()
                        .find(|workspace| &workspace.workspace_id == workspace_id)?
                        .label = self.operation.original_location_label()?.to_owned();
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
