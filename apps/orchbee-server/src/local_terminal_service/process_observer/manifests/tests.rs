mod automatic;

mod update;

use super::*;
#[test]
fn configuration_apply_changes_live_schedule_but_refuses_active_or_stopped_maintenance() {
    let registry = Registry::default();
    let startup = UserSettings::default();
    registry
        .initialize_settings(
            &startup,
            &ScreenDetector::bundled().unwrap(),
            Ok(None),
            true,
            true,
        )
        .unwrap();
    let disabled = UserSettings {
        manifest_check: false,
        manifest_check_source: "file",
        ..startup.clone()
    };
    assert!(registry.apply_configuration(&disabled));
    {
        let mut slot = registry.0.lock().unwrap();
        let state = slot.as_mut().unwrap();
        assert!(!state.schedule.as_ref().unwrap().project(true).enabled);
        state.active = Some(Owner::Automatic);
    }
    assert!(!registry.apply_configuration(&startup));
    {
        let mut slot = registry.0.lock().unwrap();
        let state = slot.as_mut().unwrap();
        assert!(!state.schedule.as_ref().unwrap().project(true).enabled);
        state.active = None;
    }
    assert!(registry.apply_configuration(&startup));
    assert!(
        registry
            .0
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .schedule
            .as_ref()
            .unwrap()
            .project(true)
            .enabled
    );
    {
        let _maintenance = registry.0.lock().unwrap();
        assert!(
            !registry.apply_configuration(&disabled),
            "configuration admission must not wait behind manifest maintenance"
        );
    }
    registry.cancel();
    assert!(!registry.apply_configuration(&disabled));
}
fn id(n: u64) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
#[test]
fn reload_receipts_survive_duplicates_failures_and_stop_without_replaying() {
    let registry = Registry::default();
    registry
        .initialize(None, &ScreenDetector::bundled().unwrap())
        .unwrap();
    let action = Action::Reload {
        operation_id: id(1),
    };
    let pending = registry.execute(&action);
    assert_eq!(registry.execute(&action), pending);
    assert_eq!(
        registry.execute(&Action::Reload {
            operation_id: id(2)
        }),
        failed(Failure::Busy)
    );
    assert!(registry.take().is_some());
    assert!(registry.take().is_none());
    registry.complete(id(1), Ok(ScreenDetector::bundled().unwrap()), |_| Ok(()));
    let applied = registry.execute(&action);
    assert!(
        matches!(applied, Outcome::Operation { status: Operation::Applied { revision }, .. } if revision.value() == 2)
    );
    assert_eq!(registry.execute(&action), applied);
    assert!(registry.take().is_none());
    registry.execute(&Action::Reload {
        operation_id: id(2),
    });
    registry.complete(id(2), Err("bad bundle".into()), |_| {
        panic!("must retain old engine")
    });
    assert!(
        matches!(registry.execute(&Action::Current {}), Outcome::Current { snapshot } if snapshot.revision.value() == 2 && snapshot.manifests.len() == 22)
    );
    registry.execute(&Action::Reload {
        operation_id: id(3),
    });
    registry.take().unwrap();
    registry.cancel();
    registry.complete(id(3), Ok(ScreenDetector::bundled().unwrap()), |_| {
        panic!("stop forbids install")
    });
    assert!(matches!(
        registry.execute(&Action::Operation {
            operation_id: id(3)
        }),
        Outcome::Operation {
            status: Operation::Failed {
                reason: Failure::Cancelled
            },
            ..
        }
    ));
    assert_eq!(
        registry.execute(&Action::Operation {
            operation_id: id(4)
        }),
        failed(Failure::NotFound)
    );
}
#[test]
fn failed_install_and_full_receipt_ledger_preserve_the_installed_snapshot() {
    let registry = Registry::default();
    registry
        .initialize(None, &ScreenDetector::bundled().unwrap())
        .unwrap();
    let before = registry.execute(&Action::Current {});
    registry.execute(&Action::Reload {
        operation_id: id(1),
    });
    registry.take().unwrap();
    registry.complete(id(1), Ok(ScreenDetector::bundled().unwrap()), |_| {
        Err(Failure::Unavailable)
    });
    assert_eq!(registry.execute(&Action::Current {}), before);
    let settled = registry.execute(&Action::Operation {
        operation_id: id(1),
    });
    assert!(matches!(
        settled,
        Outcome::Operation {
            status: Operation::Failed {
                reason: Failure::Unavailable
            },
            ..
        }
    ));
    {
        let mut lock = registry.0.lock().unwrap();
        let state = lock.as_mut().unwrap();
        for n in 2..=RECEIPTS as u64 {
            state.operations.insert(
                id(n),
                Receipt {
                    kind: Kind::Reload,
                    status: Operation::Failed {
                        reason: Failure::LoadFailed,
                    },
                },
            );
        }
    }
    assert_eq!(
        registry.execute(&Action::Reload {
            operation_id: id(RECEIPTS as u64 + 1)
        }),
        failed(Failure::ResourceLimit)
    );
    assert_eq!(
        registry.execute(&Action::Reload {
            operation_id: id(1)
        }),
        settled
    );
    assert!(registry.take().is_none());
}
#[test]
fn reload_installs_changed_rules_once_from_the_original_root() {
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let path = self.0.canonicalize().unwrap();
            let parent = std::env::temp_dir().canonicalize().unwrap();
            assert_eq!(path.parent(), Some(parent.as_path()));
            assert!(
                path.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchbee-manifest-reload-")
            );
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = Root(std::env::temp_dir().join(format!(
        "orchbee-manifest-reload-{:032x}",
        u128::from_le_bytes(nonce)
    )));
    std::fs::create_dir(&root.0).unwrap();
    let mut installed =
        ScreenDetector::load(Some(&root.0), crate::agent_screen::RemoteCache::Ignored).unwrap();
    let registry = Registry::default();
    registry
        .initialize(Some(root.0.clone()), &installed)
        .unwrap();
    let rules = root.0.join("agent-detection");
    std::fs::create_dir(&rules).unwrap();
    std::fs::write(
        rules.join("codex.toml"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/agent-detection/codex.toml"
        ))
        .replace("version = \"2026.09.14.1\"", "version = \"0\""),
    )
    .unwrap();
    assert!(installed.summaries().iter().any(|m| m.agent == "codex"
        && m.manifest.source == orchbee_workbench_protocol::AgentManifestSource::Bundled));
    registry.execute(&Action::Reload {
        operation_id: id(1),
    });
    let work = registry.take().unwrap();
    registry.complete(
        match work.id {
            Owner::Manual(id) => id,
            Owner::Automatic => panic!("manual reload"),
        },
        ScreenDetector::load(
            work.root.as_deref(),
            updating::remote_cache(work.root.as_deref(), &work.source),
        ),
        |next| {
            installed = next;
            Ok(())
        },
    );
    let codex = installed
        .summaries()
        .into_iter()
        .find(|m| m.agent == "codex")
        .unwrap();
    assert_eq!(codex.version.as_deref(), Some("0"));
    assert_eq!(
        codex.manifest.source,
        orchbee_workbench_protocol::AgentManifestSource::LocalOverride
    );
    assert!(
        matches!(registry.execute(&Action::Current {}), Outcome::Current { snapshot } if snapshot.revision.value()==2 && snapshot.manifests.contains(&codex))
    );
    std::fs::write(rules.join("codex.toml"), "invalid").unwrap();
    registry.execute(&Action::Reload {
        operation_id: id(1),
    });
    assert!(registry.take().is_none());
    assert!(installed.summaries().contains(&codex));
}
