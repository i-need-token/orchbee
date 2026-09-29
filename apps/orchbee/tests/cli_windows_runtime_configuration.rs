//! Running-Site configuration CLI receipts through an actual Windows named-pipe peer.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_ipc::LocalListener;
use orchbee_workbench_protocol::*;
use serde_json::Value;
use support::{Fixture, assert_closed, bound_snapshot_reply, peer, validate};

#[test]
fn current_reload_rejections_and_lost_reply_have_closed_typed_receipts_without_replay() {
    for case in [
        "startup",
        "current",
        "reload",
        "restart",
        "denied",
        "invalid",
        "lost",
        "wrong-action",
        "en",
        "zh",
    ] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let reading = matches!(case, "startup" | "current");
        let handler = peer(&fixture, listener, move |mut stream| {
            bound_snapshot_reply(&mut stream);
            let WorkbenchRequest::Configuration(request) =
                read_workbench_request(&mut stream).unwrap()
            else {
                panic!("configuration");
            };
            assert_eq!(
                request.action,
                if reading {
                    ConfigurationAction::Current
                } else {
                    ConfigurationAction::Reload
                }
            );
            if case == "lost" {
                return;
            }
            let snapshot = ConfigurationSnapshot {
                revision: WorkbenchRevision::new(if case == "startup" { 0 } else { 3 }),
                keys_profile: "[keys]\nprefix='ctrl+a'\n".into(),
                restart_required: if case == "restart" {
                    vec![ConfigurationRestart::DefaultShell]
                } else {
                    Vec::new()
                },
                manifest_check: false,
            };
            let outcome = match case {
                "denied" => ConfigurationOutcome::failed(ConfigurationFailure::Denied),
                "invalid" => ConfigurationOutcome::Failed {
                    reason: ConfigurationFailure::LoadFailed,
                    field: Some("keys.help".into()),
                    detail: Some("invalid-keybinding".into()),
                },
                "startup" | "current" | "wrong-action" => {
                    ConfigurationOutcome::Current { snapshot }
                }
                _ => ConfigurationOutcome::Reloaded {
                    snapshot,
                    changed: true,
                },
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Configuration(ConfigurationResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let text = matches!(case, "en" | "zh");
        let output = fixture.run_with_env(
            &[
                "site",
                if reading { "config" } else { "reload-config" },
                "--format",
                if text { "text" } else { "json" },
            ],
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
                "lost" | "wrong-action" => 8,
                "denied" => 3,
                "invalid" => 1,
                _ => 0,
            }),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if text {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains(if case == "zh" {
                        "配置已发布"
                    } else {
                        "Configuration published"
                    })
            );
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.site.configuration.json"),
            );
            if matches!(case, "lost" | "wrong-action") {
                assert!(document["result"]["reply"].is_null());
            }
        }
    }
}

#[test]
fn reload_does_not_activate_a_site_and_rejects_unrelated_flags_before_admission() {
    let fixture = Fixture::new();
    let _rendezvous = fixture.rendezvous();
    let output = fixture.run(&["site", "reload-config", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for arguments in [
        vec!["site", "reload-config", "--workspace", "x"],
        vec!["site", "config", "extra"],
    ] {
        let output = fixture.run(&arguments);
        assert_eq!(output.status.code(), Some(2));
    }
}
