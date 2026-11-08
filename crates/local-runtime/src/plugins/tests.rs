use super::*;
#[cfg(windows)]
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const SOURCE: &str = "id='example.tools'\nname='Tools'\nversion='1.0.0'\n[[actions]]\nid='hello'\ntitle='Hello'\ncommand=['cmd.exe','/c','echo hello']\n";
#[test]
fn plugin_manifest_boundaries_and_platform_inheritance() {
    let source = SOURCE.replace(
        "id='example.tools'",
        "id=' example.tools '\nplatforms=['linux','windows']",
    );
    let manifest = PluginManifest::parse(&source).unwrap();
    assert_eq!(manifest.id, "example.tools");
    assert!(manifest.supports_windows(&None));
    assert!(!manifest.supports_windows(&Some(vec![PluginPlatform::Linux])));
    assert_eq!(manifest.actions[0].command[2], "echo hello");
    for invalid in [
        SOURCE.replace("example.tools", "bad/id"),
        SOURCE.replace("hello'\ntitle", "hello.world'\ntitle"),
        SOURCE.replace("command=['cmd.exe','/c','echo hello']", "command=[]"),
        format!("{SOURCE}unknown=true"),
        format!("{SOURCE}[[actions]]\nid='hello'\ntitle='Again'\ncommand=['cmd.exe']"),
        SOURCE.replace("name='Tools'", "name='Tools'\nplatforms=['unknown']"),
        SOURCE.replace("name='Tools'", "name='Tools'\nmin_orchbee_version='9.0.0'"),
    ] {
        assert!(PluginManifest::parse(&invalid).is_err(), "{invalid}");
    }
    assert_eq!(
        PluginManifest::parse(&" ".repeat(128 * 1024 + 1)),
        Err(PluginRegistryError::LimitExceeded)
    );
}
#[cfg(windows)]
struct Root(PathBuf);
#[cfg(windows)]
impl Root {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-plugin-catalog-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self) -> PathBuf {
        let path = self.0.join("orchbee-plugin.toml");
        fs::write(&path, SOURCE).unwrap();
        path
    }
    fn registry(&self) -> PluginRegistry {
        PluginRegistry::at(self.0.join("registry")).unwrap()
    }
}
#[cfg(windows)]
impl Drop for Root {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-plugin-catalog-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[cfg(windows)]
#[test]
fn plugin_catalog_transitions_preserve_sources_and_user_dirs() {
    let root = Root::new();
    let registry = root.registry();
    assert!(registry.list(None).unwrap().plugins.is_empty());
    assert!(!registry.root().exists());
    let path = root.source();
    let link = registry
        .mutate(PluginRegistryMutation::Link {
            path: path.clone(),
            enabled: true,
        })
        .unwrap();
    assert_eq!(link.revision, 1);
    assert!(link.changed);
    let config = registry.config_dir("example.tools").unwrap();
    fs::write(config.join("user.txt"), b"keep").unwrap();
    assert!(
        !registry
            .mutate(PluginRegistryMutation::Link {
                path: path.clone(),
                enabled: true
            })
            .unwrap()
            .changed
    );
    let disabled = registry
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "example.tools".into(),
            enabled: false,
        })
        .unwrap();
    assert_eq!(disabled.revision, 2);
    assert!(!disabled.plugins[0].enabled);
    fs::remove_file(&path).unwrap();
    let unavailable = registry.list(None).unwrap();
    assert_eq!(
        unavailable.plugins[0].warning,
        Some(PluginRegistryError::ManifestUnavailable)
    );
    assert_eq!(unavailable.plugins[0].manifest.name, "Tools");
    assert!(
        registry
            .mutate(PluginRegistryMutation::SetEnabled {
                id: "example.tools".into(),
                enabled: true
            })
            .is_err()
    );
    assert_eq!(registry.list(None).unwrap().revision, 2);
    root.source();
    let removed = registry
        .mutate(PluginRegistryMutation::Unlink {
            id: "example.tools".into(),
        })
        .unwrap();
    assert_eq!(removed.revision, 3);
    assert!(removed.plugins.is_empty());
    assert!(path.exists());
    assert_eq!(fs::read(config.join("user.txt")).unwrap(), b"keep");
    assert!(
        !registry
            .mutate(PluginRegistryMutation::Unlink {
                id: "example.tools".into()
            })
            .unwrap()
            .changed
    );
    assert_eq!(
        registry
            .mutate(PluginRegistryMutation::SetEnabled {
                id: "absent".into(),
                enabled: true
            })
            .unwrap_err(),
        PluginRegistryError::PluginNotFound
    );
}

#[cfg(windows)]
#[test]
fn registered_catalog_does_not_refresh_linked_manifests() {
    let root = Root::new();
    let registry = root.registry();
    let path = root.source();
    registry
        .mutate(PluginRegistryMutation::Link {
            path: path.clone(),
            enabled: true,
        })
        .unwrap();
    let before = registry.list_registered().unwrap();
    assert_eq!(before.plugins[0].manifest.name, "Tools");

    fs::write(&path, SOURCE.replace("name='Tools'", "name='Changed'")).unwrap();
    let stored = registry.list_registered().unwrap();
    assert_eq!(stored, before, "linked edits are not click authority");
    assert_eq!(
        registry.list(None).unwrap().plugins[0].manifest.name,
        "Changed"
    );

    registry
        .mutate(PluginRegistryMutation::Link {
            path,
            enabled: true,
        })
        .unwrap();
    let published = registry.list_registered().unwrap();
    assert_eq!(published.plugins[0].manifest.name, "Changed");
    assert_ne!(published.fingerprint, before.fingerprint);
}

#[cfg(windows)]
#[test]
fn registered_catalog_refuses_non_disk_roots_before_reading() {
    let registry = PluginRegistry::at(PathBuf::from(r"\\server\share\plugins")).unwrap();
    assert_eq!(
        registry.list_registered().unwrap_err(),
        PluginRegistryError::InvalidPath
    );
}
#[cfg(windows)]
#[test]
fn plugin_catalog_lock_and_corruption_never_overwrite_previous_state() {
    let root = Root::new();
    let registry = root.registry();
    let path = root.source();
    registry
        .mutate(PluginRegistryMutation::Link {
            path: path.clone(),
            enabled: true,
        })
        .unwrap();
    let mut cache = crate::PrivateCache::acquire(registry.root(), "registry").unwrap();
    let before = cache.read("registry.json", 1024 * 1024).unwrap().unwrap();
    assert_eq!(
        registry
            .mutate(PluginRegistryMutation::Unlink {
                id: "example.tools".into()
            })
            .unwrap_err(),
        PluginRegistryError::Busy
    );
    assert_eq!(
        cache.read("registry.json", 1024 * 1024).unwrap().unwrap(),
        before
    );
    cache
        .replace(
            "registry.json",
            b"{\"version\":255,\"revision\":0,\"plugins\":[]}",
        )
        .unwrap();
    drop(cache);
    assert_eq!(
        registry.list(None).unwrap_err(),
        PluginRegistryError::InvalidRegistry
    );
    assert_eq!(
        registry
            .mutate(PluginRegistryMutation::Link {
                path,
                enabled: false
            })
            .unwrap_err(),
        PluginRegistryError::InvalidRegistry
    );
}
#[cfg(windows)]
#[test]
fn plugin_catalog_user_paths_are_stable_distinct_and_windows_safe() {
    let root = Root::new();
    let registry = root.registry();
    let mut folders = std::collections::BTreeSet::new();
    for id in [
        "..",
        "CON",
        "con.example",
        "a:b",
        "a_b",
        "Example",
        "example",
    ] {
        let (config, data) = registry.user_directories(id).unwrap();
        assert!(config.is_dir() && data.is_dir());
        assert_eq!(config, registry.config_dir(id).unwrap());
        // Each plugin's folders share one folder of its own directly in the store, distinct
        // from every other plugin's even where letter case does not distinguish names.
        let folder = config.parent().unwrap();
        assert_eq!(data.parent(), Some(folder));
        assert_eq!(folder.parent(), Some(registry.root()));
        assert!(folders.insert(folder.to_string_lossy().to_ascii_lowercase()));
        // An id that is already a plain folder name is the name a person finds.
        if matches!(id, "a_b" | "Example") {
            assert_eq!(folder.file_name().unwrap(), id);
        }
    }
    assert!(registry.config_dir("../outside").is_err());
}

#[test]
fn plugin_root_process_spelling_drops_only_a_faithful_verbatim_prefix() {
    use super::root_spelling::process_spelling;
    for (stored, process) in [
        (r"\\?\C:\work\plugin", r"C:\work\plugin"),
        (r"\\?\C:\", r"C:\"),
        (r"\\?\UNC\server\share\plugin", r"\\server\share\plugin"),
        (r"C:\work\plugin", r"C:\work\plugin"),
    ] {
        assert_eq!(process_spelling(stored), process);
    }
    for kept in [
        r"\\?\C:\work\trailing.",
        r"\\?\C:\work\trailing \plugin",
        r"\\?\C:\work\..\plugin",
        r"\\?\C:\work\NUL",
        r"\\?\C:\work\com1.txt",
        r"\\?\C:/work",
        r"\\?\C:\work\\plugin",
        r"\\?\UNC\",
        r"\\?\GLOBALROOT\Device",
    ] {
        assert_eq!(process_spelling(kept), kept);
    }
}
