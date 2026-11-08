//! Only startup adapters select the preferences record; wire requests cannot supply paths.
use crate::DataRoot;
use orchbee_workbench_protocol::PreferencesFailure;
use std::{ffi::OsString, path::Path, path::PathBuf};

/// The folder that holds one data root's preferences record, `internal/state/preferences.json`.
/// An explicit settings file does not move it; only another data root does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPreferencesScope {
    pub(super) directory: PathBuf,
    /// The root whose `internal/backups/` receives the record before a migration or reset.
    pub(super) root: DataRoot,
}
impl UserPreferencesScope {
    pub fn for_data_root(root: &DataRoot) -> Self {
        Self {
            directory: root.state(),
            root: root.clone(),
        }
    }

    /// The managed preferences directory is not an arbitrary user-provided wire operand.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn from_environment(
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, PreferencesFailure> {
        DataRoot::resolve(env)
            .map(|root| Self::for_data_root(&root))
            .map_err(|_| PreferencesFailure::Unavailable)
    }
}
