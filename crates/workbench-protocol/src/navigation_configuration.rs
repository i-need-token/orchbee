//! Atomic organization and display order, shared by authority and receipt verification.
//! Allocation, revision CAS, persistence, publication and terminal effects stay with callers.
use crate::{
    MAXIMUM_WORKSPACES_PER_PROJECT, Navigation, WorkbenchProjectId, WorkbenchRejectionReason,
    WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NavigationConfiguration {
    MoveProject {
        project_id: WorkbenchProjectId,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        before_project_id: Option<WorkbenchProjectId>,
    },
    MoveWorkspace {
        workspace_id: WorkbenchWorkspaceId,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        project_id: Option<WorkbenchProjectId>,
        #[serde(deserialize_with = "Deserialize::deserialize")]
        before_workspace_id: Option<WorkbenchWorkspaceId>,
    },
}

/// Returns no candidate for a semantic no-op. Every rejection leaves the source unchanged.
pub fn configure_navigation(
    navigation: &Navigation,
    operation: &NavigationConfiguration,
) -> Result<Option<Navigation>, WorkbenchRejectionReason> {
    use WorkbenchRejectionReason as Rejection;
    crate::validation::validate_navigation_structure(navigation)
        .map_err(|_| Rejection::InvalidNavigationConfiguration)?;
    let mut candidate = navigation.clone();
    match operation {
        NavigationConfiguration::MoveProject {
            project_id,
            before_project_id,
        } => {
            let original = navigation
                .ordered_projects()
                .map_err(|_| Rejection::InvalidNavigationConfiguration)?
                .into_iter()
                .map(|project| project.project_id.clone())
                .collect::<Vec<_>>();
            if !original.contains(project_id)
                || before_project_id
                    .as_ref()
                    .is_some_and(|id| !original.contains(id))
            {
                return Err(Rejection::ProjectUnavailable);
            }
            let ordered = moved_before(&original, project_id, before_project_id.as_ref())?;
            if ordered != original {
                candidate.project_order = ordered;
            }
        }
        NavigationConfiguration::MoveWorkspace {
            workspace_id,
            project_id,
            before_workspace_id,
        } => {
            let source = navigation
                .workspace(workspace_id)
                .ok_or(Rejection::WorkspaceUnavailable)?;
            if let Some(project) = project_id {
                if navigation.project(project).is_none() {
                    return Err(Rejection::ProjectUnavailable);
                }
                if source.project_id.as_ref() != Some(project)
                    && navigation.workspaces_in_project(Some(project)).count()
                        >= MAXIMUM_WORKSPACES_PER_PROJECT
                {
                    return Err(Rejection::ResourceLimit);
                }
            }
            if let Some(before) = before_workspace_id {
                let anchor = navigation
                    .workspace(before)
                    .ok_or(Rejection::WorkspaceUnavailable)?;
                if anchor.project_id != *project_id {
                    return Err(Rejection::InvalidNavigationConfiguration);
                }
            }
            let original = navigation
                .ordered_workspaces()
                .map_err(|_| Rejection::InvalidNavigationConfiguration)?
                .into_iter()
                .map(|(_, workspace)| workspace.workspace_id.clone())
                .collect::<Vec<_>>();
            candidate
                .workspaces
                .iter_mut()
                .find(|workspace| &workspace.workspace_id == workspace_id)
                .ok_or(Rejection::WorkspaceUnavailable)?
                .project_id = project_id.clone();
            // Refill only destination-group slots. Other groups retain their exact positions.
            let destination_ids = candidate
                .workspaces
                .iter()
                .filter(|workspace| workspace.project_id == *project_id)
                .map(|workspace| workspace.workspace_id.clone())
                .collect::<std::collections::HashSet<_>>();
            let destination = original
                .iter()
                .filter(|id| destination_ids.contains(*id))
                .cloned()
                .collect::<Vec<_>>();
            let ordered = moved_before(&destination, workspace_id, before_workspace_id.as_ref())?;
            let mut members = ordered.into_iter();
            let mut next = original.clone();
            for id in &mut next {
                if destination_ids.contains(id) {
                    *id = members
                        .next()
                        .ok_or(Rejection::InvalidNavigationConfiguration)?;
                }
            }
            if members.next().is_some() {
                return Err(Rejection::InvalidNavigationConfiguration);
            }
            if next != original {
                candidate.workspace_order = next;
            }
        }
    }
    crate::validation::validate_navigation_structure(&candidate)
        .map_err(|_| Rejection::InvalidNavigationConfiguration)?;
    Ok((candidate != *navigation).then_some(candidate))
}

fn moved_before<T: Clone + Eq>(
    original: &[T],
    source: &T,
    before: Option<&T>,
) -> Result<Vec<T>, WorkbenchRejectionReason> {
    if before == Some(source) {
        return Ok(original.to_vec());
    }
    let mut ordered = original
        .iter()
        .filter(|id| *id != source)
        .cloned()
        .collect::<Vec<_>>();
    if ordered.len() + 1 != original.len() {
        return Err(WorkbenchRejectionReason::InvalidNavigationConfiguration);
    }
    let index = match before {
        Some(id) => ordered
            .iter()
            .position(|candidate| candidate == id)
            .ok_or(WorkbenchRejectionReason::InvalidNavigationConfiguration)?,
        None => ordered.len(),
    };
    ordered.insert(index, source.clone());
    Ok(ordered)
}

#[cfg(test)]
mod tests;
