use super::*;
use orchbee_workbench_protocol::parse_agent_view_spec;
fn set(source: &str) -> AgentViewAction {
    AgentViewAction::Set {
        spec: Box::new(
            parse_agent_view_spec(
                &serde_json::to_vec(&serde_json::json!({"source":source})).unwrap(),
            )
            .unwrap(),
        ),
    }
}
#[test]
fn latest_source_replaces_guarded_clear_preserves_and_identical_set_is_noop() {
    let mut registry = Registry::default();
    let first = registry.apply(&set("first"), |_| panic!()).unwrap();
    assert_eq!(first.revision.value(), 1);
    assert_eq!(registry.apply(&set("first"), |_| panic!()).unwrap(), first);
    let next = registry.apply(&set("next"), |_| panic!()).unwrap();
    assert_eq!(next.revision.value(), 2);
    assert_eq!(
        registry
            .apply(
                &AgentViewAction::Clear {
                    source: Some("first".into())
                },
                |_| panic!()
            )
            .unwrap(),
        next
    );
    assert_eq!(
        registry
            .apply(
                &AgentViewAction::Clear {
                    source: Some(" next ".into())
                },
                |_| panic!()
            )
            .unwrap()
            .spec,
        None
    );
    assert_eq!(
        registry
            .apply(&AgentViewAction::Clear { source: None }, |_| panic!())
            .unwrap()
            .revision
            .value(),
        3
    );
}
#[test]
fn failed_set_or_plugin_inspection_preserves_and_disable_clears_on_read() {
    let mut registry = Registry::default();
    let prior = registry.apply(&set("user"), |_| panic!()).unwrap();
    for reason in [
        Failure::PluginDisabled,
        Failure::PluginNotFound,
        Failure::SourceUnavailable,
    ] {
        assert_eq!(
            registry.apply(&set("plugin:foo"), |_| Err(reason)),
            Err(reason)
        );
        assert_eq!(registry.snapshot, prior);
    }
    let plugin = registry.apply(&set("plugin:foo"), |_| Ok(())).unwrap();
    assert_eq!(
        registry.apply(&AgentViewAction::Read {}, |_| Err(
            Failure::SourceUnavailable
        )),
        Err(Failure::SourceUnavailable)
    );
    assert_eq!(registry.snapshot, plugin);
    let cleared = registry
        .apply(&AgentViewAction::Read {}, |_| Err(Failure::PluginDisabled))
        .unwrap();
    assert!(cleared.spec.is_none());
    assert_eq!(cleared.revision.value(), 3);
}
#[test]
fn invalid_spec_and_revision_exhaustion_never_commit() {
    let mut registry = Registry::default();
    let prior = registry.apply(&set("user"), |_| panic!()).unwrap();
    let AgentViewAction::Set { mut spec } = set("bad") else {
        panic!()
    };
    spec.sort = vec![
        orchbee_workbench_protocol::AgentViewSort {
            field: orchbee_workbench_protocol::AgentViewSortField::Builtin(
                orchbee_workbench_protocol::AgentViewBuiltinSortField::Seen
            ),
            order: orchbee_workbench_protocol::AgentViewSortOrder::Asc
        };
        9
    ];
    assert_eq!(
        registry.apply(&AgentViewAction::Set { spec }, |_| panic!()),
        Err(Failure::InvalidRequest)
    );
    assert_eq!(registry.snapshot, prior);
    registry.snapshot.revision = WorkbenchRevision::new(9_007_199_254_740_991);
    let prior = registry.snapshot.clone();
    assert_eq!(
        registry.apply(&set("new"), |_| panic!()),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(registry.snapshot, prior);
}

// The plugin store answers Unsupported off Windows.
#[cfg(windows)]
#[test]
fn plugin_view_lifecycle_reads_real_private_registry_without_creating_on_read() {
    use orchbee_local_runtime::{PluginRegistry, PluginRegistryMutation};
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let root = self.0.canonicalize().unwrap();
            let temporary = std::env::temp_dir().canonicalize().unwrap();
            assert!(root.starts_with(&temporary));
            assert!(
                root.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("orchbee-agent-view-")
            );
            std::fs::remove_dir_all(&root).unwrap();
        }
    }
    let root = Root(std::env::temp_dir().join(format!(
            "orchbee-agent-view-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir(&root.0).unwrap();
    let plugins = PluginRegistry::at(root.0.join("registry")).unwrap();
    let mut views = Registry::default();
    assert_eq!(
        views.apply(&set("plugin:tools"), |id| plugin_from_registry(
            &plugins, id
        )),
        Err(Failure::PluginNotFound)
    );
    assert!(
        !plugins.root().exists(),
        "read of a missing plugin cannot create a registry"
    );
    let path = root.0.join("orchbee-plugin.toml");
    std::fs::write(&path, "id='tools'\nname='Tools'\nversion='1'\n").unwrap();
    plugins
        .mutate(PluginRegistryMutation::Link {
            path: path.clone(),
            enabled: true,
        })
        .unwrap();
    let active = views
        .apply(&set("plugin:tools"), |id| {
            plugin_from_registry(&plugins, id)
        })
        .unwrap();
    assert!(active.spec.is_some());
    plugins
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: false,
        })
        .unwrap();
    let clear = views
        .apply(&AgentViewAction::Read {}, |id| {
            plugin_from_registry(&plugins, id)
        })
        .unwrap();
    assert!(clear.spec.is_none());
    plugins
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: true,
        })
        .unwrap();
    views
        .apply(&set("plugin:tools"), |id| {
            plugin_from_registry(&plugins, id)
        })
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let prior = views.snapshot.clone();
    assert_eq!(
        views.apply(&AgentViewAction::Read {}, |id| plugin_from_registry(
            &plugins, id
        )),
        Err(Failure::SourceUnavailable)
    );
    assert_eq!(views.snapshot, prior);
    plugins
        .mutate(PluginRegistryMutation::Unlink { id: "tools".into() })
        .unwrap();
    assert!(
        views
            .apply(&AgentViewAction::Read {}, |id| plugin_from_registry(
                &plugins, id
            ))
            .unwrap()
            .spec
            .is_none()
    );
}

// Linux and macOS withhold Agent view requests at admission (`withheld_off_windows`).
#[cfg(windows)]
#[test]
fn agent_view_permissions_precede_source_inspection_and_session_mismatch_preserves() {
    use crate::{local_terminal_service::LocalTerminalService, worker_registry::WorkerRegistry};
    use orchbee_workbench_protocol::*;
    use std::sync::{Arc, Mutex};
    let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    let service = |grants: u8| {
        WorkbenchService::new(
            super::super::WorkbenchPolicy {
                read: grants & 1 != 0,
                manage_navigation: grants & 2 != 0,
                associate_directory: grants & 4 != 0,
                create_terminal: grants & 8 != 0,
                manage_terminal_lifecycle: grants & 16 != 0,
            },
            terminals.clone(),
        )
    };
    let admin = service(31);
    let super::super::WorkbenchExecution::Response(WorkbenchResponse::Snapshot(result)) = admin
        .execute(WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        }))
    else {
        panic!()
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!()
    };
    let request = AgentViewRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"2".repeat(32)).unwrap(),
        session_id: snapshot.session_id,
        action: set("ordinary"),
    };
    for grants in 0..32 {
        let owned = service(grants);
        for action in [
            AgentViewAction::Read {},
            set("ordinary"),
            AgentViewAction::Clear { source: None },
        ] {
            let expected = AgentViewRequest {
                action,
                ..request.clone()
            };
            let permitted = if expected.action.mutating() {
                grants & 2 != 0
            } else {
                grants & 1 != 0
            };
            let super::super::WorkbenchExecution::Response(WorkbenchResponse::AgentView(result)) =
                owned.execute(WorkbenchRequest::AgentView(expected.clone()))
            else {
                panic!()
            };
            assert!(result.answers(&expected));
            assert_eq!(
                matches!(result.outcome, AgentViewOutcome::Available { .. }),
                permitted
            );
            if !permitted {
                assert_eq!(
                    result.outcome,
                    AgentViewOutcome::Failed {
                        reason: Failure::Denied
                    }
                );
            }
        }
        if grants & 2 == 0 {
            let super::super::WorkbenchExecution::Response(WorkbenchResponse::AgentView(result)) =
                owned.execute(WorkbenchRequest::AgentView(AgentViewRequest {
                    action: set("plugin:"),
                    ..request.clone()
                }))
            else {
                panic!()
            };
            assert_eq!(
                result.outcome,
                AgentViewOutcome::Failed {
                    reason: Failure::Denied
                }
            );
        }
    }
    let prior = admin.agent_view(&request);
    assert!(prior.answers(&request));
    let mut wrong = request.clone();
    wrong.session_id = SessionId::parse(&"f".repeat(32)).unwrap();
    wrong.action = AgentViewAction::Clear { source: None };
    assert_eq!(
        admin.agent_view(&wrong).outcome,
        AgentViewOutcome::Failed {
            reason: Failure::SessionMismatch
        }
    );
    let read = AgentViewRequest {
        action: AgentViewAction::Read {},
        ..request
    };
    assert_eq!(admin.agent_view(&read).outcome, prior.outcome);
}
