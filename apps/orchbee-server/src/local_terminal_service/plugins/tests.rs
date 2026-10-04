use super::*;
use orchbee_local_site_runtime::PluginRegistryMutation;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!(
            "orchbee-plugin-resolve-{:032x}",
            u128::from_be_bytes(nonce)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-plugin-resolve-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn plugin_resolution_checks_availability_platform_ambiguity_and_freezes_argv_context() {
    let root = Root::new();
    let registry = PluginRegistry::at(root.0.join("catalog")).unwrap();
    let source = "id='tools'\nname='Tools'\nversion='1'\n[[actions]]\nid='hello'\ntitle='Hello'\ncommand=['cmd.exe','/d','/c','echo hello']\n[[link_handlers]]\nid='web'\ntitle='Web'\npattern='^https://'\naction='hello'\n";
    let path = root.0.join("orchbee-plugin.toml");
    fs::write(&path, source).unwrap();
    registry
        .mutate(PluginRegistryMutation::Link {
            path: path.clone(),
            enabled: false,
        })
        .unwrap();
    let context = PluginInvocationContext {
        selected_text: Some("你好".into()),
        ..Default::default()
    };
    assert_eq!(
        resolve(&registry, None, "hello", &context, None),
        Err(Failure::Disabled)
    );
    registry
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: true,
        })
        .unwrap();
    let intent = resolve(&registry, None, "tools.hello", &context, None).unwrap();
    assert_eq!(intent.argv, vec!["cmd.exe", "/d", "/c", "echo hello"]);
    assert_eq!(
        serde_json::from_str::<PluginInvocationContext>(
            &intent
                .environment
                .iter()
                .find(|(name, _)| name == "ORCHBEE_PLUGIN_CONTEXT_JSON")
                .unwrap()
                .1
        )
        .unwrap(),
        context
    );
    for name in ["ORCHBEE_PLUGIN_CONFIG_DIR", "ORCHBEE_PLUGIN_STATE_DIR"] {
        assert!(
            PathBuf::from(
                &intent
                    .environment
                    .iter()
                    .find(|(n, _)| n == name)
                    .unwrap()
                    .1
            )
            .is_dir()
        );
    }
    fs::write(
        &path,
        source.replace("title='Hello'", "title='Hello'\nplatforms=['linux']"),
    )
    .unwrap();
    assert_eq!(
        resolve(&registry, None, "hello", &context, None),
        Err(Failure::Unsupported)
    );
    fs::write(&path, source).unwrap();
    let other = root.0.join("other.toml");
    fs::write(
        &other,
        source
            .replace("id='tools'", "id='other'")
            .replace("pattern='^https://'", "pattern='^other://'"),
    )
    .unwrap();
    registry
        .mutate(PluginRegistryMutation::Link {
            path: other,
            enabled: true,
        })
        .unwrap();
    assert_eq!(
        resolve(&registry, None, "hello", &context, None),
        Err(Failure::Ambiguous)
    );
    assert!(resolve(&registry, Some("tools"), "hello", &context, None).is_ok());
    let registered = registry.list_registered().unwrap();
    let guard = orchbee_workbench_protocol::PluginLinkCatalogGuard {
        revision: registered.revision,
        fingerprint: registered.fingerprint,
    };
    let link_context = PluginInvocationContext {
        invocation_source: Some("link_click".into()),
        clicked_url: Some("https://example.test".into()),
        link_handler_id: Some("web".into()),
        ..context.clone()
    };
    fs::write(&path, "bad=true").unwrap();
    assert!(
        resolve(
            &registry,
            Some("tools"),
            "hello",
            &link_context,
            Some(&guard)
        )
        .is_ok(),
        "guarded link invocation must not refresh the linked source"
    );
    let mut wrong_handler = link_context.clone();
    wrong_handler.link_handler_id = Some("other".into());
    assert_eq!(
        resolve(
            &registry,
            Some("tools"),
            "hello",
            &wrong_handler,
            Some(&guard)
        ),
        Err(Failure::Conflict)
    );
    assert_eq!(
        resolve(&registry, Some("tools"), "hello", &context, None),
        Err(Failure::SourceUnavailable)
    );
    fs::write(
        &path,
        source.replace("pattern='^https://'", "pattern='^other://'"),
    )
    .unwrap();
    registry
        .mutate(PluginRegistryMutation::Link {
            path,
            enabled: true,
        })
        .unwrap();
    assert_eq!(
        resolve(
            &registry,
            Some("tools"),
            "hello",
            &link_context,
            Some(&guard)
        ),
        Err(Failure::Conflict),
        "a newly published registered catalog invalidates the old guard"
    );
}
#[test]
fn plugin_server_dispatch_pins_worker_and_preserves_transport_uncertainty() {
    use crate::{
        worker_obcn_carrier::create_worker_carrier,
        worker_registry::{WorkerAdmission, WorkerRegistry},
    };
    use orchbee_local_ipc::LocalStream;
    use orchbee_workbench_protocol::{SiteId, WorkbenchRequestId};
    for lost in [false, true] {
        let workers = Arc::new(Mutex::new(WorkerRegistry::new()));
        let local = orchbee_local_site_protocol::WorkerId::from_bytes([7; 16]);
        let WorkerAdmission::Accepted { generation } = workers.lock().unwrap().admit(local) else {
            panic!()
        };
        let owner = WorkerRegistry::register_carrier(
            &workers,
            generation,
            create_worker_carrier(local, generation),
        )
        .unwrap();
        let (server, mut peer) = LocalStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let wire = thread::spawn(move || owner.run(server));
        let site = SiteId::parse(&"2".repeat(32)).unwrap();
        let service = LocalTerminalService::from_identity(workers, Ok(site.clone()));
        let driver = thread::spawn(move || {
            let node::CapabilityNodeMessage::PluginJobRequest(request) =
                node::read_message(&mut peer).unwrap()
            else {
                panic!()
            };
            assert!(matches!(
                request.action,
                node::PluginJobAction::Cancel { .. }
            ));
            if lost {
                return;
            }
            node::write_message(
                &mut peer,
                &node::CapabilityNodeMessage::PluginJobResult(node::PluginJobResult {
                    operation_id: request.operation_id,
                    worker_id: request.worker_id,
                    worker_generation: request.worker_generation,
                    outcome: node::PluginJobOutcome::Cancellation {
                        already_complete: false,
                    },
                }),
            )
            .unwrap();
        });
        let request = PluginRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
            site_id: site,
            action: PluginAction::Cancel {
                worker: PluginWorker {
                    worker_id: WorkerId::parse(&hex(local.as_bytes())).unwrap(),
                    generation: WorkbenchRevision::new(generation.get() + 1),
                },
                job_id: "4".repeat(32),
            },
        };
        assert_eq!(
            service.execute_plugin(request.clone()).outcome,
            Outcome::Failed {
                reason: Failure::WorkerChanged
            }
        );
        let mut request = request;
        if let PluginAction::Cancel { worker, .. } = &mut request.action {
            worker.generation = WorkbenchRevision::new(generation.get());
        }
        let result = service.execute_plugin(request.clone());
        assert!(result.answers(&request));
        assert!(if lost {
            matches!(result.outcome, Outcome::Unknown { .. })
        } else {
            matches!(
                result.outcome,
                Outcome::Cancellation {
                    already_complete: false,
                    ..
                }
            )
        });
        driver.join().unwrap();
        drop(service);
        wire.join().unwrap();
    }
}
