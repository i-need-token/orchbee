use super::{
    NativeContextLocation, NativeContextTabs, SetupKnownCommit, Snapshot, WorkbenchAction,
    WorkbenchSetupCommit, project_native_context_tabs, validate_canonical_refresh,
};

impl SetupKnownCommit {
    pub(super) fn target(&self) -> NativeContextLocation {
        match (&self.operation.command.action, &self.commit) {
            (
                WorkbenchAction::WorkspaceLaunch { .. },
                WorkbenchSetupCommit::WorkspaceLaunched(commit),
            ) => NativeContextLocation::Workspace {
                workspace_id: commit.workspace_id.clone(),
                terminal_context_id: commit.terminal_context_id.clone(),
            },
            (
                WorkbenchAction::WorkspaceCreate { .. },
                WorkbenchSetupCommit::WorkspaceCreated {
                    workspace_id,
                    terminal_context_id,
                    ..
                },
            ) => NativeContextLocation::Workspace {
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
        if matches!(
            self.operation.command.action,
            WorkbenchAction::NavigationConfigure { .. }
        ) {
            let navigation = self.operation.configured_navigation(&self.commit)?;
            if snapshot.revision == self.commit.revision() {
                let mut exact = baseline.canonical_snapshot().clone();
                exact.revision = self.commit.revision();
                exact.navigation = navigation;
                if snapshot != &exact {
                    return None;
                }
            }
            return validate_canonical_refresh(baseline, baseline.project_snapshot(snapshot).ok()?)
                .ok();
        }
        if matches!(
            self.operation.command.action,
            WorkbenchAction::TabsConfigure { .. }
        ) {
            let expected = self.operation.configured_context(&self.commit)?;
            if snapshot.revision == self.commit.revision() {
                let mut exact = baseline.canonical_snapshot().clone();
                exact.revision = self.commit.revision();
                let context = exact
                    .terminal_contexts
                    .iter_mut()
                    .find(|context| context.terminal_context_id == expected.terminal_context_id)?;
                *context = expected;
                if snapshot != &exact {
                    return None;
                }
            }
            let candidate = baseline.project_snapshot(snapshot).ok()?;
            return validate_canonical_refresh(baseline, candidate).ok();
        }
        if let WorkbenchSetupCommit::WorkspaceLaunched(commit) = &self.commit {
            return self.validate_launch(snapshot, commit);
        }
        let active =
            validate_canonical_refresh(baseline, baseline.project_snapshot(snapshot).ok()?).ok()?;
        let mut expected = baseline.canonical_snapshot().clone();
        expected.revision = self.commit.revision();
        let navigation = &mut expected.navigation;
        match (&self.operation.command.action, &self.commit) {
            (
                WorkbenchAction::WorkspaceSetProject {
                    workspace_id,
                    project_id,
                },
                WorkbenchSetupCommit::WorkspaceProjectChanged { .. },
            ) => {
                self.operation.organization_receipt(&self.commit)?;
                navigation
                    .workspaces
                    .iter_mut()
                    .find(|workspace| &workspace.workspace_id == workspace_id)?
                    .project_id = project_id.clone();
            }
            (
                WorkbenchAction::WorkspaceCreateProject {
                    workspace_id,
                    label,
                },
                WorkbenchSetupCommit::WorkspaceProjectChanged {
                    project_id: Some(project_id),
                    ..
                },
            ) => {
                self.operation.organization_receipt(&self.commit)?;
                if !navigation.project_order.is_empty() {
                    navigation.project_order.push(project_id.clone());
                }
                navigation
                    .projects
                    .push(orchbee_workbench_protocol::Project {
                        project_id: project_id.clone(),
                        label: label.as_str().to_owned(),
                    });
                navigation
                    .workspaces
                    .iter_mut()
                    .find(|workspace| &workspace.workspace_id == workspace_id)?
                    .project_id = Some(project_id.clone());
            }
            (
                WorkbenchAction::ProjectDissolve { project_id },
                WorkbenchSetupCommit::ProjectDissolved { .. },
            ) => {
                self.operation.organization_receipt(&self.commit)?;
                navigation
                    .projects
                    .retain(|project| &project.project_id != project_id);
                navigation.project_order.retain(|id| id != project_id);
                for workspace in &mut navigation.workspaces {
                    if workspace.project_id.as_ref() == Some(project_id) {
                        workspace.project_id = None;
                    }
                }
            }
            (
                WorkbenchAction::ProjectCreate { label },
                WorkbenchSetupCommit::ProjectCreated { project_id, .. },
            ) => {
                if navigation.project(project_id).is_some() {
                    return None;
                }
                snapshot.navigation.project(project_id)?;
                if !navigation.project_order.is_empty() {
                    navigation.project_order.push(project_id.clone());
                }
                navigation
                    .projects
                    .push(orchbee_workbench_protocol::Project {
                        project_id: project_id.clone(),
                        label: label.as_str().to_owned(),
                    });
            }
            (
                WorkbenchAction::WorkspaceCreate { project_id, label },
                WorkbenchSetupCommit::WorkspaceCreated {
                    workspace_id,
                    terminal_context_id,
                    ..
                },
            ) => {
                if navigation.workspace(workspace_id).is_some()
                    || expected
                        .terminal_contexts
                        .iter()
                        .any(|context| &context.terminal_context_id == terminal_context_id)
                {
                    return None;
                }
                let workspace = snapshot.navigation.workspace(workspace_id)?;
                if &workspace.terminal_context_id != terminal_context_id {
                    return None;
                }
                navigation
                    .workspaces
                    .push(orchbee_workbench_protocol::Workspace {
                        project_id: project_id.clone(),
                        label: label.as_str().to_owned(),
                        workspace_id: workspace_id.clone(),
                        terminal_context_id: terminal_context_id.clone(),
                        primary_directory_id: None,
                    });
                if navigation.workspace_order.is_empty() {
                    navigation.workspace_order = navigation
                        .workspaces
                        .iter()
                        .map(|workspace| workspace.workspace_id.clone())
                        .collect();
                } else {
                    navigation.workspace_order.push(workspace_id.clone());
                }
                expected
                    .terminal_contexts
                    .push(orchbee_workbench_protocol::TerminalContext {
                        terminal_context_id: terminal_context_id.clone(),
                        tabs: vec![],
                        groups: vec![],
                    });
            }
            (
                WorkbenchAction::WorkspacePrimaryDirectoryAccept { workspace_id, path },
                WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
                    directory_id,
                    worker_id,
                    ..
                },
            ) => {
                if navigation.directory(directory_id).is_some() {
                    return None;
                }
                let actual = snapshot.navigation.workspace_directory(workspace_id)?;
                if &actual.directory_id != directory_id
                    || &actual.worker_id != worker_id
                    || &actual.path != path
                {
                    return None;
                }
                navigation
                    .workspaces
                    .iter_mut()
                    .find(|workspace| &workspace.workspace_id == workspace_id)?
                    .primary_directory_id = Some(directory_id.clone());
                navigation.directories.push(actual.clone());
            }
            (
                WorkbenchAction::ProjectRename { project_id, label },
                WorkbenchSetupCommit::ProjectRenamed { .. },
            ) => {
                self.operation.validate_label_receipt(&self.commit)?;
                snapshot.navigation.project(project_id)?;
                navigation
                    .projects
                    .iter_mut()
                    .find(|project| &project.project_id == project_id)?
                    .label = label.as_str().to_owned();
            }
            (
                WorkbenchAction::WorkspaceRename {
                    workspace_id,
                    label,
                },
                WorkbenchSetupCommit::WorkspaceRenamed { .. },
            ) => {
                self.operation.validate_label_receipt(&self.commit)?;
                let workspace = snapshot.navigation.workspace(workspace_id)?;
                if &workspace.terminal_context_id != baseline.terminal_context_id() {
                    return None;
                }
                navigation
                    .workspaces
                    .iter_mut()
                    .find(|workspace| &workspace.workspace_id == workspace_id)?
                    .label = label.as_str().to_owned();
            }
            _ => return None,
        }
        if snapshot.revision == self.commit.revision() && snapshot != &expected {
            return None;
        }
        let _ = active;
        project_native_context_tabs(snapshot, &self.target()).ok()
    }

    fn validate_launch(
        &self,
        snapshot: &Snapshot,
        commit: &crate::local_client::workbench::workspace_launch::WorkspaceLaunchCommit,
    ) -> Option<NativeContextTabs> {
        use orchbee_workbench_protocol::{WorkbenchTerminalOutcome, WorkspacePlacement};
        let WorkbenchAction::WorkspaceLaunch {
            placement,
            options,
            label,
            ..
        } = &self.operation.command.action
        else {
            return None;
        };
        let before = self.operation.baseline.canonical_snapshot();
        if before.navigation.workspace(&commit.workspace_id).is_some()
            || before.navigation.directory(&commit.directory_id).is_some()
            || before
                .terminal_contexts
                .iter()
                .any(|context| context.terminal_context_id == commit.terminal_context_id)
        {
            return None;
        }
        let target = project_native_context_tabs(snapshot, &self.target()).ok()?;
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
            || primary
                .binding()
                .is_some_and(|binding| binding.worker_id() != &directory.worker_id)
            || (matches!(
                commit.terminal_outcome,
                Some(WorkbenchTerminalOutcome::Started { .. })
            ) && primary.binding().is_none())
        {
            return None;
        }
        if snapshot.revision == commit.revision {
            if target.tabs().len() != 1 || target.tabs()[0].panel_count() != 1 {
                return None;
            }
            let mut expected = before.clone();
            expected.revision = commit.revision;
            let navigation = &mut expected.navigation;
            let new_project = match placement {
                WorkspacePlacement::Ungrouped {} => {
                    if commit.project_id.is_some() {
                        return None;
                    }
                    None
                }
                WorkspacePlacement::Project { project_id } => {
                    if commit.project_id.as_ref() != Some(project_id) {
                        return None;
                    }
                    None
                }
                WorkspacePlacement::NewProject { label } => Some(label),
                WorkspacePlacement::Alongside {
                    workspace_id,
                    project_label,
                } => {
                    let source = navigation
                        .workspaces
                        .iter_mut()
                        .find(|workspace| &workspace.workspace_id == workspace_id)?;
                    if source.project_id.is_some() {
                        if source.project_id != commit.project_id {
                            return None;
                        }
                        None
                    } else {
                        source.project_id = commit.project_id.clone();
                        Some(project_label)
                    }
                }
            };
            if let Some(label) = new_project {
                let project_id = commit.project_id.as_ref()?;
                if navigation.project(project_id).is_some() {
                    return None;
                }
                if !navigation.project_order.is_empty() {
                    navigation.project_order.push(project_id.clone());
                }
                navigation
                    .projects
                    .push(orchbee_workbench_protocol::Project {
                        project_id: project_id.clone(),
                        label: label.as_str().to_owned(),
                    });
            }
            navigation.directories.push(directory.clone());
            navigation
                .workspaces
                .push(orchbee_workbench_protocol::Workspace {
                    workspace_id: commit.workspace_id.clone(),
                    project_id: commit.project_id.clone(),
                    label: label.as_str().to_owned(),
                    primary_directory_id: Some(commit.directory_id.clone()),
                    terminal_context_id: commit.terminal_context_id.clone(),
                });
            if navigation.workspace_order.is_empty() {
                navigation.workspace_order = navigation
                    .workspaces
                    .iter()
                    .map(|workspace| workspace.workspace_id.clone())
                    .collect();
            } else {
                navigation.workspace_order.push(commit.workspace_id.clone());
            }
            expected
                .terminal_contexts
                .push(target.canonical_context().clone());
            if snapshot != &expected {
                return None;
            }
        }
        Some(target)
    }
}
