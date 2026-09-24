use super::{
    CommitPlan, PlanFailure, WorkbenchAuthority, WorkbenchGrant, allocate, identity, result,
    unavailable,
};
use orchbee_workbench_protocol::{
    ContractError, Directory, MAXIMUM_DIRECTORIES_PER_PROJECT, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchDirectoryId, WorkbenchProtocolDocument, WorkbenchRejectionReason, WorkerId,
    serialize_document,
};

impl WorkbenchAuthority {
    /// `None` admits a read-only Worker probe, without retaining a request or consuming IDs.
    /// The Site must release its owner gates for that probe and repeat admission at commit.
    #[must_use]
    pub fn preflight_workspace_directory(
        &self,
        grant: Option<&WorkbenchGrant>,
        command: &WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        if !grant.is_some_and(|grant| {
            grant.site_id == *self.site_id() && grant.mutate_navigation && grant.associate_directory
        }) {
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
        self.directory_target(command).err().map(|failure| {
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

    /// Commits the pure canonical transition. The Site caller must hold registry then Site
    /// gates and establish that its successful Worker probe belongs to the current generation.
    #[must_use]
    pub fn accept_workspace_directory(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
        worker_id: WorkerId,
    ) -> WorkbenchCommandResult {
        if let Some(response) = self.preflight_workspace_directory(grant, &command) {
            return response;
        }
        let plan = match self.directory_plan(&command, worker_id) {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => return result(&command, self.rejected(reason)),
            Err(PlanFailure::Internal) => return self.retire(&command),
        };
        match self.prepare_command_commit(command, plan) {
            Ok(prepared) => prepared.commit(),
            Err(response) => *response,
        }
    }

    fn directory_target(&self, command: &WorkbenchCommand) -> Result<(usize, usize), PlanFailure> {
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            project_id,
            workspace_id,
            ..
        } = &command.action
        else {
            return Err(PlanFailure::Internal);
        };
        let snapshot = self.projection.snapshot();
        if command.site_id != snapshot.site_id {
            return super::reject(WorkbenchRejectionReason::SiteMismatch);
        }
        if command.expected_revision != snapshot.revision {
            return super::reject(WorkbenchRejectionReason::RevisionMismatch);
        }
        let (project_index, project) = snapshot
            .navigation
            .projects
            .iter()
            .enumerate()
            .find(|(_, project)| &project.project_id == project_id)
            .ok_or(PlanFailure::Rejected(
                WorkbenchRejectionReason::ProjectUnavailable,
            ))?;
        let (workspace_index, workspace) = project
            .workspaces
            .iter()
            .enumerate()
            .find(|(_, workspace)| &workspace.workspace_id == workspace_id)
            .ok_or(PlanFailure::Rejected(
                WorkbenchRejectionReason::WorkspaceUnavailable,
            ))?;
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == workspace.terminal_context_id)
            .ok_or(PlanFailure::Internal)?;
        if workspace.primary_directory_id.is_some()
            || !context.tabs.is_empty()
            || !context.groups.is_empty()
        {
            return super::reject(WorkbenchRejectionReason::WorkspaceDirectoryConflict);
        }
        if project.directories.len() >= MAXIMUM_DIRECTORIES_PER_PROJECT {
            return super::reject(WorkbenchRejectionReason::ResourceLimit);
        }
        allocate(self.last_identity, 1)?;
        self.next_revision()?;
        Ok((project_index, workspace_index))
    }

    fn directory_plan(
        &self,
        command: &WorkbenchCommand,
        worker_id: WorkerId,
    ) -> Result<CommitPlan, PlanFailure> {
        let (project_index, workspace_index) = self.directory_target(command)?;
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } = &command.action else {
            return Err(PlanFailure::Internal);
        };
        let last_identity = allocate(self.last_identity, 1)?;
        let directory_id = WorkbenchDirectoryId::parse(&identity(last_identity))
            .map_err(|_error| PlanFailure::Internal)?;
        let revision = self.next_revision()?;
        let mut candidate = self.projection.snapshot().clone();
        let project = candidate
            .navigation
            .projects
            .get_mut(project_index)
            .ok_or(PlanFailure::Internal)?;
        project
            .workspaces
            .get_mut(workspace_index)
            .ok_or(PlanFailure::Internal)?
            .primary_directory_id = Some(directory_id.clone());
        project.directories.push(Directory {
            directory_id: directory_id.clone(),
            worker_id: worker_id.clone(),
            path: path.clone(),
        });
        candidate.revision = revision;
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).map_err(
            |error| match error {
                ContractError::EncodedValueTooLarge { .. } => {
                    PlanFailure::Rejected(WorkbenchRejectionReason::ResourceLimit)
                }
                _ => PlanFailure::Internal,
            },
        )?;
        let projection = self
            .projection
            .prepare_advance(&encoded)
            .map_err(|_error| PlanFailure::Internal)?;
        Ok(CommitPlan {
            projection: Some(projection),
            last_identity: Some(last_identity),
            catalog_next: None,
            outcome: WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
                revision,
                directory_id,
                worker_id,
            },
        })
    }
}
