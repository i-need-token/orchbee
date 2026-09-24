//! Canonical Workspace checkout observations, fenced by Directory and Worker identity.
use super::worktree_operation::same_path;
use super::{WorkbenchAuthority, WorktreeOperationPlan, WorktreeSourceTarget};
use orchbee_workbench_protocol::WorkspaceGroupCloseUnavailable;
use orchbee_workbench_protocol::{
    WorkbenchProjectId, WorkbenchWorkspaceId, WorkerId, WorktreeCheckoutFacts,
};
use std::num::NonZeroU64;

pub(super) struct WorktreeGroupSelection {
    pub(super) members: Vec<(WorkbenchProjectId, WorkbenchWorkspaceId)>,
    pub(super) worker: Option<(WorkerId, NonZeroU64)>,
}

#[derive(Clone)]
pub(super) struct Association {
    pub(super) project_id: WorkbenchProjectId,
    pub(super) workspace_id: WorkbenchWorkspaceId,
    pub(super) directory: WorktreeSourceTarget,
    pub(super) worker_generation: NonZeroU64,
    pub(super) checkout: WorktreeCheckoutFacts,
}
impl WorkbenchAuthority {
    pub fn checkout_workspaces(
        &self,
        path: &orchbee_workbench_protocol::WorkbenchDirectoryPath,
        worker: &WorkerId,
    ) -> Vec<(WorkbenchProjectId, WorkbenchWorkspaceId)> {
        self.snapshot()
            .navigation
            .projects
            .iter()
            .flat_map(|project| {
                project.workspaces.iter().filter_map(|workspace| {
                    let directory = project.directories.iter().find(|directory| {
                        Some(&directory.directory_id) == workspace.primary_directory_id.as_ref()
                    })?;
                    (directory.worker_id == *worker
                        && same_path(directory.path.as_str(), path.as_str()))
                    .then(|| (project.project_id.clone(), workspace.workspace_id.clone()))
                })
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
        self.workspace_directory_target(&association.project_id, &association.workspace_id)
            .as_ref()
            == Some(&association.directory)
    }

    pub(super) fn workspace_directory_target(
        &self,
        project_id: &WorkbenchProjectId,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Option<WorktreeSourceTarget> {
        let project = self
            .snapshot()
            .navigation
            .projects
            .iter()
            .find(|project| project.project_id == *project_id)?;
        let workspace = project
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == *workspace_id)?;
        let directory = project.directories.iter().find(|directory| {
            Some(&directory.directory_id) == workspace.primary_directory_id.as_ref()
        })?;
        Some(WorktreeSourceTarget {
            directory_id: Some(directory.directory_id.clone()),
            path: directory.path.clone(),
            worker_id: directory.worker_id.clone(),
        })
    }

    /// Retains the actual source checkout, including a Workspace rooted in a checkout subdirectory.
    pub fn associate_worktree_source(
        &mut self,
        plan: &WorktreeOperationPlan,
        checkout: WorktreeCheckoutFacts,
    ) -> bool {
        if !self.worktree_source_matches(plan) {
            return false;
        }
        let Some(workspace_id) = &plan.source_workspace_id else {
            return true;
        };
        self.retain_worktree_association(Association {
            project_id: plan.project_id.clone(),
            workspace_id: workspace_id.clone(),
            directory: plan.source.clone(),
            worker_generation: plan.worker_generation,
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
        let Some(directory) = self.workspace_directory_target(&plan.project_id, workspace_id)
        else {
            return false;
        };
        if directory.worker_id != plan.source.worker_id
            || !same_path(directory.path.as_str(), checkout.path.as_str())
        {
            return false;
        }
        self.retain_worktree_association(Association {
            project_id: plan.project_id.clone(),
            workspace_id: workspace_id.clone(),
            directory,
            worker_generation: plan.worker_generation,
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

impl WorkbenchAuthority {
    pub(super) fn worktree_group_members(
        &self,
        project_id: &WorkbenchProjectId,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Result<WorktreeGroupSelection, WorkspaceGroupCloseUnavailable> {
        if !self.snapshot().navigation.projects.iter().any(|project| {
            project.project_id == *project_id
                && project
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.workspace_id == *workspace_id)
        }) {
            return Err(WorkspaceGroupCloseUnavailable::SourceUnavailable);
        }
        let singleton = || WorktreeGroupSelection {
            members: vec![(project_id.clone(), workspace_id.clone())],
            worker: None,
        };
        let Some(source) = self.workspace_directory_target(project_id, workspace_id) else {
            return Ok(singleton());
        };
        let known = self
            .worktrees
            .associations
            .iter()
            .filter(|entry| self.worktree_association_current(entry))
            .collect::<Vec<_>>();
        let mut selected: Option<(&Association, bool)> = None;
        for entry in &known {
            if source.worker_id != entry.directory.worker_id {
                continue;
            }
            let role = if entry.workspace_id == *workspace_id {
                Some(same_path(
                    entry.checkout.path.as_str(),
                    entry.checkout.repository_root.as_str(),
                ))
            } else if same_path(
                source.path.as_str(),
                entry.checkout.repository_root.as_str(),
            ) {
                Some(true)
            } else if same_path(source.path.as_str(), entry.checkout.path.as_str()) {
                Some(false)
            } else {
                None
            };
            if let Some(main) = role {
                if let Some((prior, prior_main)) = selected {
                    if !same_group(prior, entry) || prior_main != main {
                        return Err(WorkspaceGroupCloseUnavailable::AmbiguousMembership);
                    }
                } else {
                    selected = Some((entry, main));
                }
            }
        }
        let Some((source_group, main)) = selected else {
            return Ok(singleton());
        };
        let worker = Some((
            source_group.directory.worker_id.clone(),
            source_group.worker_generation,
        ));
        if !main {
            return Ok(WorktreeGroupSelection {
                worker,
                ..singleton()
            });
        }
        let mut members = vec![];
        for project in &self.snapshot().navigation.projects {
            for workspace in &project.workspaces {
                if workspace.workspace_id == *workspace_id {
                    continue;
                }
                let Some(directory) =
                    self.workspace_directory_target(&project.project_id, &workspace.workspace_id)
                else {
                    continue;
                };
                if directory.worker_id != source_group.directory.worker_id {
                    continue;
                }
                let own = known
                    .iter()
                    .find(|entry| entry.workspace_id == workspace.workspace_id);
                let matches_path = same_path(
                    directory.path.as_str(),
                    source_group.checkout.repository_root.as_str(),
                ) || known.iter().any(|entry| {
                    same_group(source_group, entry)
                        && same_path(directory.path.as_str(), entry.checkout.path.as_str())
                });
                if let Some(own) = own {
                    if !same_group(source_group, own) {
                        if matches_path {
                            return Err(WorkspaceGroupCloseUnavailable::AmbiguousMembership);
                        }
                        continue;
                    }
                } else if !matches_path {
                    continue;
                }
                members.push((project.project_id.clone(), workspace.workspace_id.clone()));
            }
        }
        members.push((project_id.clone(), workspace_id.clone()));
        Ok(WorktreeGroupSelection { members, worker })
    }
}
fn same_group(left: &Association, right: &Association) -> bool {
    left.directory.worker_id == right.directory.worker_id
        && left.worker_generation == right.worker_generation
        && same_path(
            left.checkout.common_directory.as_str(),
            right.checkout.common_directory.as_str(),
        )
        && same_path(
            left.checkout.repository_root.as_str(),
            right.checkout.repository_root.as_str(),
        )
}
