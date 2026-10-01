//! Real native CLI processes against admitted configured-command peers.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};
#[path = "cli_windows_configured_commands/events.rs"]
mod events;

#[test]
fn configured_command_cli_catalog_refusal_and_delivery_unknown_never_replay() {
    for mode in [
        "list",
        "accepted",
        "stale",
        "failed",
        "lost",
        "wrong-action",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::ConfiguredCommand(catalog) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("catalog")
            };
            assert!(matches!(catalog.action, ConfiguredCommandAction::List {}));
            let entries = if mode == "stale" {
                vec![]
            } else {
                vec![ConfiguredCommandDescriptor {
                    id: "cmd_profile_0".into(),
                    action: ConfiguredCommandKind::Shell,
                    bindings: vec![],
                    description: Some("Background command".into()),
                    width: None,
                    height: None,
                }]
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::ConfiguredCommand(ConfiguredCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: catalog.request_id,
                    site_id: catalog.site_id,
                    outcome: ConfiguredCommandOutcome::Catalog {
                        revision: WorkbenchRevision::new(1),
                        entries,
                    },
                }),
            )
            .unwrap();
            if matches!(mode, "list" | "stale") {
                assert_closed(&mut stream);
                return;
            }
            let WorkbenchRequest::ClientView(list) = read_workbench_request(&mut stream).unwrap()
            else {
                panic!("client lookup")
            };
            assert!(matches!(list.action, ClientViewAction::List {}));
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::ClientView(WorkbenchClientViewResult {
                    protocol_version: ProtocolVersion,
                    request_id: list.request_id,
                    site_id: list.site_id,
                    outcome: ClientViewOutcome::Listed { clients: vec![] },
                }),
            )
            .unwrap();
            let WorkbenchRequest::ConfiguredCommand(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("invoke")
            };
            let ConfiguredCommandAction::Invoke { invocation } = &request.action else {
                panic!()
            };
            assert_eq!(invocation.command_id, "cmd_profile_0");
            assert!(invocation.source.is_none());
            assert!(invocation.selection.is_none());
            if mode == "lost" {
                return;
            }
            let outcome = match mode {
                "failed" => ConfiguredCommandOutcome::Failed {
                    reason: ConfiguredCommandFailure::NotFound,
                },
                "wrong-action" => ConfiguredCommandOutcome::Catalog {
                    revision: WorkbenchRevision::new(1),
                    entries: vec![],
                },
                _ => ConfiguredCommandOutcome::ShellStarted {
                    command_id: invocation.command_id.clone(),
                    worker: PluginWorker {
                        worker_id: WorkerId::parse(&"3".repeat(32)).unwrap(),
                        generation: WorkbenchRevision::new(1),
                    },
                    job_id: request.request_id.clone(),
                    existing: false,
                },
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::ConfiguredCommand(ConfiguredCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let output = if mode == "list" {
            fixture.run(&["command", "list", "--format=json"])
        } else {
            fixture.run(&["command", "invoke", "cmd_profile_0", "--format=json"])
        };
        assert_eq!(
            output.status.code(),
            Some(match mode {
                "stale" => 3,
                "failed" => 3,
                "lost" | "wrong-action" => 8,
                _ => 0,
            }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        if mode == "stale" {
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.cli.failure.command.json"),
            );
        } else {
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.command.json"),
            );
        }
        if matches!(mode, "lost" | "wrong-action") {
            assert!(!document["result"]["complete"].as_bool().unwrap());
            assert!(document["result"]["reply"].is_null());
        }
        handler.join().unwrap();
    }
}
