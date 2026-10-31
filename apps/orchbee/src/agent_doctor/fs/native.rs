//! The native observation port each platform adapter fills: physical observation functions and
//! the hook configuration targets they locate.
use super::{Budget, EnvValue, FileRead, Lstat, PathScan};
use crate::agent_doctor::probes::Provider;
use orchbee_agent_integration_writer::{
    HookConfigurationKind, HookConfigurationScan, RootPolicy, scan_hook_configuration,
};
use std::{
    ffi::{OsStr, OsString},
    fs::{DirEntry, ReadDir},
    io,
    path::{Path, PathBuf},
};

type ConfigurationTargetFactory =
    fn(&Provider, &Path, &'static [HookConfigurationKind]) -> Option<Vec<ConfigurationTarget>>;

pub(crate) struct ConfigurationTarget {
    pub(crate) path: PathBuf,
    pub(super) kind: HookConfigurationKind,
    pub(super) root_policy: RootPolicy,
}
impl ConfigurationTarget {
    pub(crate) fn scan(&self, bytes: &[u8]) -> HookConfigurationScan {
        scan_hook_configuration(self.kind, bytes, self.root_policy)
    }
}

/// Physical observations only; probe order, budgets and report decisions belong to the caller.
pub(super) struct Native {
    pub(super) lstat: fn(&Path) -> Lstat,
    pub(super) read_regular: fn(&Path, u64, &mut Budget) -> FileRead,
    pub(super) read_dir: fn(&Path) -> io::Result<ReadDir>,
    pub(super) reject_entry: fn(&DirEntry) -> bool,
    pub(super) env_value: fn(&str) -> EnvValue,
    pub(super) home_directory: fn() -> Option<PathBuf>,
    pub(super) inherited_path: fn() -> Option<OsString>,
    pub(super) current_executable: fn() -> io::Result<PathBuf>,
    pub(super) scan_path_for: fn(&str, &OsStr) -> PathScan,
    pub(super) endpoint_user: fn() -> Option<u32>,
    pub(super) codex_target: fn(&Provider, &Path) -> Option<PathBuf>,
    pub(super) guarded_target: fn(&Provider, &Path) -> Option<PathBuf>,
    pub(super) bootstrap_target: fn(&Provider, &Path) -> Option<PathBuf>,
    pub(super) configuration_targets: Option<ConfigurationTargetFactory>,
}
