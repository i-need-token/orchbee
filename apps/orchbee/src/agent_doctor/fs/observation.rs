//! An admitted native observation capability. Unsupported hosts cannot construct a probe port.
use super::native::{ConfigurationTarget, Native};
use super::{Budget, EnvValue, FileRead, Lstat, PathScan};
use crate::agent_doctor::probes::Provider;
use orchbee_agent_integration_writer::configuration_kinds;
use std::{
    ffi::{OsStr, OsString},
    fs::{DirEntry, ReadDir},
    io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct Unsupported;

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
    /// This environment's Capsule folder in its data root.
    pub(crate) fn capsule_folder(self) -> Option<PathBuf> {
        (self.0.capsule_folder)()
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
