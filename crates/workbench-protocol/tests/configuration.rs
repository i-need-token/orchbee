use orchbee_workbench_protocol::*;
fn request() -> ConfigurationRequest {
    ConfigurationRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
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
        site_id: request.site_id.clone(),
        outcome: ConfigurationOutcome::Reloaded {
            snapshot: ConfigurationSnapshot {
                revision: WorkbenchRevision::new(1),
                keys_profile: format!("[keys]\n#{}", "x".repeat(20_000)),
                default_shell: String::new(),
                shell_mode: orchbee_workbench_protocol::ConfigurationShellMode::Auto,
                manifest_check: true,
                notification_profile: "[ui.toast]\ndelivery='off'\n".into(),
                agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
                spaces_sidebar_profile: r#"{"rows":[],"row_gap":0}"#.into(),
                window_title: "{hostname}: {workspace}".into(),
                restart_required: vec![],
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
        WorkbenchResponse::Configuration(result)
    );
}
#[test]
fn configuration_rejects_unknown_fields_oversize_and_invalid_failure_shapes() {
    // Keep malformed Unicode outside the corpus container: serde_json rejects it before DTO admission.
    let invalid = br#"{"kind":"workbench.configuration.result","protocolVersion":1,"requestId":"00000000000000000000000000000001","siteId":"00000000000000000000000000000002","outcome":{"kind":"current","snapshot":{"revision":"0","keysProfile":"[keys]","manifestCheck":true,"restartRequired":[],"defaultShell":"\ud800","shellMode":"auto"}}}"#;
    assert!(parse_workbench_response(invalid).is_err());
    let request = request();
    let mut value = serde_json::to_value(&request).unwrap();
    value["unexpected"] = true.into();
    assert!(serde_json::from_value::<ConfigurationRequest>(value).is_err());
    let mut result = ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id,
        site_id: request.site_id,
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
            keys_profile: "x".repeat(65537),
            notification_profile: "[ui.toast]".into(),
            agent_sidebar_profile: r#"{"rows":null,"rows_by_agent":{},"row_gap":0}"#.into(),
            spaces_sidebar_profile: r#"{"rows":[],"row_gap":0}"#.into(),
            default_shell: String::new(),
            shell_mode: orchbee_workbench_protocol::ConfigurationShellMode::Auto,
            manifest_check: false,
            window_title: "{hostname}: {workspace}".into(),
            restart_required: vec![],
        },
    };
    assert!(serialize_workbench_response(&WorkbenchResponse::Configuration(result)).is_err());
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
        site_id: request.site_id,
        outcome: ConfigurationOutcome::Current {
            snapshot: ConfigurationSnapshot {
                revision: WorkbenchRevision::new(1),
                keys_profile: "x".repeat(20000),
                notification_profile: "[ui.toast]".into(),
                agent_sidebar_profile: agent,
                spaces_sidebar_profile: space,
                window_title: String::new(),
                manifest_check: true,
                default_shell: String::new(),
                shell_mode: ConfigurationShellMode::Auto,
                restart_required: vec![],
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
