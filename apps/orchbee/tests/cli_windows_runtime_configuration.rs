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
                notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
                window_title: if case == "en" {
                    "{tab}\n\u{202e}"
                } else {
                    "{hostname}: {workspace}"
                }
                .into(),
                restart_required: if case == "restart" {
                    vec![ConfigurationRestart::ResumeAgents]
                } else {
                    Vec::new()
                },
                default_shell: if case == "en" {
                    "C:\\tools\\\u{202e}shell.exe".into()
                } else {
                    String::new()
                },
                shell_mode: orchbee_workbench_protocol::ConfigurationShellMode::Auto,
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
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(if case == "zh" {
                "配置已发布"
            } else {
                "Configuration published"
            }));
            assert!(text.contains("mode=auto"));
            if case == "zh" {
                assert!(text.contains("（平台默认）"));
                assert!(text.contains("窗口标题模板：\"{hostname}: {workspace}\""));
            } else {
                assert!(text.contains(r#"Window title template: "{tab}\n\u{202e}""#));
                assert!(text.contains(r"C:\\tools\\\u{202e}shell.exe"));
                assert!(!text.contains('\u{202e}'));
            }
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.site.configuration.json"),
            );
            assert_eq!(document["version"], 4);
            if matches!(case, "startup" | "current" | "reload" | "restart") {
                assert_eq!(
                    document["result"]["reply"]["outcome"]["snapshot"]["defaultShell"],
                    ""
                );
                assert_eq!(
                    document["result"]["reply"]["outcome"]["snapshot"]["shellMode"],
                    "auto"
                );
            }
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

#[test]
fn public_configuration_versions_keep_closed_independent_snapshots() {
    let schema: Value = serde_json::from_str(include_str!(
        "../src/cli/schemas/orchbee.site.configuration.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut document = serde_json::json!({
        "schema": "orchbee.site.configuration", "version": 1,
        "source": {"kind": "local-runtime"},
        "result": {"action": "current", "interrupted": false, "cleanupFailed": false,
            "reply": {"protocolVersion": 1, "requestId": "00000000000000000000000000000001", "siteId": "00000000000000000000000000000002",
                "outcome": {"kind": "current", "snapshot": {"revision": "0", "keysProfile": "[keys]", "manifestCheck": true, "restartRequired": []}}}}
    });
    assert!(validator.is_valid(&document));
    document["version"] = 2.into();
    assert!(!validator.is_valid(&document));
    let snapshot = &mut document["result"]["reply"]["outcome"]["snapshot"];
    snapshot["defaultShell"] = "pwsh.exe".into();
    snapshot["shellMode"] = "non_login".into();
    assert!(validator.is_valid(&document));
    document["version"] = 1.into();
    assert!(!validator.is_valid(&document));
    document["version"] = 3.into();
    assert!(!validator.is_valid(&document));
    document["result"]["reply"]["outcome"]["snapshot"]["windowTitle"] = "{tab}".into();
    assert!(validator.is_valid(&document));
    document["version"] = 4.into();
    assert!(!validator.is_valid(&document));
    document["result"]["reply"]["outcome"]["snapshot"]["notificationProfile"] = "[ui.toast]".into();
    assert!(validator.is_valid(&document));
    document["version"] = 3.into();
    assert!(!validator.is_valid(&document));
    document["result"]["reply"]["outcome"]["snapshot"]
        .as_object_mut()
        .unwrap()
        .remove("notificationProfile");
    assert!(validator.is_valid(&document));
    document["version"] = 2.into();
    assert!(!validator.is_valid(&document));
    document["result"]["reply"]["outcome"]["snapshot"]
        .as_object_mut()
        .unwrap()
        .remove("windowTitle");
    assert!(validator.is_valid(&document));
    document["result"]["reply"]["outcome"]["snapshot"]["shellMode"] = "unknown".into();
    assert!(!validator.is_valid(&document));
}
