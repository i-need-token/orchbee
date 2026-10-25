//! Admits a directory Workspace into the terminal creation commit boundary.
//! The Site caller owns the successful Worker probe and its generation fence.
use super::super::workspace_organization::PreparedPlacement;
use super::super::{
    LaunchProfileUse, PlanFailure, ProfileSelectorResolution, WorkbenchAuthority, WorkbenchGrant,
    allocate, empty_context, identity, resolve_profile_selector, result, unavailable,
};
use super::{
    StagedWorkbenchTabCreateMetadata, WorkbenchTabCreateAdmission, WorkbenchTabCreateBegin,
    WorkbenchTabCreateTarget, WorkspaceLaunchIds, WorkspaceLaunchPreparation, parse_tab_ids,
};
use orchbee_workbench_protocol::{
    Directory, LaunchProfileCatalogRevision, MAXIMUM_DIRECTORIES, MAXIMUM_TERMINAL_CONTEXTS,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchDirectoryId, WorkbenchRejectionReason,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId, Workspace,
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

    fn workspace_launch_target(
        &self,
        command: &WorkbenchCommand,
    ) -> Result<PreparedPlacement, PlanFailure> {
        let WorkbenchAction::WorkspaceLaunch {
            placement, options, ..
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
        let placement = self.prepare_workspace_placement(placement)?;
        if snapshot.navigation.directories.len() >= MAXIMUM_DIRECTORIES
            || snapshot.terminal_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS
        {
            return Err(reject(WorkbenchRejectionReason::ResourceLimit));
        }
        allocate(self.last_identity, 6 + placement.identity_count())?;
        self.next_revision()?;
        Ok(placement)
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
        let (Ok(placement), Ok(revision), Some(path)) = (
            self.workspace_launch_target(&command),
            self.next_revision(),
            options.cwd,
        ) else {
            return WorkbenchTabCreateBegin::Ready(self.retire(&command));
        };
        let mut candidate = self.projection.snapshot().clone();
        let mut last_identity = self.last_identity;
        let Ok(project_id) = placement.apply(&mut candidate.navigation, &mut last_identity) else {
            return WorkbenchTabCreateBegin::Ready(self.retire(&command));
        };
        let Ok(last_identity) = allocate(last_identity, 6) else {
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
        candidate.navigation.workspaces.push(Workspace {
            project_id: project_id.clone(),
            workspace_id: workspace_id.clone(),
            primary_directory_id: Some(directory_id.clone()),
            terminal_context_id: terminal_context_id.clone(),
            label: label.as_str().to_owned(),
        });
        candidate.navigation.directories.push(Directory {
            directory_id: directory_id.clone(),
            worker_id: worker_id.clone(),
            path: path.clone(),
        });
        super::super::workspace_organization::append_workspace_order(
            &mut candidate.navigation,
            &workspace_id,
        );
        candidate
            .terminal_contexts
            .push(empty_context(terminal_context_id.clone()));
        WorkbenchTabCreateBegin::Admitted(Box::new(WorkbenchTabCreateAdmission {
            authority: self,
            command,
            label: "1".to_owned(),
            staged: StagedWorkbenchTabCreateMetadata {
                revision,
                terminal_context_id,
                tab_id: ids.tab_id,
                panel_id: ids.panel_id,
                presentation_id: ids.presentation_id,
                geometry,
                target: WorkbenchTabCreateTarget { worker_id, path },
            },
            last_identity,
            profile_use,
            plugin_use: None,
            created_group: None,
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
