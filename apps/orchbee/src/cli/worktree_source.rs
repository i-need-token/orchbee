//! Shared source selection; a directory query never requires a Project.
use super::{Failure, FailureKind, results, target_details};
use crate::invocation::{Resource, Target};
use orchbee_workbench_protocol::{
    Project, Snapshot, WorkbenchDirectoryPath, WorkbenchProjectId, WorktreeSource,
};
use std::path::PathBuf;

#[cfg(windows)]
pub(super) mod windows;

pub(super) fn resolve(
    project_target: &Option<Target>,
    workspace_target: &Option<Target>,
    cwd_path: &Option<WorkbenchDirectoryPath>,
    snapshot: &Snapshot,
    require_project: bool,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<(Option<WorkbenchProjectId>, WorktreeSource), Failure> {
    let projects = &snapshot.navigation.projects;
    let project = project_target
        .as_ref()
        .map(|target| {
            results::resolve_or_fail(
                projects
                    .iter()
                    .map(|p| (p.project_id.as_str(), p.label.as_str(), p)),
                target,
                Resource::Project,
            )
        })
        .transpose()?;
    let workspace = if let Some(target) = workspace_target {
        Some(results::resolve_or_fail(
            projects
                .iter()
                .filter(|p| project.is_none_or(|selected| p.project_id == selected.project_id))
                .flat_map(|p| {
                    p.workspaces
                        .iter()
                        .map(move |w| (w.workspace_id.as_str(), w.label.as_str(), (p, w)))
                }),
            target,
            Resource::Workspace,
        )?)
    } else {
        None
    };
    let project = match (project, workspace) {
        (_, Some((project, _))) | (Some(project), None) => Some(project.project_id.clone()),
        (None, None) if require_project => Some(only_project(projects)?.project_id.clone()),
        (None, None) => None,
    };
    let source = match workspace {
        Some((_, workspace)) => WorktreeSource::Workspace {
            workspace_id: workspace.workspace_id.clone(),
        },
        None => WorktreeSource::Directory {
            path: match cwd_path {
                Some(path) => path.clone(),
                None => {
                    let path = cwd().map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
                    WorkbenchDirectoryPath::parse(
                        path.to_str()
                            .ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
                    )
                    .map_err(|_| Failure::new(FailureKind::InvalidUsage))?
                }
            },
        },
    };
    Ok((project, source))
}

fn only_project(projects: &[Project]) -> Result<&Project, Failure> {
    match projects {
        [project] => Ok(project),
        [] => {
            Err(Failure::new(FailureKind::NotFound).details(target_details(Resource::Project, "")))
        }
        _ => {
            let mut details = target_details(Resource::Project, "");
            details.insert(
                "candidates".to_owned(),
                serde_json::json!(
                    projects
                        .iter()
                        .map(|p| p.project_id.as_str())
                        .collect::<Vec<_>>()
                ),
            );
            Err(Failure::new(FailureKind::Ambiguous).details(details))
        }
    }
}
