use orchbee_workbench_protocol::*;
fn request() -> ConfigurationRequest {
    ConfigurationRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        action: ConfigurationAction::Reload,
    }
}
#[test]
fn configuration_roundtrips_have_distinct_carrier_kinds_and_exact_correlation() {
    let request = request();
    let mut bytes = Vec::new();
    write_workbench_request(
        &mut bytes,
        &WorkbenchRequest::Configuration(request.clone()),
    )
    .unwrap();
    assert_eq!(u16::from_be_bytes([bytes[6], bytes[7]]), 39);
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        WorkbenchRequest::Configuration(request.clone())
    );
    let result = ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome: ConfigurationOutcome::Reloaded {
            snapshot: ConfigurationSnapshot {
                revision: WorkbenchRevision::new(1),
                diagnostics: ConfigurationDiagnostics::default(),
                keys_profile: format!("[keys]\n#{}", "x".repeat(20_000)),
                manifest_check: true,
                focus_new_panel: true,
                worktrees_directory: Some("C:/trees".into()),
                new_cwd: "C:/项目".into(),
                notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
                agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
                spaces_sidebar_profile: r#"{"rows":[],"row_gap":0}"#.into(),
                window_title: "{hostname}: {workspace}".into(),
            },
            changed: true,
        },
    };
    assert!(result.answers(&request));
    let mut wrong = request.clone();
    wrong.action = ConfigurationAction::Current;
    assert!(!result.answers(&wrong));
    bytes.clear();
    write_workbench_response(
        &mut bytes,
        &WorkbenchResponse::Configuration(result.clone()),
    )
    .unwrap();
    assert_eq!(u16::from_be_bytes([bytes[6], bytes[7]]), 40);
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        WorkbenchResponse::Configuration(result.clone())
    );
    for path in [
        String::new(),
        "C:/\u{7}trees".into(),
        format!("C:/{}", "x".repeat(4094)),
        "trees".into(),
        r"\\?\C:".into(),
    ] {
        for new_cwd in [false, true] {
            let mut invalid = result.clone();
            if let ConfigurationOutcome::Reloaded { snapshot, .. } = &mut invalid.outcome {
                if new_cwd {
                    snapshot.new_cwd = path.clone();
                } else {
                    snapshot.worktrees_directory = Some(path.clone());
                }
            }
            assert!(invalid.validate().is_err(), "{path:?}");
        }
    }
    for new_cwd in ["follow", "home", "current", "workspace", "/srv/project"] {
        let mut valid = result.clone();
        if let ConfigurationOutcome::Reloaded { snapshot, .. } = &mut valid.outcome {
            snapshot.new_cwd = new_cwd.into();
        }
        assert!(valid.validate().is_ok(), "{new_cwd}");
    }
}
#[test]
fn configuration_rejects_unknown_fields_oversize_and_invalid_failure_shapes() {
    // Keep malformed Unicode outside the corpus container: serde_json rejects it before DTO admission.
    let invalid = br#"{"kind":"workbench.configuration.result","protocolVersion":1,"requestId":"00000000000000000000000000000001","sessionId":"00000000000000000000000000000002","outcome":{"kind":"current","snapshot":{"revision":"0","keysProfile":"[keys]","manifestCheck":true,"windowTitle":"\ud800"}}}"#;
    assert!(parse_workbench_response(invalid).is_err());
    let request = request();
    let mut value = serde_json::to_value(&request).unwrap();
    value["unexpected"] = true.into();
    assert!(serde_json::from_value::<ConfigurationRequest>(value).is_err());
    let mut result = ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        session_id: request.session_id,
        outcome: ConfigurationOutcome::Failed {
            reason: ConfigurationFailure::Busy,
            field: Some("file".into()),
            detail: Some("io-error".into()),
        },
    };
    assert!(result.validate().is_err());
    result.outcome = ConfigurationOutcome::Current {
        snapshot: ConfigurationSnapshot {
            revision: WorkbenchRevision::new(1),
            diagnostics: ConfigurationDiagnostics::default(),
            keys_profile: "x".repeat(65537),
            notification_profile: "[ui.toast]".into(),
            agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
            spaces_sidebar_profile: r#"{"rows":[],"row_gap":0}"#.into(),
            manifest_check: false,
            focus_new_panel: true,
            worktrees_directory: None,
            new_cwd: "follow".into(),
            window_title: "{hostname}: {workspace}".into(),
        },
    };
    assert!(serialize_workbench_response(&WorkbenchResponse::Configuration(result)).is_err());
}

fn diagnostic(field: impl Into<String>, reason: impl Into<String>) -> ConfigurationDiagnostic {
    ConfigurationDiagnostic {
        field: field.into(),
        reason: reason.into(),
        disposition: ConfigurationDiagnosticDisposition::Defaulted,
    }
}

#[test]
fn configuration_diagnostics_are_bounded_closed_and_canonical() {
    let mut diagnostics = ConfigurationDiagnostics {
        items: (0..MAXIMUM_CONFIGURATION_DIAGNOSTICS)
            .map(|index| diagnostic(format!("keys.action.{index:03}"), "invalid-binding"))
            .collect(),
        omitted_count: MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED,
    };
    assert!(diagnostics.validate().is_ok());

    diagnostics
        .items
        .push(diagnostic("keys.overflow", "wrong-type"));
    assert!(diagnostics.validate().is_err());
    diagnostics.items.pop();

    diagnostics.items = vec![diagnostic(format!("{}a", "界".repeat(85)), "unknown-key")];
    assert_eq!(diagnostics.items[0].field.len(), 256);
    assert!(diagnostics.validate().is_ok());
    diagnostics.items[0].field.push('界');
    assert!(diagnostics.validate().is_err());

    diagnostics.items = vec![diagnostic("keys.help", "Invalid_Binding")];
    assert!(diagnostics.validate().is_err());
    diagnostics.items[0].reason = "a".repeat(MAXIMUM_CONFIGURATION_DIAGNOSTIC_REASON_BYTES);
    assert!(diagnostics.validate().is_ok());
    diagnostics.items[0].reason.push('a');
    assert!(diagnostics.validate().is_err());

    diagnostics.items = vec![diagnostic("keys.\u{85}help", "wrong-type")];
    assert!(diagnostics.validate().is_err());
    diagnostics.items = vec![
        diagnostic("keys.help", "invalid-binding"),
        diagnostic("keys.help", "wrong-type"),
    ];
    assert!(diagnostics.validate().is_ok());
    diagnostics.items.reverse();
    assert!(diagnostics.validate().is_err());
    diagnostics.items = vec![
        diagnostic("\u{e000}", "wrong-type"),
        diagnostic("\u{10000}", "wrong-type"),
    ];
    assert!(diagnostics.validate().is_ok());
    diagnostics.items.reverse();
    assert!(diagnostics.validate().is_err());
    diagnostics.items = vec![
        diagnostic("keys.help", "wrong-type"),
        diagnostic("keys.help", "wrong-type"),
    ];
    assert!(diagnostics.validate().is_err());
    diagnostics.items = vec![
        diagnostic("keys.zoom", "wrong-type"),
        diagnostic("keys.help", "wrong-type"),
    ];
    assert!(diagnostics.validate().is_err());

    diagnostics.items.clear();
    diagnostics.omitted_count = MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED + 1;
    assert!(diagnostics.validate().is_err());
}

#[test]
fn configuration_matches_shared_corpus() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-configuration-conformance-v1.json"
    ))
    .unwrap();
    for (group, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let result = if case["direction"] == "request" {
                parse_workbench_request(&bytes).is_ok()
            } else {
                parse_workbench_response(&bytes).is_ok()
            };
            assert_eq!(result, accepted, "{}", case["name"]);
        }
    }
}

#[test]
fn configuration_notification_profile_has_a_closed_16_kib_wire_bound() {
    const CONTRACT_BYTES: usize = 16 * 1024;
    let request = request();
    let result = |notification_profile| ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome: ConfigurationOutcome::Current {
            snapshot: ConfigurationSnapshot {
                revision: WorkbenchRevision::new(1),
                diagnostics: ConfigurationDiagnostics::default(),
                keys_profile: "[keys]".into(),
                notification_profile,
                agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
                spaces_sidebar_profile: r#"{"rows":[],"row_gap":0}"#.into(),
                manifest_check: true,
                focus_new_panel: true,
                worktrees_directory: None,
                new_cwd: "follow".into(),
                window_title: String::new(),
            },
        },
    };
    assert!(result("x".repeat(CONTRACT_BYTES)).validate().is_ok());
    assert!(result("x".repeat(CONTRACT_BYTES + 1)).validate().is_err());
}

#[test]
fn configuration_frame_fits_both_bounded_escaped_profiles_and_checks_header_before_body() {
    let request = request();
    let profile = |name| {
        serde_json::json!({"rows":[[{"name":name,"style":{"fg":null,"bold":null,"dim":null},"rules":(0..4).map(|_|serde_json::json!({"condition":{"kind":"Equals","value":"\\".repeat(3600)},"ignore_case":false,"style":{"fg":null,"bold":null,"dim":null},"hide":false})).collect::<Vec<_>>()}]],"row_gap":0}).to_string()
    };
    let agent = profile("agent");
    let space = profile("workspace");
    assert!(agent.len() <= 32768 && space.len() <= 32768);
    let response = WorkbenchResponse::Configuration(ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        session_id: request.session_id,
        outcome: ConfigurationOutcome::Current {
            snapshot: ConfigurationSnapshot {
                revision: WorkbenchRevision::new(1),
                diagnostics: ConfigurationDiagnostics::default(),
                keys_profile: "x".repeat(20000),
                notification_profile: "[ui.toast]".into(),
                agent_sidebar_profile: agent,
                spaces_sidebar_profile: space,
                window_title: String::new(),
                manifest_check: true,
                focus_new_panel: true,
                worktrees_directory: None,
                new_cwd: "follow".into(),
            },
        },
    });
    let mut bytes = Vec::new();
    write_workbench_response(&mut bytes, &response).unwrap();
    assert!(bytes.len() > 128 * 1024);
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        response
    );
    bytes[8..12].copy_from_slice(&((MAXIMUM_CONFIGURATION_BYTES + 1) as u32).to_be_bytes());
    assert!(matches!(
        read_workbench_response(&mut &bytes[..12]),
        Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge { actual, maximum })) if actual == MAXIMUM_CONFIGURATION_BYTES+1 && maximum == MAXIMUM_CONFIGURATION_BYTES
    ));
}
