//! Actual Windows CLI owner exchange, retained reload ID and localized output.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};
#[test]
fn cli_manifest_query_reload_status_and_lost_receipts_are_not_replayed() {
    for case in [
        "current",
        "reload",
        "status",
        "lost",
        "wrong-operation",
        "denied",
        "en",
        "zh",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::AgentManifests(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("manifest request")
            };
            if case == "lost" {
                assert!(matches!(
                    request.action,
                    AgentManifestsAction::Reload { .. }
                ));
                return;
            }
            let outcome = match &request.action {
                AgentManifestsAction::Current {} => {
                    let corpus: Value = serde_json::from_str(include_str!("../../../schemas/workbench/v1/workbench-agent-manifests-conformance-v1.json")).unwrap();
                    let mut outcome = corpus["valid"][0]["document"]["outcome"].clone();
                    outcome["snapshot"]["configurationRoot"] = json!("C:\\rules\u{202e}");
                    serde_json::from_value(outcome).unwrap()
                }
                AgentManifestsAction::Reload { operation_id }
                | AgentManifestsAction::Operation { operation_id } => {
                    assert_eq!(operation_id.as_str(), "11111111111111111111111111111111");
                    if case == "denied" {
                        AgentManifestsOutcome::Failed {
                            reason: AgentManifestsFailure::Denied,
                        }
                    } else {
                        AgentManifestsOutcome::Operation {
                            operation_id: if case == "wrong-operation" {
                                WorkbenchRequestId::parse("22222222222222222222222222222222")
                                    .unwrap()
                            } else {
                                operation_id.clone()
                            },
                            status: if case == "reload" {
                                AgentManifestsOperation::Pending {}
                            } else {
                                AgentManifestsOperation::Applied {
                                    revision: WorkbenchRevision::new(2),
                                }
                            },
                        }
                    }
                }
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::AgentManifests(AgentManifestsResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome,
                }),
            )
            .unwrap();
            if case == "reload" {
                let WorkbenchRequest::AgentManifests(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("status only")
                };
                let AgentManifestsAction::Operation { operation_id } = request.action else {
                    panic!("never reload twice")
                };
                assert_eq!(operation_id.as_str(), "11111111111111111111111111111111");
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::AgentManifests(AgentManifestsResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        site_id: request.site_id,
                        outcome: AgentManifestsOutcome::Operation {
                            operation_id,
                            status: AgentManifestsOperation::Applied {
                                revision: WorkbenchRevision::new(2),
                            },
                        },
                    }),
                )
                .unwrap();
            }
            assert_closed(&mut stream);
        });
        let text = matches!(case, "en" | "zh");
        let mut args = vec!["site"];
        match case {
            "current" | "en" | "zh" => args.push("agent-manifests"),
            "status" => args.extend(["agent-manifests-status", "11111111111111111111111111111111"]),
            _ => args.extend([
                "reload-agent-manifests",
                "--request-id",
                "11111111111111111111111111111111",
            ]),
        }
        args.extend(["--format", if text { "text" } else { "json" }]);
        let output = fixture.run_with_env(
            &args,
            None,
            None,
            &[(
                "LC_ALL",
                std::ffi::OsStr::new(if case == "zh" {
                    "zh_CN.UTF-8"
                } else {
                    "en_US.UTF-8"
                }),
            )],
        );
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(match case {
                "lost" | "wrong-operation" => 8,
                "denied" => 3,
                _ => 0,
            }),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if text {
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(
                text.contains(if case == "zh" {
                    "规则修订"
                } else {
                    "Manifest revision"
                }),
                "{text}"
            );
            assert!(!text.contains('\u{202e}'));
            assert!(text.contains("\\u{202e}"));
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.site.agent-manifests.json"),
            );
            if matches!(case, "lost" | "wrong-operation") {
                assert_eq!(document["observation"]["kind"], "unknown");
                assert_eq!(
                    document["action"]["operationId"],
                    "11111111111111111111111111111111"
                );
            }
        }
    }
}
