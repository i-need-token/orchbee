//! Resolve the caller against one validated snapshot before any Git submission.
use crate::{
    cli::{Failure, FailureKind, caller_context::PanelTarget, results, target_details},
    invocation::{Resource, Target, WorktreeStartCommand, WorktreeStartKind},
};
use orchbee_workbench_protocol::{
    Project, Snapshot, WorkbenchDirectoryPath, WorkbenchProjectId, WorktreeIntent, WorktreeSource,
};
use std::{ffi::OsString, path::PathBuf};

pub(super) fn intent(
    command: &WorktreeStartCommand,
    snapshot: &Snapshot,
    mut environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<(WorkbenchProjectId, WorktreeIntent), Failure> {
    let projects = &snapshot.navigation.projects;
    let project = command
        .project
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
    let workspace = if let Some(target) = &command.workspace {
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
    } else if command.cwd.is_none()
        && (environment("ORCHBEE_TERMINAL_ID").is_some()
            || environment("ORCHBEE_WORKER_ID").is_some())
    {
        let panel = PanelTarget::prepare(None, &mut environment)?.resolve(snapshot)?;
        let Target::Id(panel) = panel else {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        };
        let rows = results::terminal_rows(snapshot);
        let row = rows
            .iter()
            .find(|row| row.panel_id == panel)
            .ok_or_else(|| {
                Failure::new(FailureKind::NotFound).details(target_details(Resource::Panel, &panel))
            })?;
        let mut found = projects
            .iter()
            .flat_map(|p| p.workspaces.iter().map(move |w| (p, w)))
            .filter(|(_, w)| w.terminal_context_id.as_str() == row.terminal_context_id);
        let selected = found.next().ok_or_else(|| {
            Failure::new(FailureKind::NotFound).details(target_details(Resource::Workspace, ""))
        })?;
        if found.next().is_some() {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        if project.is_some_and(|p| p.project_id != selected.0.project_id) {
            return Err(Failure::new(FailureKind::NotFound).details(target_details(
                Resource::Workspace,
                selected.1.workspace_id.as_str(),
            )));
        }
        Some(selected)
    } else {
        None
    };
    let project = match (project, workspace) {
        (_, Some((project, _))) | (Some(project), None) => project,
        (None, None) => only_project(projects)?,
    };
    let source = match workspace {
        Some((_, workspace)) => WorktreeSource::Workspace {
            workspace_id: workspace.workspace_id.clone(),
        },
        None => WorktreeSource::Directory {
            path: match &command.cwd {
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
    let intent = match &command.kind {
        WorktreeStartKind::Create { branch, base, path } => WorktreeIntent::Create {
            source,
            branch: branch.clone(),
            base: base.clone(),
            path: path.clone(),
            label: command.label.clone(),
            geometry: command.geometry,
            trust_repository: command.trust_repository,
        },
        WorktreeStartKind::Open { selector } => WorktreeIntent::Open {
            source,
            selector: selector.clone(),
            label: command.label.clone(),
            geometry: command.geometry,
            trust_repository: command.trust_repository,
        },
        WorktreeStartKind::Remove { force } => WorktreeIntent::Remove {
            workspace_id: workspace
                .ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?
                .1
                .workspace_id
                .clone(),
            force: *force,
            trust_repository: command.trust_repository,
        },
    };
    Ok((project.project_id.clone(), intent))
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
