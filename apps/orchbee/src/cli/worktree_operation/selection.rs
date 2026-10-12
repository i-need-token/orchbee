//! Resolve canonical Project/source operands, then build the retained Git intent.
use crate::{
    cli::{Failure, FailureKind},
    invocation::{WorktreeOpenSelector, WorktreeStartCommand, WorktreeStartKind},
};
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchProjectId, WorktreeIntent, WorktreeSelector, WorktreeSource,
};
use std::path::PathBuf;

pub(super) fn intent(
    command: &WorktreeStartCommand,
    snapshot: &Snapshot,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<(Option<WorkbenchProjectId>, WorktreeIntent), Failure> {
    let (project, source) = crate::cli::worktree_source::resolve(
        &command.project,
        &command.workspace,
        &command.cwd,
        snapshot,
        cwd,
    )?;
    intent_from_source(command, project, source)
}

pub(super) fn intent_from_source(
    command: &WorktreeStartCommand,
    project: Option<WorkbenchProjectId>,
    source: WorktreeSource,
) -> Result<(Option<WorkbenchProjectId>, WorktreeIntent), Failure> {
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
            selector: match selector {
                WorktreeOpenSelector::Path { path } => {
                    WorktreeSelector::Path { path: path.clone() }
                }
                WorktreeOpenSelector::Branch { branch } => WorktreeSelector::Branch {
                    branch: branch.clone(),
                },
            },
            label: command.label.clone(),
            geometry: command.geometry,
            trust_repository: command.trust_repository,
        },
        WorktreeStartKind::Remove { force } => WorktreeIntent::Remove {
            workspace_id: match source {
                WorktreeSource::Workspace { workspace_id } => workspace_id,
                WorktreeSource::Directory { .. } => {
                    return Err(Failure::new(FailureKind::InvalidUsage));
                }
            },
            force: *force,
            trust_repository: command.trust_repository,
        },
    };
    Ok((project, intent))
}
