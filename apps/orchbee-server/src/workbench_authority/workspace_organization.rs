//! Project membership decisions at the existing canonical navigation commit boundary.
use super::{PlanFailure, WorkbenchAuthority, allocate, identity};
use orchbee_workbench_protocol::{
    MAXIMUM_PROJECTS, MAXIMUM_WORKSPACES_PER_PROJECT, Navigation, Project, WorkbenchLabel,
    WorkbenchProjectId, WorkbenchRejectionReason, WorkbenchWorkspaceId, WorkspacePlacement,
};

/// New writes materialize display order even when an older snapshot used implicit order.
/// The caller has already appended the Workspace to canonical membership.
pub(super) fn append_workspace_order(
    navigation: &mut Navigation,
    workspace_id: &WorkbenchWorkspaceId,
) {
    if navigation.workspace_order.is_empty() {
        navigation.workspace_order = navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect();
    } else {
        navigation.workspace_order.push(workspace_id.clone());
    }
}

pub(super) enum PreparedPlacement {
    Existing(Option<WorkbenchProjectId>),
    Create {
        label: WorkbenchLabel,
        source: Option<WorkbenchWorkspaceId>,
    },
}

impl PreparedPlacement {
    pub(super) fn identity_count(&self) -> u128 {
        u128::from(matches!(self, Self::Create { .. }))
    }

    /// Mutates only the unpublished candidate; the caller commits this with the new Workspace.
    pub(super) fn apply(
        self,
        navigation: &mut Navigation,
        last_identity: &mut u128,
    ) -> Result<Option<WorkbenchProjectId>, PlanFailure> {
        match self {
            Self::Existing(project_id) => Ok(project_id),
            Self::Create { label, source } => {
                *last_identity = allocate(*last_identity, 1)?;
                let project_id = WorkbenchProjectId::parse(&identity(*last_identity))
                    .map_err(|_| PlanFailure::Internal)?;
                if !navigation.project_order.is_empty() {
                    navigation.project_order.push(project_id.clone());
                }
                navigation.projects.push(Project {
                    project_id: project_id.clone(),
                    label: label.as_str().to_owned(),
                });
                if let Some(source) = source {
                    navigation
                        .workspaces
                        .iter_mut()
                        .find(|workspace| workspace.workspace_id == source)
                        .ok_or(PlanFailure::Internal)?
                        .project_id = Some(project_id.clone());
                }
                Ok(Some(project_id))
            }
        }
    }
}

pub(super) fn admit_project_member(
    navigation: &Navigation,
    project_id: Option<&WorkbenchProjectId>,
) -> Result<(), PlanFailure> {
    let Some(project_id) = project_id else {
        return Ok(());
    };
    if navigation.project(project_id).is_none() {
        return Err(PlanFailure::Rejected(
            WorkbenchRejectionReason::ProjectUnavailable,
        ));
    }
    if navigation.workspaces_in_project(Some(project_id)).count() >= MAXIMUM_WORKSPACES_PER_PROJECT
    {
        return Err(PlanFailure::Rejected(
            WorkbenchRejectionReason::ResourceLimit,
        ));
    }
    Ok(())
}

impl WorkbenchAuthority {
    pub(super) fn workspace_organization_busy(&self, workspace_id: &WorkbenchWorkspaceId) -> bool {
        self.snapshot()
            .navigation
            .workspace(workspace_id)
            .is_some_and(|workspace| {
                self.open_close_fences_context(&workspace.terminal_context_id)
                    || self.workspace_group_close_contains(workspace_id)
            })
    }

    pub(super) fn prepare_workspace_placement(
        &self,
        placement: &WorkspacePlacement,
    ) -> Result<PreparedPlacement, PlanFailure> {
        let navigation = &self.snapshot().navigation;
        let result = match placement {
            WorkspacePlacement::Ungrouped {} => PreparedPlacement::Existing(None),
            WorkspacePlacement::Project { project_id } => {
                admit_project_member(navigation, Some(project_id))?;
                PreparedPlacement::Existing(Some(project_id.clone()))
            }
            WorkspacePlacement::NewProject { label } => PreparedPlacement::Create {
                label: label.clone(),
                source: None,
            },
            WorkspacePlacement::Alongside {
                workspace_id,
                project_label,
            } => {
                let source = navigation
                    .workspace(workspace_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::WorkspaceUnavailable,
                    ))?;
                if self.workspace_organization_busy(workspace_id) {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::WorkspaceUnavailable,
                    ));
                }
                if let Some(project_id) = &source.project_id {
                    admit_project_member(navigation, Some(project_id))?;
                    PreparedPlacement::Existing(Some(project_id.clone()))
                } else {
                    PreparedPlacement::Create {
                        label: project_label.clone(),
                        source: Some(workspace_id.clone()),
                    }
                }
            }
        };
        if matches!(result, PreparedPlacement::Create { .. })
            && navigation.projects.len() >= MAXIMUM_PROJECTS
        {
            return Err(PlanFailure::Rejected(
                WorkbenchRejectionReason::ResourceLimit,
            ));
        }
        Ok(result)
    }
}
