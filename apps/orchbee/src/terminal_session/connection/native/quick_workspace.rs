//! Read-only source-directory preparation under the ordinary bounded refresh lease.
use super::{NativeExecutor, Result, WorkbenchSnapshotOutcome};
use crate::terminal_session::state::workspace_setup::quick::QuickPreparation;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

use super::source_directory::RootDirectory;

impl NativeExecutor {
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
        if let Some(binding) = &preparation.primary {
            match self.read_source_root(
                &preparation.baseline,
                preparation.primary_pane.as_ref(),
                preparation.deadlines,
                binding,
            )? {
                RootDirectory::Available(Some(path)) => return Ok(Some(path)),
                RootDirectory::Available(None) => {}
                RootDirectory::Unavailable => return Ok(None),
            }
        }
        if let Some(directory) = preparation.baseline.directory() {
            return Ok(Some(directory.path.clone()));
        }
        #[cfg(windows)]
        {
            Ok(orchbee_local_site_runtime::normalize_windows_user_path(
                "~",
                |name| std::env::var_os(name),
                std::env::current_dir,
            )
            .ok()
            .and_then(|path| WorkbenchDirectoryPath::parse(&path).ok()))
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    }
}
