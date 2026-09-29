//! Admits a directory Workspace into the terminal creation commit boundary.
//! The Site caller owns the successful Worker probe and its generation fence.
use super::super::{
    LaunchProfileUse, PlanFailure, ProfileSelectorResolution, WorkbenchAuthority, WorkbenchGrant,
    allocate, empty_context, identity, resolve_profile_selector, result, unavailable,
};
use super::{
    StagedWorkbenchTabCreateMetadata, WorkbenchTabCreateAdmission, WorkbenchTabCreateBegin,
    WorkbenchTabCreateTarget, WorkspaceLaunchIds, WorkspaceLaunchPreparation, parse_tab_ids,
};
use orchbee_workbench_protocol::{
    Directory, LaunchProfileCatalogRevision, MAXIMUM_DIRECTORIES_PER_PROJECT,
    MAXIMUM_TERMINAL_CONTEXTS, MAXIMUM_WORKSPACES_PER_PROJECT, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchDirectoryId, WorkbenchRejectionReason, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId, Workspace,
};

impl WorkbenchAuthority {
    /// Admits only a read-only directory probe; consumes no IDs or ledger entry.
    pub fn preflight_workspace_launch(
        &self,
        grant: Option<&WorkbenchGrant>,
        command: &WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        if !matches!(command.action, WorkbenchAction::WorkspaceLaunch { .. })
            || !grant.is_some_and(|grant| {
                grant.site_id == *self.site_id()
                    && grant.mutate_navigation
                    && grant.associate_directory
                    && grant.create_terminal
            })
        {
            return Some(result(command, WorkbenchCommandOutcome::Denied {}));
        }
        if let Some(response) = self.command_preflight(command) {
            return Some(response);
        }
        if self.worktree_removal_active() {
            return Some(result(
                command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        self.workspace_launch_target(command).err().map(|failure| {
            result(
                command,
                match failure {
                    PlanFailure::Rejected(reason) => self.rejected(reason),
                    PlanFailure::Internal => {
                        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
                    }
                },
            )
        })
    }

    fn workspace_launch_target(&self, command: &WorkbenchCommand) -> Result<usize, PlanFailure> {
        let WorkbenchAction::WorkspaceLaunch {
            project_id,
            options,
            ..
        } = &command.action
        else {
            return Err(PlanFailure::Internal);
        };
        let snapshot = self.projection.snapshot();
        let reject = |reason| PlanFailure::Rejected(reason);
        if command.site_id != snapshot.site_id {
            return Err(reject(WorkbenchRejectionReason::SiteMismatch));
        }
        if command.expected_revision != snapshot.revision {
            return Err(reject(WorkbenchRejectionReason::RevisionMismatch));
        }
        if options.cwd.is_none() {
            return Err(PlanFailure::Internal);
        }
        let index = snapshot
            .navigation
            .projects
            .iter()
            .position(|project| &project.project_id == project_id)
            .ok_or(reject(WorkbenchRejectionReason::ProjectUnavailable))?;
        let project = &snapshot.navigation.projects[index];
        if project.workspaces.len() >= MAXIMUM_WORKSPACES_PER_PROJECT
            || project.directories.len() >= MAXIMUM_DIRECTORIES_PER_PROJECT
            || snapshot.terminal_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS
        {
            return Err(reject(WorkbenchRejectionReason::ResourceLimit));
        }
        allocate(self.last_identity, 6)?;
        self.next_revision()?;
        Ok(index)
    }

    /// Prepares the Workspace and first Tab together. Caller must hold registry then Site
    /// gates and prove a successful directory probe on the current Worker generation.
    pub fn begin_workspace_launch(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
        worker_id: WorkerId,
    ) -> WorkbenchTabCreateBegin<'_> {
        if let Some(response) = self.preflight_workspace_launch(grant, &command) {
            return WorkbenchTabCreateBegin::Ready(response);
        }
        let WorkbenchAction::WorkspaceLaunch {
            project_id,
            label,
            geometry,
            profile,
            options,
            ..
        } = command.action.clone()
        else {
            return WorkbenchTabCreateBegin::Ready(self.retire(&command));
        };
        let profile_use = match &profile {
            None => None,
            Some(selector) => match resolve_profile_selector(&self.catalog, selector) {
                ProfileSelectorResolution::Found(entry) => Some(LaunchProfileUse {
                    launch: entry.launch.clone(),
                    profile_id: entry.profile_id.clone(),
                    catalog_revision: LaunchProfileCatalogRevision::new(self.catalog_revision),
                }),
                ProfileSelectorResolution::NotFound => {
                    return WorkbenchTabCreateBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileUnavailable {},
                    ));
                }
                ProfileSelectorResolution::Ambiguous(candidate_ids) => {
                    return WorkbenchTabCreateBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids },
                    ));
                }
            },
        };
        let (Ok(index), Ok(last_identity), Ok(revision), Some(path)) = (
            self.workspace_launch_target(&command),
            allocate(self.last_identity, 6),
            self.next_revision(),
            options.cwd,
        ) else {
            return WorkbenchTabCreateBegin::Ready(self.retire(&command));
        };
        let (Ok(workspace_id), Ok(terminal_context_id), Ok(directory_id), Some(ids)) = (
            WorkbenchWorkspaceId::parse(&identity(last_identity - 5)),
            WorkbenchTerminalContextId::parse(&identity(last_identity - 4)),
            WorkbenchDirectoryId::parse(&identity(last_identity - 3)),
            parse_tab_ids(last_identity),
        ) else {
            return WorkbenchTabCreateBegin::Ready(self.retire(&command));
        };
        let mut candidate = self.projection.snapshot().clone();
        let project = &mut candidate.navigation.projects[index];
        project.workspaces.push(Workspace {
            workspace_id: workspace_id.clone(),
            primary_directory_id: Some(directory_id.clone()),
            terminal_context_id: terminal_context_id.clone(),
            label: label.as_str().to_owned(),
        });
        project.directories.push(Directory {
            directory_id: directory_id.clone(),
            worker_id: worker_id.clone(),
            path: path.clone(),
        });
        if !candidate.navigation.workspace_order.is_empty() {
            candidate
                .navigation
                .workspace_order
                .push(workspace_id.clone());
        }
        candidate
            .terminal_contexts
            .push(empty_context(terminal_context_id.clone()));
        WorkbenchTabCreateBegin::Admitted(Box::new(WorkbenchTabCreateAdmission {
            authority: self,
            command,
            label: label.as_str().to_owned(),
            staged: StagedWorkbenchTabCreateMetadata {
                revision,
                terminal_context_id,
                tab_id: ids.tab_id,
                panel_id: ids.panel_id,
                presentation_id: ids.presentation_id,
                geometry,
                target: WorkbenchTabCreateTarget::Workspace { worker_id, path },
            },
            last_identity,
            profile_use,
            plugin_use: None,
            workspace: Some(WorkspaceLaunchPreparation {
                candidate,
                ids: WorkspaceLaunchIds {
                    project_id,
                    workspace_id,
                    directory_id,
                },
            }),
        }))
    }
}
