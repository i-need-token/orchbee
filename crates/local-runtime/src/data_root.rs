//! The data root (ORC-397): the one folder per environment that holds everything Orchbee keeps,
//! `<home>/.<stem>`, or the folder `ORCHBEE_HOME` names. Its top level holds only what a person
//! reads, edits, or keeps; everything Orchbee manages for itself lives under `internal/`. Every
//! Orchbee state path derives from here, so callers never spell a folder or choose a home variable
//! themselves. Resolving touches no file; the platform adapters create private folders on first
//! write, and the Windows adapter records the root's layout. Every process that opens a root admits its layout first,
//! before any other read.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

mod version_record;
#[cfg(windows)]
pub(crate) use version_record::compare_product_versions;
pub use version_record::{LAYOUT, LayoutRefusal, PRODUCT_VERSION};
#[cfg(windows)]
mod windows;
#[cfg(all(test, windows))]
pub(crate) use windows::settle_version_record;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{create_private_directory, prepare_default_runtime_base};
#[cfg(windows)]
pub use windows::{create_private_directory, prepare_default_runtime_base};
#[cfg(windows)]
pub(crate) use windows::{create_state_directory, open_state_root};

/// The variable that names another data root for every environment that reads it.
pub const HOME_VARIABLE: &str = "ORCHBEE_HOME";
const CONFIG_FILE: &str = "config.toml";
const INTERNAL: &str = "internal";
/// The record name of the TUI position a Session keeps in its folder: `tui-position.json`, beside
/// its `tui-position.lock` and `tui-position.pending`.
pub const TUI_POSITION_RECORD: &str = "tui-position";

/// One environment's data root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataRoot {
    path: PathBuf,
}

/// Why the data root is unavailable: the variable that should supply it, and `missing` or
/// `invalid-path`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataRootError {
    pub variable: &'static str,
    pub reason: &'static str,
}

/// A device record under `internal/state/`. Each record is one JSON file whose writer lock and
/// staging file sit beside it, named after it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateRecord {
    /// Whether the person dismissed the TUI onboarding notice.
    Onboarding,
    /// The saved machine catalog.
    Machines,
}

impl StateRecord {
    /// The record's name: its file is `<name>.json`, and its lock and staging file are
    /// `<name>.lock` and `<name>.pending`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Onboarding => "onboarding",
            Self::Machines => "machines",
        }
    }

    /// The record's file name.
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Onboarding => "onboarding.json",
            Self::Machines => "machines.json",
        }
    }
}

/// The plugin store, `internal/plugins/`: the registry, the source a running install fetches, and
/// one folder per plugin holding its `config`, `data`, and `source/<version>` folders. The store
/// chooses each folder and version name; this type only places them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PluginStorePaths {
    root: PathBuf,
}

impl PluginStorePaths {
    /// The registry's record name: its file is `registry.json`, beside `registry.lock` and
    /// `registry.pending`.
    pub(crate) const REGISTRY_RECORD: &'static str = "registry";
    /// The registry file.
    pub(crate) const REGISTRY_FILE: &'static str = "registry.json";
    /// The source a running install fetches before its plugin id and version name its folder.
    pub(crate) const PENDING_SOURCE: &'static str = "source.pending";
    /// The folder of a plugin's managed sources, one per version.
    pub(crate) const SOURCES: &'static str = "source";

    /// The store at `root`, which is `internal/plugins/` of a data root unless a test keeps its
    /// own.
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn pending_source(&self) -> PathBuf {
        self.root.join(Self::PENDING_SOURCE)
    }

    /// One plugin's folder.
    pub(crate) fn plugin(&self, folder: &str) -> PathBuf {
        self.root.join(folder)
    }

    /// The configuration folder the plugin owns, the one folder in the store meant for hand
    /// editing.
    pub(crate) fn config(&self, folder: &str) -> PathBuf {
        self.plugin(folder).join("config")
    }

    /// The data folder the plugin owns.
    pub(crate) fn data(&self, folder: &str) -> PathBuf {
        self.plugin(folder).join("data")
    }

    /// The plugin's managed sources.
    pub(crate) fn sources(&self, folder: &str) -> PathBuf {
        self.plugin(folder).join(Self::SOURCES)
    }

    /// One managed source, the checkout of one version.
    pub(crate) fn source(&self, folder: &str, version: &str) -> PathBuf {
        self.sources(folder).join(version)
    }

    /// Whether `name` is one the store keeps for itself, so no plugin folder may take it. Letter
    /// case does not distinguish names on Windows.
    pub(crate) fn reserved(name: &str) -> bool {
        [
            Self::REGISTRY_FILE,
            "registry.lock",
            "registry.pending",
            Self::PENDING_SOURCE,
        ]
        .iter()
        .any(|reserved| name.eq_ignore_ascii_case(reserved))
    }
}

impl DataRoot {
    /// `ORCHBEE_HOME` when set, otherwise `<home>/.<stem>`. Home follows `USERPROFILE` on Windows,
    /// as Node's `os.homedir()` does so Desktop derives the same folder, and `HOME` elsewhere.
    /// Only `env` is consulted, so a caller that supplies its own environment never reaches the
    /// process's own home.
    pub fn resolve(env: impl Fn(&str) -> Option<OsString>) -> Result<Self, DataRootError> {
        if let Some(value) = env(HOME_VARIABLE) {
            return admit_absolute(&value, HOME_VARIABLE).map(|path| Self { path });
        }
        let variable = HOME_SOURCE;
        let home = env(variable)
            .filter(|value| !value.is_empty())
            .ok_or(DataRootError {
                variable,
                reason: "missing",
            })?;
        Ok(Self::under_home(&admit_absolute(&home, variable)?))
    }

    /// This environment's root below `home`, `<home>/.<stem>`, without admitting `home`; help
    /// passes a `%USERPROFILE%` reference to spell the default without reading the environment.
    pub fn under_home(home: &Path) -> Self {
        Self {
            path: home.join(format!(".{}", crate::environment_stem())),
        }
    }

    /// The variable that supplies the home folder the default root sits in.
    pub const fn home_variable() -> &'static str {
        HOME_SOURCE
    }

    /// The variable that supplies the root, as diagnostics and help name it.
    pub fn variable(env: impl Fn(&str) -> Option<OsString>) -> &'static str {
        if env(HOME_VARIABLE).is_some() {
            HOME_VARIABLE
        } else {
            HOME_SOURCE
        }
    }

    /// A root at an already admitted absolute folder, for an owner that received it from another
    /// process or a test that keeps its own.
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The default settings file, which `ORCHBEE_CONFIG_FILE` replaces without moving anything
    /// else.
    pub fn config_file(&self) -> PathBuf {
        self.path.join(CONFIG_FILE)
    }

    /// Detection rule overrides the person writes, `agent-detection/<provider>.toml`.
    pub fn agent_detection(&self) -> PathBuf {
        self.path.join("agent-detection")
    }

    /// One folder per Session, named by the Session in the spelling it was created with. Letter
    /// case does not distinguish Sessions, so the catalog finds a Session's folder whatever the
    /// spelling it is asked for.
    pub fn sessions(&self) -> PathBuf {
        self.path.join("sessions")
    }

    /// The lock that serializes starting and deleting this root's Sessions, kept out of the top
    /// level as the settings file's lock is.
    pub fn sessions_lock(&self) -> PathBuf {
        self.internal().join("sessions.lock")
    }

    /// Logs and destructive-command records.
    pub fn logs(&self) -> PathBuf {
        self.path.join("logs")
    }

    /// The folder for everything Orchbee manages for itself.
    pub fn internal(&self) -> PathBuf {
        self.path.join(INTERNAL)
    }

    /// The layout version record.
    pub fn version_file(&self) -> PathBuf {
        self.internal().join("version.json")
    }

    /// The device records folder; [`StateRecord`] names its files.
    pub fn state(&self) -> PathBuf {
        self.internal().join("state")
    }

    /// The writer lock of the default settings file, kept out of the top level.
    pub fn config_lock_directory(&self) -> PathBuf {
        self.internal()
    }

    /// Downloaded detection rules, discarded and rebuilt freely.
    pub fn agent_detection_cache(&self) -> PathBuf {
        self.internal().join("cache").join("agent-detection")
    }

    /// Copies taken before a reset, one folder per `<YYYY-MM-DD>-<reason>`.
    pub fn backups(&self) -> PathBuf {
        self.internal().join("backups")
    }

    /// The plugin store; `PluginStorePaths` places what it holds.
    pub fn plugins(&self) -> PathBuf {
        self.internal().join("plugins")
    }

    /// What Agent integrations install for this environment.
    pub fn integrations(&self) -> PathBuf {
        self.internal().join("integrations")
    }

    /// The Agent observation Capsules this environment's Agent entries run, one folder per
    /// version; the integration writer chooses the version names.
    pub fn agent_observation_capsules(&self) -> PathBuf {
        self.integrations().join("agent-observation")
    }

    /// The default runtime base: one runtime folder per Session, named by the Session. An
    /// explicit `ORCHBEE_RUNTIME_BASE` replaces it.
    pub fn runtime(&self) -> PathBuf {
        self.internal().join("runtime")
    }

    /// Scrollback documents the Terminal Host writes for an editor; nothing else lives here.
    pub fn tmp(&self) -> PathBuf {
        self.internal().join("tmp")
    }
}

/// `~/.orchbee/agent-observation-adapters`, where every environment installed Agent observation
/// Capsules before each kept its own in its data root. Only release Orchbee owns it, so other
/// environments have none.
pub fn legacy_agent_observation_capsules(home: &Path) -> Option<PathBuf> {
    crate::is_release().then(|| home.join(".orchbee").join("agent-observation-adapters"))
}

/// Cheap local evidence, without running provider commands or reading their configuration, that an
/// Agent integration may have been enabled for this environment: enabling installs a Capsule in its
/// data root, and release Orchbee once installed them in the legacy folder. `false` means neither
/// folder exists, so no provider entry can name this environment. `true` proves nothing about
/// current entries; an unreadable location also answers `true`.
pub fn agent_integration_may_have_been_enabled() -> bool {
    let present = |path: &Path| !matches!(path.try_exists(), Ok(false));
    let Ok(data_root) = DataRoot::resolve(|name| std::env::var_os(name)) else {
        return true;
    };
    present(&data_root.agent_observation_capsules())
        || std::env::home_dir().map_or(crate::is_release(), |home| {
            legacy_agent_observation_capsules(&home).is_some_and(|legacy| present(&legacy))
        })
}

/// The runtime base this environment uses when `ORCHBEE_RUNTIME_BASE` does not name one: the data
/// root's `internal/runtime`, absent only when the data root cannot be resolved.
pub fn default_runtime_base(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    DataRoot::resolve(env).ok().map(|root| root.runtime())
}

#[cfg(windows)]
pub(crate) const HOME_SOURCE: &str = "USERPROFILE";
#[cfg(not(windows))]
pub(crate) const HOME_SOURCE: &str = "HOME";

/// Admits an absolute path, such as `ORCHBEE_HOME` or `ORCHBEE_CONFIG_FILE`, without touching the
/// file system. Windows applies the user-path grammar the settings file always used, so forward
/// slashes and verbatim prefixes normalize the same way there; other hosts take a plain absolute
/// path without `.` or `..` parts.
pub(crate) fn admit_absolute(
    value: &OsString,
    variable: &'static str,
) -> Result<PathBuf, DataRootError> {
    let invalid = DataRootError {
        variable,
        reason: "invalid-path",
    };
    let raw = value
        .to_str()
        .filter(|raw| !raw.is_empty())
        .ok_or(invalid)?;
    #[cfg(windows)]
    let path = {
        if !Path::new(raw).is_absolute() {
            return Err(invalid);
        }
        let normalized =
            crate::normalize_user_path(raw, |_| None, || Err(std::io::ErrorKind::NotFound.into()))
                .map_err(|_| invalid)?;
        PathBuf::from(normalized)
    };
    #[cfg(not(windows))]
    let path = {
        let path = PathBuf::from(raw);
        if !path.is_absolute()
            || path.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(invalid);
        }
        path
    };
    if path
        .to_str()
        .is_none_or(|text| text.len() > crate::USER_PATH_LIMIT)
    {
        return Err(invalid);
    }
    Ok(path)
}

#[cfg(test)]
mod tests;
