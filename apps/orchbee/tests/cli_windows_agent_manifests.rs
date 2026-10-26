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
        "automatic",
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
                    let mut outcome = if case == "automatic" {
                        corpus["valid"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|c| c["name"] == "automatic-updated")
                            .unwrap()["document"]["outcome"]
                            .clone()
                    } else {
                        corpus["valid"][0]["document"]["outcome"].clone()
                    };
                    outcome["snapshot"]["configurationRoot"] = json!("C:\\rules\u{202e}");
                    serde_json::from_value(outcome).unwrap()
                }
                AgentManifestsAction::Reload { operation_id }
                | AgentManifestsAction::Update { operation_id }
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
                    session_id: request.session_id,
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
                        session_id: request.session_id,
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
        let mut args = vec!["server"];
        match case {
            "current" | "automatic" | "en" | "zh" => args.push("agent-manifests"),
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
            if case == "automatic" {
                assert_eq!(document["version"], 1);
            }
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.server.agent-manifests.json"),
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

#[test]
fn cli_update_polls_only_status_and_reports_partial_or_unknown_effects() {
    for case in [
        "updated-and-activated",
        "partial-update-with-activation",
        "cancelled-cache-facts-no-activation",
        "update-effects-unknown",
        "wrong-kind",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::AgentManifests(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("manifest update")
            };
            let AgentManifestsAction::Update { operation_id } = request.action else {
                panic!("update action")
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::AgentManifests(AgentManifestsResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: AgentManifestsOutcome::Operation {
                        operation_id: operation_id.clone(),
                        status: AgentManifestsOperation::Pending {},
                    },
                }),
            )
            .unwrap();
            let WorkbenchRequest::AgentManifests(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("status")
            };
            assert_eq!(
                request.action,
                AgentManifestsAction::Operation {
                    operation_id: operation_id.clone()
                }
            );
            let corpus: Value = serde_json::from_str(include_str!(
                "../../../schemas/workbench/v1/workbench-agent-manifests-conformance-v1.json"
            ))
            .unwrap();
            let status = if case == "wrong-kind" {
                AgentManifestsOperation::Applied {
                    revision: WorkbenchRevision::new(2),
                }
            } else {
                let row = corpus["valid"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["name"] == case)
                    .unwrap();
                serde_json::from_value(row["document"]["outcome"]["status"].clone()).unwrap()
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::AgentManifests(AgentManifestsResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    session_id: request.session_id,
                    outcome: AgentManifestsOutcome::Operation {
                        operation_id,
                        status,
                    },
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let output = fixture.run_with_env(
            &[
                "server",
                "update-agent-manifests",
                "--request-id",
                "11111111111111111111111111111111",
                "--format",
                "json",
            ],
            None,
            None,
            &[],
        );
        handler.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(match case {
                "updated-and-activated" => 0,
                "partial-update-with-activation" => 1,
                "cancelled-cache-facts-no-activation" => 7,
                _ => 8,
            }),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["version"], 1);
        if case == "wrong-kind" {
            assert_eq!(
                document["observation"]["reply"]["outcome"]["status"]["kind"],
                "pending"
            );
        }
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.server.agent-manifests.json"),
        );
    }
}
