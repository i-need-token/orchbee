//! Quick Workspace directory defaults under the ordinary bounded refresh lease.
use super::{ClientExecutor, Result, WorkbenchSnapshotOutcome};
use crate::workbench_state::workspace_setup::quick::QuickPreparation;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

impl ClientExecutor {
    /// A folder chosen in the menu, else the current directory of the focused Workspace's first
    /// terminal, else that Workspace's own directory, otherwise the directory this Client was
    /// launched from.
    pub(super) fn prepare_quick_cwd(
        &mut self,
        preparation: &QuickPreparation,
        outcome: &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorkbenchDirectoryPath>> {
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Ok(None);
        };
        if preparation
            .baseline
            .project_snapshot(snapshot)
            .ok()
            .as_ref()
            != Some(&preparation.baseline)
        {
            return Ok(None);
        }
        if let Some(directory) = preparation
            .target
            .as_ref()
            .and_then(|target| target.directory())
        {
            return Ok(Some(directory.clone()));
        }
        if let Some(binding) = &preparation.first_terminal
            && let super::source_directory::RootDirectory::Available(Some(path)) = self
                .read_source_root(
                    &preparation.baseline,
                    preparation.first_terminal_pane.as_ref(),
                    preparation.deadlines,
                    binding,
                )?
        {
            return Ok(Some(without_trailing_separator(path)));
        }
        if let Some(directory) = preparation.baseline.directory() {
            return Ok(Some(directory.path.clone()));
        }
        launch_directory().map_err(|_| super::CarrierFailure::Transport)
    }
}

/// A process reports its directory with a trailing separator, which would otherwise become part
/// of the new Workspace's directory; a drive root keeps its own.
fn without_trailing_separator(path: WorkbenchDirectoryPath) -> WorkbenchDirectoryPath {
    let trimmed = path.as_str().trim_end_matches(['/', '\\']);
    if trimmed.len() == path.as_str().len() || trimmed.is_empty() || trimmed.ends_with(':') {
        return path;
    }
    WorkbenchDirectoryPath::parse(trimmed).unwrap_or(path)
}

#[cfg(windows)]
fn launch_directory() -> std::io::Result<Option<WorkbenchDirectoryPath>> {
    // The Client never changes its process directory, so it remains the launch directory.
    Ok(orchbee_local_runtime::normalize_user_path(
        ".",
        |name| std::env::var_os(name),
        std::env::current_dir,
    )
    .ok()
    .and_then(|path| WorkbenchDirectoryPath::parse(&path).ok()))
}

#[cfg(not(windows))]
fn launch_directory() -> std::io::Result<Option<WorkbenchDirectoryPath>> {
    Err(
        crate::host_capabilities::Unsupported(crate::host_capabilities::Capability::DirectoryInput)
            .into(),
    )
}
