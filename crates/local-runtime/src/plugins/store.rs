//! Owns the private plugin registry document, guarded revisions, registered-source
//! fingerprints, and manifest refresh observations. It does not execute plugin commands or
//! choose application behavior from display metadata.

use super::{
    PluginManagedSource, PluginManifest, PluginRegistryError as Error, manifest::MANIFEST_LIMIT,
    plugin_identifier,
};
use crate::{PrivateCache, user_settings_path};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

const REGISTRY_LIMIT: usize = 1024 * 1024;
const MAX_REVISION: u64 = 9_007_199_254_740_991;
const FILE: &str = "registry.json";
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
    cleanup: Vec<String>,
    blocked_cleanup: Vec<String>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            plugins: vec![],
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
    root: PathBuf,
}
impl PluginRegistry {
    pub fn from_environment(
        env: impl Fn(&str) -> Option<std::ffi::OsString>,
    ) -> Result<Self, Error> {
        platform::require()?;
        let path = user_settings_path(env)
            .map_err(|_| Error::InvalidPath)?
            .ok_or(Error::InvalidPath)?;
        Self::at(path.parent().ok_or(Error::InvalidPath)?.join("plugins"))
    }
    /// The root is an absolute, private catalog directory, not a linked plugin source.
    pub fn at(root: PathBuf) -> Result<Self, Error> {
        platform::require()?;
        if !root.is_absolute()
            || root.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || root
                .to_str()
                .is_none_or(|p| p.len() > 4096 || p.contains('\0'))
        {
            return Err(Error::InvalidPath);
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn list(&self, id: Option<&str>) -> Result<PluginRegistryResult, Error> {
        let id = id.map(plugin_identifier).transpose()?;
        let bytes =
            PrivateCache::read_existing(&self.root, FILE, REGISTRY_LIMIT).map_err(storage)?;
        let mut catalog = decode(bytes.as_deref(), &self.root)?;
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
        if !fixed_local_volume(&self.root) {
            return Err(Error::InvalidPath);
        }
        let read = PrivateCache::read_existing_pinned(&self.root, FILE, REGISTRY_LIMIT)
            .map_err(storage)?;
        let catalog = decode(read.bytes(), &self.root)?;
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
    pub fn user_directories(&self, id: &str) -> Result<(PathBuf, PathBuf), Error> {
        let id = plugin_identifier(id)?;
        let component = component(&id);
        let config = self.root.join("config").join(&component);
        let state = self.root.join("state").join(component);
        // Each statement releases its cache guard before acquiring the next directory.
        PrivateCache::acquire(&config).map_err(storage)?;
        PrivateCache::acquire(&state).map_err(storage)?;
        Ok((config, state))
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
        let mut store = PrivateCache::acquire(&self.root).map_err(storage)?;
        let bytes = store.read(FILE, REGISTRY_LIMIT).map_err(storage)?;
        let mut catalog = decode(bytes.as_deref(), &self.root)?;
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
            let encoded = serde_json::to_vec(&catalog).map_err(|_| Error::InvalidRegistry)?;
            if encoded.len() > REGISTRY_LIMIT {
                return Err(Error::LimitExceeded);
            }
            if linked_dirs_needed(&catalog, &id) {
                self.config_dir(&id)?;
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
fn linked_dirs_needed(catalog: &Catalog, id: &str) -> bool {
    catalog.plugins.iter().any(|entry| entry.manifest.id == id)
}
fn storage(error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::WouldBlock => Error::Busy,
        _ => Error::StorageUnavailable,
    }
}
fn decode(bytes: Option<&[u8]>, root: &Path) -> Result<Catalog, Error> {
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
    if catalog.version != 1
        || catalog.revision > MAX_REVISION
        || catalog.plugins.len() > 32
        || catalog.cleanup.len() > 64
        || catalog.blocked_cleanup.len() > 64
        || catalog
            .cleanup
            .iter()
            .enumerate()
            .any(|(i, id)| !super::source::installation_id(id) || catalog.cleanup[..i].contains(id))
        || catalog
            .blocked_cleanup
            .iter()
            .any(|id| !catalog.cleanup.contains(id))
    {
        return Err(Error::InvalidRegistry);
    }
    let mut previous: Option<&str> = None;
    for entry in &catalog.plugins {
        if let Some(source) = &entry.source {
            source.validate()?;
            let checkout = root
                .canonicalize()
                .map_err(storage)?
                .join("managed")
                .join(format!("i-{}", source.installation_id))
                .join("checkout");
            let expected = source
                .github
                .subdir
                .as_ref()
                .map_or_else(|| checkout.clone(), |subdir| checkout.join(subdir));
            if Path::new(&entry.plugin_root) != expected {
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
fn component(id: &str) -> String {
    let digest = Sha256::digest(id.as_bytes());
    let mut name = String::from("p-");
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(name, "{byte:02x}");
    }
    name
}

#[cfg(windows)]
#[path = "store/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "store/unix.rs"]
mod platform;
