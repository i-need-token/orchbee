//! Resolve the caller against one validated snapshot before any Git submission.
use crate::{
    cli::{Failure, FailureKind},
    invocation::{WorktreeStartCommand, WorktreeStartKind},
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchProjectId, WorktreeIntent, WorktreeSource};
use std::{ffi::OsString, path::PathBuf};

pub(super) fn intent(
    command: &WorktreeStartCommand,
    snapshot: &Snapshot,
    environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<(WorkbenchProjectId, WorktreeIntent), Failure> {
    let (project, source) = crate::cli::worktree_source::resolve(
        &command.project,
        &command.workspace,
        &command.cwd,
        snapshot,
        true,
        environment,
        cwd,
    )?;
    let project = project.ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
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
