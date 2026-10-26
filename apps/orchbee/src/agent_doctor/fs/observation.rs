//! An admitted native observation capability. Unsupported hosts cannot construct a probe port.
use super::{Budget, EnvValue, FileRead, Lstat, PathScan};
use crate::agent_doctor::probes::Provider;
use crate::agent_integrations::{
    HookConfigurationKind, HookConfigurationScan, RootPolicy, configuration_kinds,
    scan_hook_configuration,
};
use std::{
    ffi::{OsStr, OsString},
    fs::{DirEntry, ReadDir},
    io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct Unsupported;

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

#[derive(Clone, Copy)]
pub(crate) struct Observation(&'static Native);

impl Observation {
    pub(crate) fn admit() -> Result<Self, Unsupported> {
        #[cfg(windows)]
        {
            Ok(Self(&super::windows::NATIVE))
        }
        #[cfg(target_os = "linux")]
        {
            Ok(Self(&super::linux::NATIVE))
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Err(Unsupported)
        }
    }

    pub(crate) fn lstat(self, path: &Path) -> Lstat {
        (self.0.lstat)(path)
    }
    pub(super) fn read_regular(self, path: &Path, cap: u64, budget: &mut Budget) -> FileRead {
        (self.0.read_regular)(path, cap, budget)
    }
    pub(super) fn read_dir(self, path: &Path) -> io::Result<ReadDir> {
        (self.0.read_dir)(path)
    }
    pub(super) fn reject_entry(self, entry: &DirEntry) -> bool {
        (self.0.reject_entry)(entry)
    }
    pub(crate) fn env_value(self, name: &str) -> EnvValue {
        (self.0.env_value)(name)
    }
    pub(crate) fn home_directory(self) -> Option<PathBuf> {
        (self.0.home_directory)()
    }
    pub(crate) fn inherited_path(self) -> Option<OsString> {
        (self.0.inherited_path)()
    }
    pub(super) fn current_executable(self) -> io::Result<PathBuf> {
        (self.0.current_executable)()
    }
    pub(crate) fn scan_path_for(self, name: &str, path_env: &OsStr) -> PathScan {
        (self.0.scan_path_for)(name, path_env)
    }
    pub(crate) fn endpoint_user(self) -> Option<u32> {
        (self.0.endpoint_user)()
    }
    pub(crate) fn codex_target(self, provider: &Provider, home: &Path) -> Option<PathBuf> {
        (self.0.codex_target)(provider, home)
    }
    pub(crate) fn guarded_target(self, provider: &Provider, home: &Path) -> Option<PathBuf> {
        (self.0.guarded_target)(provider, home)
    }
    pub(crate) fn bootstrap_target(self, provider: &Provider, home: &Path) -> Option<PathBuf> {
        (self.0.bootstrap_target)(provider, home)
    }
    pub(crate) fn configuration_targets(
        self,
        provider: &Provider,
        home: &Path,
    ) -> Result<Option<Vec<ConfigurationTarget>>, Unsupported> {
        let factory = self.0.configuration_targets.ok_or(Unsupported)?;
        let Some(kinds) = configuration_kinds(provider.id) else {
            return Ok(None);
        };
        Ok(factory(provider, home, kinds))
    }
}
