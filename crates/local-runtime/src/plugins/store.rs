//! Owns the private plugin registry document, guarded revisions, registered-source
//! fingerprints, manifest refresh observations, and the folder each plugin id received in the
//! store. It does not execute plugin commands or choose application behavior from display
//! metadata.

use super::{
    PluginManagedSource, PluginManifest, PluginRegistryError as Error, manifest::MANIFEST_LIMIT,
    plugin_identifier,
};
use crate::{DataRoot, PrivateCache, data_root::PluginStorePaths};
use naming::Generation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

const REGISTRY_LIMIT: usize = 1024 * 1024;
const MAX_REVISION: u64 = 9_007_199_254_740_991;
const FILE: &str = PluginStorePaths::REGISTRY_FILE;
const REGISTRY_RECORD: &str = PluginStorePaths::REGISTRY_RECORD;
/// Folders outlive their plugin's registration, so the registry remembers more of them than it
/// registers plugins; records of unregistered plugins whose folder is gone are forgotten first.
const FOLDER_LIMIT: usize = 256;
mod naming;
mod provision;
pub use provision::{
    PluginProvisionCommand, PluginProvisionExecutor, PluginProvisionFailure, PluginProvisionOutput,
    PluginProvisionReport, PluginProvisionStage,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRegistryEntry {
    pub manifest_path: String,
    pub plugin_root: String,
    pub enabled: bool,
    pub manifest: PluginManifest,
    #[serde(deserialize_with = "Option::deserialize")]
    pub warning: Option<Error>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub source: Option<PluginManagedSource>,
}
impl PluginRegistryEntry {
    /// The plugin root as plugin processes receive it: the ordinary Win32 spelling of the stored
    /// canonical root whenever that names the same directory, otherwise the stored spelling.
    #[must_use]
    pub fn process_root(&self) -> String {
        super::root_spelling::process_spelling(&self.plugin_root)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    version: u8,
    revision: u64,
    plugins: Vec<PluginRegistryEntry>,
    /// The folder each plugin id received, kept after unlink because its folders stay.
    folders: BTreeMap<String, String>,
    /// Source folders to remove, as [`Generation`] keys recorded before each was created.
    cleanup: Vec<String>,
    blocked_cleanup: Vec<String>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            plugins: vec![],
            folders: BTreeMap::new(),
            cleanup: vec![],
            blocked_cleanup: vec![],
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistryResult {
    pub revision: u64,
    pub changed: bool,
    pub plugins: Vec<PluginRegistryEntry>,
}
/// Persisted catalog authority for a link click. It never reads linked manifest paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginRegisteredCatalog {
    pub revision: u64,
    pub fingerprint: String,
    pub plugins: Vec<PluginRegistryEntry>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginRegistryMutation {
    Link { path: PathBuf, enabled: bool },
    Unlink { id: String },
    SetEnabled { id: String, enabled: bool },
}
#[derive(Clone, Debug)]
pub struct PluginRegistry {
    paths: PluginStorePaths,
}
impl PluginRegistry {
    pub fn from_environment(
        env: impl Fn(&str) -> Option<std::ffi::OsString>,
    ) -> Result<Self, Error> {
        platform::require()?;
        let root = DataRoot::resolve(env).map_err(|_| Error::InvalidPath)?;
        Self::at(root.plugins())
    }
    /// The root is an absolute, private catalog directory, not a linked plugin source.
    pub fn at(root: PathBuf) -> Result<Self, Error> {
        platform::require()?;
        if !root.is_absolute()
            || root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            || root
                .to_str()
                .is_none_or(|p| p.len() > 4096 || p.contains('\0'))
        {
            return Err(Error::InvalidPath);
        }
        Ok(Self {
            paths: PluginStorePaths::new(root),
        })
    }
    pub fn root(&self) -> &Path {
        self.paths.root()
    }
    pub fn list(&self, id: Option<&str>) -> Result<PluginRegistryResult, Error> {
        let id = id.map(plugin_identifier).transpose()?;
        let bytes =
            PrivateCache::read_existing(self.root(), FILE, REGISTRY_LIMIT).map_err(storage)?;
        let mut catalog = decode(bytes.as_deref(), &self.paths)?;
        for entry in &mut catalog.plugins {
            refresh(entry);
        }
        catalog
            .plugins
            .retain(|entry| id.as_ref().is_none_or(|id| *id == entry.manifest.id));
        Ok(PluginRegistryResult {
            revision: catalog.revision,
            changed: false,
            plugins: catalog.plugins,
        })
    }
    /// Read the stored catalog from a pinned fixed local volume without refreshing linked sources.
    pub fn list_registered(&self) -> Result<PluginRegisteredCatalog, Error> {
        if !fixed_local_volume(self.root()) {
            return Err(Error::InvalidPath);
        }
        let read = PrivateCache::read_existing_pinned(self.root(), FILE, REGISTRY_LIMIT)
            .map_err(storage)?;
        let catalog = decode(read.bytes(), &self.paths)?;
        let fingerprint = registered_fingerprint(catalog.revision, &catalog.plugins)?;
        Ok(PluginRegisteredCatalog {
            revision: catalog.revision,
            fingerprint,
            plugins: catalog.plugins,
        })
    }
    pub fn config_dir(&self, id: &str) -> Result<PathBuf, Error> {
        self.user_directories(id).map(|(config, _)| config)
    }
    /// The plugin's own configuration and data folders, created when absent. They stay after
    /// unlink, and the plugin finds them again under the same folder when it returns.
    pub fn user_directories(&self, id: &str) -> Result<(PathBuf, PathBuf), Error> {
        let id = plugin_identifier(id)?;
        let bytes =
            PrivateCache::read_existing(self.root(), FILE, REGISTRY_LIMIT).map_err(storage)?;
        let folder = match decode(bytes.as_deref(), &self.paths)?.folders.remove(&id) {
            Some(folder) => folder,
            // A first use records the folder under the writer lock, so two processes never give
            // one folder to different plugins.
            None => {
                let mut store =
                    PrivateCache::acquire(self.root(), REGISTRY_RECORD).map_err(storage)?;
                let mut catalog = decode(
                    store
                        .read(FILE, REGISTRY_LIMIT)
                        .map_err(storage)?
                        .as_deref(),
                    &self.paths,
                )?;
                let (folder, assigned) = self.assign_folder(&mut catalog, &id)?;
                if assigned {
                    store
                        .replace(FILE, &encode(&catalog, &self.paths)?)
                        .map_err(storage)?;
                }
                folder
            }
        };
        self.prepare_user_directories(&folder)
    }
    fn prepare_user_directories(&self, folder: &str) -> Result<(PathBuf, PathBuf), Error> {
        let config = self.paths.config(folder);
        let data = self.paths.data(folder);
        // The plugin owns what these folders hold, so no writer lock goes in them.
        PrivateCache::prepare(&config).map_err(storage)?;
        PrivateCache::prepare(&data).map_err(storage)?;
        Ok((config, data))
    }
    /// Under the writer lock: the folder `catalog` records for `id`, or a newly recorded one and
    /// `true`. The name spells the id; a name another plugin's folder, a reserved name, or
    /// something else in the store already takes gains the next free suffix. An unrecorded folder
    /// named exactly the id, which a reset registry leaves, is the plugin's own again.
    fn assign_folder(&self, catalog: &mut Catalog, id: &str) -> Result<(String, bool), Error> {
        if let Some(folder) = catalog.folders.get(id) {
            return Ok((folder.clone(), false));
        }
        let plugins = &catalog.plugins;
        let mut kept = Ok(());
        catalog.folders.retain(|owner, folder| {
            if plugins.iter().any(|entry| entry.manifest.id == *owner) {
                return true;
            }
            match fs::symlink_metadata(self.paths.plugin(folder)) {
                Ok(_) => true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => {
                    kept = Err(storage(error));
                    true
                }
            }
        });
        kept?;
        if catalog.folders.len() >= FOLDER_LIMIT {
            return Err(Error::LimitExceeded);
        }
        let base = naming::spelling(id, "plugin");
        for folder in naming::candidates(&base) {
            if PluginStorePaths::reserved(&folder)
                || catalog
                    .folders
                    .values()
                    .any(|taken| taken.eq_ignore_ascii_case(&folder))
            {
                continue;
            }
            let free = match self.stored_name(&folder)? {
                None => true,
                Some((name, directory)) => directory && name == id,
            };
            if free {
                catalog.folders.insert(id.to_owned(), folder.clone());
                return Ok((folder, true));
            }
        }
        Err(Error::LimitExceeded)
    }
    /// The exact spelling of the store entry that `name` finds, which on Windows may differ in
    /// letter case, and whether it is a plain directory.
    fn stored_name(&self, name: &str) -> Result<Option<(String, bool)>, Error> {
        match fs::symlink_metadata(self.paths.plugin(name)) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(storage(error)),
        }
        for entry in fs::read_dir(self.root()).map_err(storage)? {
            let entry = entry.map_err(storage)?;
            let Some(stored) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if stored.eq_ignore_ascii_case(name) {
                let kind = entry.file_type().map_err(storage)?;
                return Ok(Some((stored, kind.is_dir() && !kind.is_symlink())));
            }
        }
        // Present by path yet not listed: another spelling resolved to it, so it is taken.
        Ok(Some((String::new(), false)))
    }
    pub fn mutate(&self, mutation: PluginRegistryMutation) -> Result<PluginRegistryResult, Error> {
        // All caller-controlled identity and manifest validation precedes storage creation.
        let (id, linked) = match &mutation {
            PluginRegistryMutation::Link { path, enabled } => {
                let entry = load(path, *enabled)?;
                (entry.manifest.id.clone(), Some(entry))
            }
            PluginRegistryMutation::Unlink { id }
            | PluginRegistryMutation::SetEnabled { id, .. } => (plugin_identifier(id)?, None),
        };
        let mut store = PrivateCache::acquire(self.root(), REGISTRY_RECORD).map_err(storage)?;
        let bytes = store.read(FILE, REGISTRY_LIMIT).map_err(storage)?;
        let mut catalog = decode(bytes.as_deref(), &self.paths)?;
        let before = catalog.plugins.clone();
        match mutation {
            PluginRegistryMutation::Link { .. } => {
                let entry = linked.expect("link was validated");
                if let Some(existing) = catalog.plugins.iter_mut().find(|e| e.manifest.id == id) {
                    *existing = entry;
                } else {
                    catalog.plugins.push(entry);
                }
            }
            PluginRegistryMutation::Unlink { .. } => {
                catalog.plugins.retain(|e| e.manifest.id != id);
            }
            PluginRegistryMutation::SetEnabled { enabled, .. } => {
                let entry = catalog
                    .plugins
                    .iter_mut()
                    .find(|e| e.manifest.id == id)
                    .ok_or(Error::PluginNotFound)?;
                if enabled {
                    let current = load(Path::new(&entry.manifest_path), true)?;
                    if current.manifest.id != id {
                        return Err(Error::InvalidManifest);
                    }
                    let source = entry.source.take();
                    *entry = current;
                    entry.source = source;
                } else {
                    entry.enabled = false;
                }
            }
        }
        catalog
            .plugins
            .sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        let changed = catalog.plugins != before;
        if changed {
            catalog.revision = catalog
                .revision
                .checked_add(1)
                .filter(|n| *n <= MAX_REVISION)
                .ok_or(Error::LimitExceeded)?;
            if catalog.plugins.len() > 32 {
                return Err(Error::LimitExceeded);
            }
            let linked_folder = if registered(&catalog, &id) {
                Some(self.assign_folder(&mut catalog, &id)?.0)
            } else {
                None
            };
            let encoded = encode(&catalog, &self.paths)?;
            if let Some(folder) = linked_folder {
                self.prepare_user_directories(&folder)?;
            }
            store.replace(FILE, &encoded).map_err(storage)?;
        }
        for entry in &mut catalog.plugins {
            refresh(entry);
        }
        Ok(PluginRegistryResult {
            revision: catalog.revision,
            changed,
            plugins: catalog.plugins,
        })
    }
}

fn fixed_local_volume(path: &Path) -> bool {
    platform::fixed_local_volume(path)
}

fn registered_fingerprint(revision: u64, plugins: &[PluginRegistryEntry]) -> Result<String, Error> {
    let encoded = serde_json::to_vec(plugins).map_err(|_| Error::InvalidRegistry)?;
    let mut hash = Sha256::new();
    hash.update(b"orchbee.plugin.registered-catalog.v1\0");
    hash.update(revision.to_le_bytes());
    hash.update((encoded.len() as u64).to_le_bytes());
    hash.update(encoded);
    Ok(format!("{:x}", hash.finalize()))
}
fn registered(catalog: &Catalog, id: &str) -> bool {
    catalog.plugins.iter().any(|entry| entry.manifest.id == id)
}
fn storage(error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::WouldBlock => Error::Busy,
        _ => Error::StorageUnavailable,
    }
}
/// Encodes a complete next catalog that passes the same checks a read applies.
fn encode(catalog: &Catalog, paths: &PluginStorePaths) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(catalog).map_err(|_| Error::InvalidRegistry)?;
    if bytes.len() > REGISTRY_LIMIT {
        return Err(Error::LimitExceeded);
    }
    decode(Some(&bytes), paths)?;
    Ok(bytes)
}
fn decode(bytes: Option<&[u8]>, paths: &PluginStorePaths) -> Result<Catalog, Error> {
    let Some(bytes) = bytes else {
        return Ok(Catalog::default());
    };
    let catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| Error::InvalidRegistry)?;
    // Authoring TOML permits omitted fields; the retained JSON must contain the complete
    // serialized manifest and provenance. Compare JSON values so whitespace/key order are free.
    let retained: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Error::InvalidRegistry)?;
    if retained != serde_json::to_value(&catalog).map_err(|_| Error::InvalidRegistry)? {
        return Err(Error::InvalidRegistry);
    }
    let generations = catalog
        .cleanup
        .iter()
        .map(|key| Generation::parse(key))
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::InvalidRegistry)?;
    if catalog.version != 1
        || catalog.revision > MAX_REVISION
        || catalog.plugins.len() > 32
        || catalog.folders.len() > FOLDER_LIMIT
        || catalog.cleanup.len() > 64
        || catalog.blocked_cleanup.len() > 64
        || generations.iter().enumerate().any(|(i, generation)| {
            generations[..i]
                .iter()
                .any(|earlier| earlier.same_folder(generation))
        })
        || catalog
            .blocked_cleanup
            .iter()
            .any(|key| !catalog.cleanup.contains(key))
    {
        return Err(Error::InvalidRegistry);
    }
    let mut folders: Vec<&str> = Vec::with_capacity(catalog.folders.len());
    for (id, folder) in &catalog.folders {
        if plugin_identifier(id).ok().as_deref() != Some(id.as_str())
            || !naming::valid(folder)
            || PluginStorePaths::reserved(folder)
            || folders
                .iter()
                .any(|taken| taken.eq_ignore_ascii_case(folder))
        {
            return Err(Error::InvalidRegistry);
        }
        folders.push(folder);
    }
    // Managed roots are canonical, so they compare with the canonical store.
    let canonical = if catalog.plugins.iter().any(|entry| entry.source.is_some()) {
        Some(PluginStorePaths::new(
            paths.root().canonicalize().map_err(storage)?,
        ))
    } else {
        None
    };
    let mut previous: Option<&str> = None;
    for entry in &catalog.plugins {
        let folder = catalog
            .folders
            .get(&entry.manifest.id)
            .ok_or(Error::InvalidRegistry)?;
        if let Some(source) = &entry.source {
            source.validate()?;
            let sources = canonical
                .as_ref()
                .ok_or(Error::InvalidRegistry)?
                .sources(folder);
            if !managed_root(
                &entry.plugin_root,
                &sources,
                source.github.subdir.as_deref(),
            ) {
                return Err(Error::InvalidRegistry);
            }
        }
        let mut normalized = entry.manifest.clone();
        normalized.normalize().map_err(|_| Error::InvalidRegistry)?;
        if normalized != entry.manifest
            || entry.warning.is_some()
            || previous.is_some_and(|p| p >= entry.manifest.id.as_str())
        {
            return Err(Error::InvalidRegistry);
        }
        for path in [&entry.manifest_path, &entry.plugin_root] {
            if path.len() > 4096 || path.contains('\0') || !Path::new(path).is_absolute() {
                return Err(Error::InvalidRegistry);
            }
        }
        if Path::new(&entry.manifest_path).parent() != Some(Path::new(&entry.plugin_root)) {
            return Err(Error::InvalidRegistry);
        }
        previous = Some(&entry.manifest.id);
    }
    Ok(catalog)
}
/// Whether a managed plugin root is one version's source folder in its plugin's `sources`, or the
/// recorded subfolder of it.
fn managed_root(root: &str, sources: &Path, subdir: Option<&str>) -> bool {
    let Ok(rest) = Path::new(root).strip_prefix(sources) else {
        return false;
    };
    let mut parts = rest.components();
    let version = matches!(
        parts.next(),
        Some(Component::Normal(version)) if version.to_str().is_some_and(naming::valid)
    );
    let expected = subdir.into_iter().flat_map(|subdir| subdir.split('/'));
    version
        && parts
            .map(|part| part.as_os_str().to_str())
            .eq(expected.map(Some))
}
fn refresh(entry: &mut PluginRegistryEntry) {
    match load(Path::new(&entry.manifest_path), entry.enabled) {
        Ok(mut current) if current.manifest.id == entry.manifest.id => {
            current.source = entry.source.take();
            *entry = current;
        }
        Ok(_) => entry.warning = Some(Error::InvalidManifest),
        Err(error) => entry.warning = Some(error),
    }
}
fn load(path: &Path, enabled: bool) -> Result<PluginRegistryEntry, Error> {
    let path = if path.is_dir() {
        path.join("orchbee-plugin.toml")
    } else {
        path.to_owned()
    };
    let path = path
        .canonicalize()
        .map_err(|_| Error::ManifestUnavailable)?;
    let root = path.parent().ok_or(Error::InvalidPath)?;
    let manifest_path = path
        .to_str()
        .filter(|s| s.len() <= 4096)
        .ok_or(Error::InvalidPath)?
        .to_owned();
    let plugin_root = root.to_str().ok_or(Error::InvalidPath)?.to_owned();
    let file = File::open(&path).map_err(|_| Error::ManifestUnavailable)?;
    let metadata = file.metadata().map_err(|_| Error::ManifestUnavailable)?;
    if !metadata.is_file() || metadata.len() > MANIFEST_LIMIT as u64 {
        return Err(Error::LimitExceeded);
    }
    let mut source = String::new();
    file.take(MANIFEST_LIMIT as u64 + 1)
        .read_to_string(&mut source)
        .map_err(|_| Error::InvalidManifest)?;
    let manifest = PluginManifest::parse(&source)?;
    Ok(PluginRegistryEntry {
        manifest_path,
        plugin_root,
        enabled,
        manifest,
        warning: None,
        source: None,
    })
}
#[cfg(windows)]
#[path = "store/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "store/unix.rs"]
mod platform;
