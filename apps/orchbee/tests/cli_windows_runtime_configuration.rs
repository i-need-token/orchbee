//! Running-Session configuration CLI receipts through an actual Windows named-pipe peer.
#![cfg(windows)]
use crate::peer_support as support;
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
        "warning",
        "warning-en",
        "warning-zh",
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
                diagnostics: if case.starts_with("warning") {
                    ConfigurationDiagnostics {
                        items: vec![ConfigurationDiagnostic {
                            field: "keys.help".into(),
                            reason: "invalid-keybinding".into(),
                            disposition: ConfigurationDiagnosticDisposition::Disabled,
                        }],
                        omitted_count: 2,
                    }
                } else {
                    Default::default()
                },
                keys_profile: "[keys]\nprefix='ctrl+a'\n".into(),
                theme_name: orchbee_workbench_protocol::ConfigurationThemeName::TerminalNative,
                notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
                agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
                spaces_sidebar_profile: orchbee_local_runtime::SpaceSidebarSettings::default()
                    .profile(),
                window_title: if case == "en" {
                    "{tab}\n\u{202e}"
                } else {
                    "{hostname}: {workspace}"
                }
                .into(),
                restart_required: if case == "restart" {
                    vec![ConfigurationRestart::DefaultShell]
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
                    session_id: request.session_id,
                    outcome,
                }),
            )
            .unwrap();
            assert_closed(&mut stream);
        });
        let text = matches!(case, "en" | "zh" | "warning-en" | "warning-zh");
        let output = fixture.run_with_env(
            &[
                "server",
                if reading { "config" } else { "reload-config" },
                "--format",
                if text { "text" } else { "json" },
            ],
            None,
            None,
            &[(
                "LC_ALL",
                std::ffi::OsStr::new(if matches!(case, "zh" | "warning-zh") {
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
            assert!(text.contains(if case == "warning-zh" {
                "配置已发布但含警告"
            } else if case == "warning-en" {
                "Configuration published with warnings"
            } else if case == "zh" {
                "配置已发布"
            } else {
                "Configuration published"
            }));
            assert!(text.contains("mode=auto"));
            if matches!(case, "zh" | "warning-zh") {
                assert!(text.contains("（平台默认）"));
                assert!(text.contains("窗口标题模板：\"{hostname}: {workspace}\""));
            } else {
                assert!(text.contains(if case == "en" {
                    r#"Window title template: "{tab}\n\u{202e}""#
                } else {
                    r#"Window title template: "{hostname}: {workspace}""#
                }));
                if case == "en" {
                    assert!(text.contains(r"C:\\tools\\\u{202e}shell.exe"));
                }
                assert!(!text.contains('\u{202e}'));
            }
            if case.starts_with("warning") {
                assert!(text.contains("visible=1 omitted=2"));
                assert!(text.contains("field=\"keys.help\" reason=invalid-keybinding"));
            }
        } else {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.server.configuration.json"),
            );
            assert_eq!(document["version"], 1);
            if matches!(
                case,
                "startup" | "current" | "reload" | "restart" | "warning"
            ) {
                assert_eq!(
                    document["result"]["reply"]["outcome"]["snapshot"]["defaultShell"],
                    ""
                );
                assert_eq!(
                    document["result"]["reply"]["outcome"]["snapshot"]["shellMode"],
                    "auto"
                );
            }
            if case == "warning" {
                assert_eq!(
                    document["result"]["reply"]["outcome"]["snapshot"]["diagnostics"]["items"][0]["field"],
                    "keys.help"
                );
                assert_eq!(
                    document["result"]["reply"]["outcome"]["snapshot"]["diagnostics"]["omittedCount"],
                    2
                );
            }
            if matches!(case, "lost" | "wrong-action") {
                assert!(document["result"]["reply"].is_null());
            }
        }
    }
}

#[test]
fn reload_does_not_activate_a_session_and_rejects_unrelated_flags_before_admission() {
    let fixture = Fixture::new();
    let _rendezvous = fixture.rendezvous();
    let output = fixture.run(&["server", "reload-config", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for arguments in [
        vec!["server", "reload-config", "--workspace", "x"],
        vec!["server", "config", "extra"],
    ] {
        let output = fixture.run(&arguments);
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn public_configuration_requires_complete_current_snapshots() {
    let schema: Value = serde_json::from_str(include_str!(
        "../src/cli/schemas/orchbee.server.configuration.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let document = serde_json::json!({
        "schema":"orchbee.server.configuration","version":1,"source":{"kind":"local-runtime"},
        "result":{"action":"current","interrupted":false,"cleanupFailed":false,
            "reply":{"protocolVersion":1,"requestId":"00000000000000000000000000000001","sessionId":"00000000000000000000000000000002",
                "outcome":{"kind":"current","snapshot":{"revision":"0","keysProfile":"[keys]","manifestCheck":true,"restartRequired":[],
                    "defaultShell":"pwsh.exe","shellMode":"non_login","windowTitle":"{tab}","notificationProfile":"[ui.toast]",
                    "agentSidebarProfile":"{}","spacesSidebarProfile":"{}","themeName":"one-light",
                    "diagnostics":{"items":[{"field":"keys.help","reason":"invalid-keybinding","disposition":"disabled"}],"omittedCount":0}}}}}
    });
    assert!(validator.is_valid(&document));
    for field in [
        "defaultShell",
        "shellMode",
        "windowTitle",
        "notificationProfile",
        "agentSidebarProfile",
        "spacesSidebarProfile",
        "themeName",
        "diagnostics",
    ] {
        let mut missing = document.clone();
        missing["result"]["reply"]["outcome"]["snapshot"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(!validator.is_valid(&missing), "required {field}");
    }
    let mut invalid = document.clone();
    invalid["result"]["reply"]["outcome"]["snapshot"]["themeName"] = "custom".into();
    assert!(!validator.is_valid(&invalid));
    let mut invalid = document.clone();
    invalid["result"]["reply"]["outcome"]["snapshot"]["shellMode"] = "unknown".into();
    assert!(!validator.is_valid(&invalid));
    let mut invalid = document;
    let items = invalid["result"]["reply"]["outcome"]["snapshot"]["diagnostics"]["items"]
        .as_array_mut()
        .unwrap();
    items.push(items[0].clone());
    assert!(!validator.is_valid(&invalid));
}
