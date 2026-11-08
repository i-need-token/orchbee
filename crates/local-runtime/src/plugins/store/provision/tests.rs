use super::*;
use std::fs;

const MANIFEST: &str = "id='tools'\nname='Tools'\nversion='1.0.0'\n[[build]]\ncommand=['build.exe','literal & value']\n";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-plugin-install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn registry(&self) -> PluginRegistry {
        PluginRegistry::at(self.0.join("registry")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-plugin-install-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
#[derive(Default)]
struct Executor {
    failure: Option<CommandFailure>,
    mutate: bool,
    called: usize,
    commit: Option<char>,
    manifest: Option<String>,
    fetched: Vec<String>,
}
impl PluginProvisionExecutor for Executor {
    fn checkpoint(&mut self) -> Result<(), CommandFailure> {
        Ok(())
    }
    fn run(&mut self, command: &PluginProvisionCommand) -> CommandOutput {
        self.called += 1;
        let mut output = CommandOutput {
            exit_code: Some(0),
            failure: None,
            stdout: vec![],
            stderr: vec![],
            stdout_truncated: false,
            stderr_truncated: false,
        };
        if command.argv.iter().any(|a| a == "fetch") {
            self.fetched.push(command.argv.last().unwrap().clone());
        }
        if command.argv.iter().any(|a| a == "checkout") {
            fs::write(
                command.directory.join("orchbee-plugin.toml"),
                self.manifest.as_deref().unwrap_or(MANIFEST),
            )
            .unwrap();
        }
        if command.argv.iter().any(|a| a == "rev-parse") {
            output.stdout = self
                .commit
                .unwrap_or('a')
                .to_string()
                .repeat(40)
                .into_bytes();
        }
        if command.stage == PluginProvisionStage::Build {
            assert_eq!(command.argv, vec!["build.exe", "literal & value"]);
            if self.mutate {
                fs::write(
                    command.directory.join("orchbee-plugin.toml"),
                    MANIFEST.replace("1.0.0", "2.0.0"),
                )
                .unwrap();
            }
            if let Some(failure) = self.failure {
                output.failure = Some(failure);
                output.exit_code = if failure == CommandFailure::ExitFailure {
                    Some(7)
                } else {
                    None
                };
                output.stderr = b"build failed".to_vec();
            } else {
                fs::write(command.directory.join("artifact.txt"), b"built").unwrap();
            }
        }
        output
    }
}
/// The rule after any install, update, or uninstall settles: no fetched source waits, and the
/// only source folders in the store are those the catalog registers.
fn assert_only_registered_sources(registry: &PluginRegistry) {
    assert!(!registry.paths.pending_source().exists());
    let mut found = vec![];
    for plugin in fs::read_dir(registry.root().canonicalize().unwrap()).unwrap() {
        let plugin = plugin.unwrap().path();
        if let Ok(sources) = fs::read_dir(plugin.join(crate::data_root::PluginStorePaths::SOURCES))
        {
            found.extend(sources.map(|source| source.unwrap().path()));
        }
    }
    found.sort();
    let mut registered: Vec<PathBuf> = registry
        .list(None)
        .unwrap()
        .plugins
        .into_iter()
        .filter(|entry| entry.source.is_some())
        .map(|entry| PathBuf::from(entry.plugin_root))
        .collect();
    registered.sort();
    assert_eq!(found, registered);
}
/// The recorded cleanup key of a registered managed entry's source folder.
fn generation_key(registry: &PluginRegistry, entry: &PluginRegistryEntry) -> String {
    let relative = Path::new(&entry.plugin_root)
        .strip_prefix(registry.root().canonicalize().unwrap())
        .unwrap()
        .to_str()
        .unwrap()
        .replace('\\', "/");
    assert!(Generation::parse(&relative).is_some());
    relative
}
fn install(registry: &PluginRegistry, n: u8, executor: &mut Executor) -> PluginProvisionReport {
    registry.install(
        PluginGithubSource::parse("example/tools").unwrap(),
        Some("main".into()),
        &format!("{n:032x}"),
        executor,
    )
}
#[test]
fn plugin_managed_install_replace_and_uninstall_preserve_user_directories() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let first = install(&registry, 1, &mut Executor::default());
    assert_eq!(first.failure, None);
    assert!(first.changed);
    assert!(first.cleanup_pending.is_empty());
    let original = first.plugin.unwrap();
    assert!(
        Path::new(&original.plugin_root)
            .join("artifact.txt")
            .is_file()
    );
    let config = registry.config_dir("tools").unwrap();
    fs::write(config.join("user.txt"), "keep").unwrap();
    registry
        .mutate(crate::PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: false,
        })
        .unwrap();
    let second = install(&registry, 2, &mut Executor::default());
    assert_eq!(second.failure, None);
    assert!(!second.plugin.as_ref().unwrap().enabled);
    assert!(!Path::new(&original.plugin_root).exists());
    let removed = registry.uninstall("example/tools", &mut Executor::default());
    assert_eq!(removed.failure, None);
    assert!(removed.changed);
    assert!(registry.list(None).unwrap().plugins.is_empty());
    assert!(!Path::new(&second.plugin.unwrap().plugin_root).exists());
    assert_eq!(fs::read(config.join("user.txt")).unwrap(), b"keep");
}
#[test]
fn plugin_managed_update_reuses_recorded_source_and_skips_an_unchanged_commit() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let original = install(&registry, 1, &mut Executor::default())
        .plugin
        .unwrap();
    let config = registry.config_dir("tools").unwrap();
    fs::write(config.join("user.txt"), "keep").unwrap();
    registry
        .mutate(crate::PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: false,
        })
        .unwrap();
    let mut executor = Executor {
        commit: Some('b'),
        ..Executor::default()
    };
    let updated = registry.update("tools", &format!("{:032x}", 2), &mut executor);
    assert_eq!(updated.failure, None);
    assert!(updated.changed);
    assert_eq!(executor.fetched, ["main"]);
    let entry = updated.plugin.unwrap();
    assert!(!entry.enabled);
    let source = entry.source.as_ref().unwrap();
    assert_eq!(source.resolved_commit, "b".repeat(40));
    assert_eq!(source.requested_ref.as_deref(), Some("main"));
    assert!(!Path::new(&original.plugin_root).exists());
    assert_eq!(fs::read(config.join("user.txt")).unwrap(), b"keep");
    let current = registry.list(None).unwrap();
    let unchanged = registry.update(
        "tools",
        &format!("{:032x}", 3),
        &mut Executor {
            commit: Some('b'),
            ..Executor::default()
        },
    );
    assert_eq!(unchanged.failure, None);
    assert!(!unchanged.changed);
    assert_eq!(unchanged.stage, PluginProvisionStage::Done);
    assert!(unchanged.cleanup_pending.is_empty());
    assert_eq!(registry.list(None).unwrap(), current);
    let renamed = registry.update(
        "tools",
        &format!("{:032x}", 4),
        &mut Executor {
            commit: Some('c'),
            manifest: Some(MANIFEST.replace("id='tools'", "id='other'")),
            ..Executor::default()
        },
    );
    assert_eq!(
        renamed.failure,
        Some(PluginProvisionFailure::IdentityChanged)
    );
    assert_eq!(registry.list(None).unwrap(), current);
    assert_only_registered_sources(&registry);
    assert_eq!(
        registry
            .update("absent", &format!("{:032x}", 5), &mut Executor::default())
            .failure,
        Some(PluginProvisionFailure::Registry(Error::PluginNotFound))
    );
}
#[test]
fn plugin_update_refuses_a_linked_plugin_without_running_commands() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    fs::write(fixture.0.join("orchbee-plugin.toml"), MANIFEST).unwrap();
    registry
        .mutate(crate::PluginRegistryMutation::Link {
            path: fixture.0.clone(),
            enabled: true,
        })
        .unwrap();
    let before = registry.list(None).unwrap();
    let mut executor = Executor::default();
    let refused = registry.update("tools", &format!("{:032x}", 1), &mut executor);
    assert_eq!(
        refused.failure,
        Some(PluginProvisionFailure::SourceUnrecorded)
    );
    assert!(!refused.changed);
    assert_eq!(executor.called, 0);
    assert_eq!(registry.list(None).unwrap(), before);
}
#[test]
fn plugin_managed_failed_or_changed_build_keeps_previous_catalog() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    assert_eq!(
        install(&registry, 1, &mut Executor::default()).failure,
        None
    );
    let original = registry.list(None).unwrap();
    for (n, mut executor) in [
        (
            2,
            Executor {
                failure: Some(CommandFailure::ExitFailure),
                ..Executor::default()
            },
        ),
        (
            3,
            Executor {
                mutate: true,
                ..Executor::default()
            },
        ),
    ] {
        let failed = install(&registry, n, &mut executor);
        assert!(!failed.changed);
        assert!(failed.failure.is_some());
        assert!(failed.cleanup_pending.is_empty());
        assert_eq!(registry.list(None).unwrap(), original);
        assert_only_registered_sources(&registry);
    }
}
#[test]
fn plugin_managed_uninstall_reports_committed_removal_and_recoverable_cleanup() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let first = install(&registry, 1, &mut Executor::default())
        .plugin
        .unwrap();
    // A later install may reuse a freed version name, so the stranded content is what must go.
    let stranded = Path::new(&first.plugin_root).join("stranded.txt");
    fs::write(&stranded, b"old").unwrap();
    let held = crate::private_filesystem::open_file(
        &Path::new(&first.plugin_root).join("artifact.txt"),
        false,
        false,
    )
    .unwrap();
    let removed = registry.uninstall("tools", &mut Executor::default());
    assert!(removed.changed);
    assert_eq!(
        removed.failure,
        Some(PluginProvisionFailure::CleanupIncomplete)
    );
    assert_eq!(removed.cleanup_pending.len(), 1);
    assert!(registry.list(None).unwrap().plugins.is_empty());
    drop(held);
    let recovered = install(&registry, 2, &mut Executor::default());
    assert_eq!(recovered.failure, None);
    assert!(recovered.cleanup_pending.is_empty());
    assert!(!stranded.exists());
    assert_only_registered_sources(&registry);
}
#[test]
fn plugin_managed_invalid_inputs_and_local_source_collision_never_replace_link() {
    assert!(PluginGithubSource::parse("example/tools/.github/plugin").is_ok());
    for invalid in [
        "https://github.com/o/r",
        "o/r/..",
        "o/r/NUL",
        "o/r/a:b",
        "o//r",
        "-o/r",
    ] {
        assert!(PluginGithubSource::parse(invalid).is_err());
    }
    for invalid in ["--upload-pack=x", "HEAD:elsewhere", "a..b", "x y"] {
        assert!(!plugin_git_reference(invalid));
    }
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let mut executor = Executor::default();
    let bad = registry.install(
        PluginGithubSource {
            repository: "bad".into(),
            subdir: None,
        },
        None,
        &"1".repeat(32),
        &mut executor,
    );
    assert!(!registry.root().exists());
    assert!(bad.failure.is_some());
    assert_eq!(executor.called, 0);
    fs::write(fixture.0.join("orchbee-plugin.toml"), MANIFEST).unwrap();
    registry
        .mutate(crate::PluginRegistryMutation::Link {
            path: fixture.0.clone(),
            enabled: true,
        })
        .unwrap();
    let before = registry.list(None).unwrap();
    assert_eq!(
        install(&registry, 1, &mut executor).failure,
        Some(PluginProvisionFailure::LinkedConflict)
    );
    assert_eq!(registry.list(None).unwrap(), before);
    assert_eq!(
        registry
            .uninstall("tools", &mut Executor::default())
            .failure,
        None
    );
    assert!(fixture.0.join("orchbee-plugin.toml").exists());
}
#[test]
fn plugin_managed_uncertain_child_fences_cleanup_and_later_effects() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let failed = install(
        &registry,
        1,
        &mut Executor {
            failure: Some(CommandFailure::PhysicalUnknown),
            ..Executor::default()
        },
    );
    assert!(!failed.changed);
    assert_eq!(failed.cleanup_pending.len(), 1);
    let mut executor = Executor::default();
    let refused = install(&registry, 2, &mut executor);
    assert_eq!(
        refused.failure,
        Some(PluginProvisionFailure::PhysicalUnknown)
    );
    assert_eq!(executor.called, 0);
}

#[test]
fn plugin_managed_recovery_preserves_current_references_and_rejects_redirected_roots() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let entry = install(&registry, 1, &mut Executor::default())
        .plugin
        .unwrap();
    let mut cache = PrivateCache::acquire(registry.root(), "registry").unwrap();
    let mut catalog = decode(
        cache.read(FILE, REGISTRY_LIMIT).unwrap().as_deref(),
        &registry.paths,
    )
    .unwrap();
    catalog.cleanup.push(generation_key(&registry, &entry));
    cache
        .replace(FILE, &serde_json::to_vec(&catalog).unwrap())
        .unwrap();
    drop(cache);
    let missing = registry.uninstall("absent", &mut Executor::default());
    assert_eq!(
        missing.failure,
        Some(PluginProvisionFailure::Registry(Error::PluginNotFound))
    );
    assert!(missing.cleanup_pending.is_empty());
    assert!(Path::new(&entry.plugin_root).exists());
    let mut cache = PrivateCache::acquire(registry.root(), "registry").unwrap();
    let mut catalog = decode(
        cache.read(FILE, REGISTRY_LIMIT).unwrap().as_deref(),
        &registry.paths,
    )
    .unwrap();
    // A managed root outside its plugin's source folder would let cleanup or a build reach it.
    let outside = fixture.0.canonicalize().unwrap();
    catalog.plugins[0].plugin_root = outside.to_str().unwrap().to_owned();
    catalog.plugins[0].manifest_path = outside
        .join("orchbee-plugin.toml")
        .to_str()
        .unwrap()
        .to_owned();
    cache
        .replace(FILE, &serde_json::to_vec(&catalog).unwrap())
        .unwrap();
    drop(cache);
    assert_eq!(registry.list(None).unwrap_err(), Error::InvalidRegistry);
    assert_eq!(
        registry
            .uninstall("tools", &mut Executor::default())
            .failure,
        Some(PluginProvisionFailure::Registry(Error::InvalidRegistry))
    );
    assert!(Path::new(&entry.plugin_root).exists());
}

#[test]
fn plugin_incompatible_catalog_preserves_bytes_and_refuses_mutation() {
    for fault in [
        "folders",
        "cleanup",
        "blockedCleanup",
        "source",
        "description",
        "build",
    ] {
        let fixture = Fixture::new();
        let registry = fixture.registry();
        fs::write(fixture.0.join("orchbee-plugin.toml"), MANIFEST).unwrap();
        registry
            .mutate(crate::PluginRegistryMutation::Link {
                path: fixture.0.clone(),
                enabled: true,
            })
            .unwrap();
        let mut cache = PrivateCache::acquire(registry.root(), "registry").unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&cache.read(FILE, REGISTRY_LIMIT).unwrap().unwrap()).unwrap();
        assert_eq!(document["version"], 1);
        match fault {
            "source" => {
                document["plugins"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove(fault);
            }
            "description" | "build" => {
                document["plugins"][0]["manifest"]
                    .as_object_mut()
                    .unwrap()
                    .remove(fault);
            }
            _ => {
                document.as_object_mut().unwrap().remove(fault);
            }
        }
        let encoded = serde_json::to_vec(&document).unwrap();
        cache.replace(FILE, &encoded).unwrap();
        drop(cache);
        assert_eq!(registry.list(None), Err(Error::InvalidRegistry));
        assert_eq!(
            registry.mutate(crate::PluginRegistryMutation::SetEnabled {
                id: "tools".into(),
                enabled: false,
            }),
            Err(Error::InvalidRegistry)
        );
        assert_eq!(
            PrivateCache::read_existing(registry.root(), FILE, REGISTRY_LIMIT)
                .unwrap()
                .unwrap(),
            encoded
        );
        assert!(fixture.0.join("orchbee-plugin.toml").exists());
    }
}
