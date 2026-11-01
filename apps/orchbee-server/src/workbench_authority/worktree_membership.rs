//! Canonical Workspace checkout observations, fenced by Directory and Worker identity.
use super::worktree_operation::same_path;
use super::{WorkbenchAuthority, WorktreeOperationPlan, WorktreeSourceTarget};
use orchbee_workbench_protocol::{WorkbenchWorkspaceId, WorkerId, WorktreeCheckoutFacts};

#[derive(Clone)]
pub(super) struct Association {
    pub(super) workspace_id: WorkbenchWorkspaceId,
    pub(super) directory: WorktreeSourceTarget,
    pub(super) checkout: WorktreeCheckoutFacts,
}
impl WorkbenchAuthority {
    pub fn checkout_workspaces(
        &self,
        path: &orchbee_workbench_protocol::WorkbenchDirectoryPath,
        worker: &WorkerId,
    ) -> Vec<WorkbenchWorkspaceId> {
        let navigation = &self.snapshot().navigation;
        navigation
            .workspaces
            .iter()
            .filter_map(|workspace| {
                let directory = navigation.workspace_directory(&workspace.workspace_id)?;
                (directory.worker_id == *worker
                    && same_path(directory.path.as_str(), path.as_str()))
                .then(|| workspace.workspace_id.clone())
            })
            .collect()
    }

    /// Returns retained Git facts only while the exact observed Directory still belongs to the Workspace.
    pub fn worktree_checkout(
        &self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Option<WorktreeCheckoutFacts> {
        self.worktrees
            .associations
            .iter()
            .find(|entry| {
                entry.workspace_id == *workspace_id && self.worktree_association_current(entry)
            })
            .map(|entry| entry.checkout.clone())
    }

    pub(super) fn worktree_association_current(&self, association: &Association) -> bool {
        self.workspace_directory_target(&association.workspace_id)
            .as_ref()
            == Some(&association.directory)
    }

    pub(super) fn workspace_directory_target(
        &self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Option<WorktreeSourceTarget> {
        let directory = self
            .snapshot()
            .navigation
            .workspace_directory(workspace_id)?;
        Some(WorktreeSourceTarget {
            directory_id: Some(directory.directory_id.clone()),
            path: directory.path.clone(),
            worker_id: directory.worker_id.clone(),
        })
    }

    /// Retains the actual source checkout, including a Workspace rooted in a checkout subdirectory.
    /// A directory the first terminal merely stands in says nothing about the Workspace's own.
    pub fn associate_worktree_source(
        &mut self,
        plan: &WorktreeOperationPlan,
        checkout: WorktreeCheckoutFacts,
    ) -> bool {
        if !self.worktree_source_matches(plan) {
            return false;
        }
        let (Some(workspace_id), Some(_)) = (&plan.source_workspace_id, &plan.source.directory_id)
        else {
            return true;
        };
        self.retain_worktree_association(Association {
            workspace_id: workspace_id.clone(),
            directory: plan.source.clone(),
            checkout,
        });
        true
    }

    /// Retains a target checkout after proving its canonical Workspace Directory and Worker.
    pub fn associate_worktree(
        &mut self,
        plan: &WorktreeOperationPlan,
        workspace_id: &WorkbenchWorkspaceId,
        checkout: WorktreeCheckoutFacts,
    ) -> bool {
        let Some(directory) = self.workspace_directory_target(workspace_id) else {
            return false;
        };
        if directory.worker_id != plan.source.worker_id
            || !same_path(directory.path.as_str(), checkout.path.as_str())
        {
            return false;
        }
        self.retain_worktree_association(Association {
            workspace_id: workspace_id.clone(),
            directory,
            checkout,
        });
        true
    }

    fn retain_worktree_association(&mut self, association: Association) {
        let mut current = std::mem::take(&mut self.worktrees.associations);
        current.retain(|entry| {
            entry.workspace_id != association.workspace_id
                && self.worktree_association_current(entry)
        });
        current.push(association);
        self.worktrees.associations = current;
    }
}
