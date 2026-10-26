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
        if command.argv.iter().any(|a| a == "checkout") {
            fs::write(command.directory.join("orchbee-plugin.toml"), MANIFEST).unwrap();
        }
        if command.argv.iter().any(|a| a == "rev-parse") {
            output.stdout = "a".repeat(40).into_bytes();
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
        assert!(
            !registry
                .root
                .join("managed")
                .join(format!("i-{n:032x}"))
                .exists()
        );
    }
}
#[test]
fn plugin_managed_uninstall_reports_committed_removal_and_recoverable_cleanup() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let first = install(&registry, 1, &mut Executor::default())
        .plugin
        .unwrap();
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
    assert!(!Path::new(&first.plugin_root).exists());
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
    assert!(!registry.root.exists());
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
fn plugin_managed_recovery_preserves_current_references_and_rejects_redirected_provenance() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let entry = install(&registry, 1, &mut Executor::default())
        .plugin
        .unwrap();
    let mut cache = PrivateCache::acquire(&registry.root).unwrap();
    let mut catalog = decode(
        cache.read(FILE, REGISTRY_LIMIT).unwrap().as_deref(),
        &registry.root,
    )
    .unwrap();
    catalog.cleanup.push(format!("{:032x}", 1));
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
    let mut cache = PrivateCache::acquire(&registry.root).unwrap();
    let mut catalog = decode(
        cache.read(FILE, REGISTRY_LIMIT).unwrap().as_deref(),
        &registry.root,
    )
    .unwrap();
    catalog.plugins[0].source.as_mut().unwrap().installation_id = "3".repeat(32);
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
        "version",
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
        let mut cache = PrivateCache::acquire(&registry.root).unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&cache.read(FILE, REGISTRY_LIMIT).unwrap().unwrap()).unwrap();
        assert_eq!(document["version"], 1);
        match fault {
            "version" => document["version"] = serde_json::json!(2),
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
            PrivateCache::read_existing(&registry.root, FILE, REGISTRY_LIMIT)
                .unwrap()
                .unwrap(),
            encoded
        );
        assert!(fixture.0.join("orchbee-plugin.toml").exists());
    }
}
