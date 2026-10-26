//! Source Workspace identity is global; only a Directory source may select a Project.
use super::{Failure, FailureKind, results};
use crate::invocation::Target;
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchDirectoryPath, WorkbenchProjectId, WorktreeSource,
};
use std::path::PathBuf;

pub(super) mod workflow;

pub(super) fn resolve(
    project_target: &Option<Target>,
    workspace_target: &Option<Target>,
    cwd_path: &Option<WorkbenchDirectoryPath>,
    snapshot: &Snapshot,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<(Option<WorkbenchProjectId>, WorktreeSource), Failure> {
    if let Some(target) = workspace_target {
        if project_target.is_some() {
            return Err(Failure::new(FailureKind::InvalidUsage));
        }
        let workspace = results::resolve_workspace(snapshot, target)?;
        return Ok((
            None,
            WorktreeSource::Workspace {
                workspace_id: workspace.workspace_id.clone(),
            },
        ));
    }
    let project = project_target
        .as_ref()
        .map(|target| {
            results::resolve_project(snapshot, target).map(|project| project.project_id.clone())
        })
        .transpose()?;
    let path = match cwd_path {
        Some(path) => path.clone(),
        None => {
            let path = cwd().map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
            WorkbenchDirectoryPath::parse(
                path.to_str()
                    .ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?,
            )
            .map_err(|_| Failure::new(FailureKind::InvalidUsage))?
        }
    };
    Ok((project, WorktreeSource::Directory { path }))
}
