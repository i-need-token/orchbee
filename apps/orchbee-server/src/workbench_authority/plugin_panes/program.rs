//! Resolves a plugin Pane's program with the platform executable search. Builds without that search
//! resolve nothing, so the caller reports that no compatible Worker can run the Pane.
use std::path::{Path, PathBuf};

/// The executable a Windows Worker runs for `program` relative to the plugin root.
#[cfg(windows)]
pub(super) fn resolve(program: &str, root: &Path) -> Option<PathBuf> {
    Some(orchbee_local_runtime::resolve_windows_program(
        program, root,
    ))
}

/// This build has no executable search for plugin Panes.
#[cfg(not(windows))]
pub(super) fn resolve(_program: &str, _root: &Path) -> Option<PathBuf> {
    None
}
