//! Shared closed-shape decisions and the one-attempt Settings carrier boundary.
use orchbee_workbench_protocol::*;

#[test]
fn settings_matches_shared_corpus_and_roundtrips_the_distinct_carrier() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-settings-conformance-v1.json"
    ))
    .unwrap();
    for (group, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            if case["direction"] == "request" {
                let request = parse_workbench_request(&bytes);
                assert_eq!(request.is_ok(), accepted, "{}", case["name"]);
                if let Ok(request) = request {
                    let mut frame = Vec::new();
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(u16::from_be_bytes([frame[6], frame[7]]), 59);
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                }
            } else {
                let response = parse_workbench_response(&bytes);
                assert_eq!(response.is_ok(), accepted, "{}", case["name"]);
                if let Ok(response) = response {
                    let mut frame = Vec::new();
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(u16::from_be_bytes([frame[6], frame[7]]), 60);
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                }
            }
        }
    }
}

#[test]
fn settings_requires_exact_correlation_and_matching_operation() {
    let request = NativeSettingsRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("00000000000000000000000000000001").unwrap(),
        site_id: SiteId::parse("00000000000000000000000000000002").unwrap(),
        operation: NativeSettingsOperation::Open {},
    };
    let result = NativeSettingsResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: NativeSettingsOutcome::Refused {
            reason: NativeSettingsFailure::Busy,
        },
    };
    assert!(result.answers(&request));
    let mut other = request.clone();
    other.site_id = SiteId::parse("00000000000000000000000000000003").unwrap();
    assert!(!result.answers(&other));
    other = request.clone();
    other.request_id = WorkbenchRequestId::parse("00000000000000000000000000000004").unwrap();
    assert!(!result.answers(&other));
}

#[test]
fn settings_save_receipts_fence_revision_and_the_requested_named_theme() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-settings-conformance-v1.json"
    ))
    .unwrap();
    let document = &corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["document"]["outcome"]["kind"] == "saved-applied")
        .unwrap()["document"];
    let WorkbenchResponse::Settings(mut result) =
        parse_workbench_response(&serde_json::to_vec(document).unwrap()).unwrap()
    else {
        panic!("settings result");
    };
    let NativeSettingsOutcome::SavedApplied { snapshot } = &mut result.outcome else {
        panic!("saved applied");
    };
    snapshot.revision = WorkbenchRevision::new(8);
    snapshot.theme_name = ConfigurationThemeName::OneLight;
    let request = NativeSettingsRequest {
        protocol_version: ProtocolVersion,
        request_id: result.request_id.clone(),
        site_id: result.site_id.clone(),
        operation: NativeSettingsOperation::Save {
            expected_revision: WorkbenchRevision::new(7),
            source: NativeSettingsSource::parse("missing").unwrap(),
            changes: NativeSettingsChanges {
                theme_name: Some(ConfigurationThemeName::OneLight),
                background_reminders: None,
                sound_enabled: None,
            },
        },
    };
    assert!(result.answers(&request));
    let NativeSettingsOutcome::SavedApplied { snapshot } = &mut result.outcome else {
        panic!("saved applied");
    };
    snapshot.revision = WorkbenchRevision::new(7);
    assert!(
        !result.answers(&request),
        "a stale snapshot cannot prove application"
    );
    let NativeSettingsOutcome::SavedApplied { snapshot } = &mut result.outcome else {
        panic!("saved applied");
    };
    snapshot.revision = WorkbenchRevision::new(8);
    snapshot.theme_name = ConfigurationThemeName::OneDark;
    assert!(
        !result.answers(&request),
        "a different named theme does not answer this save"
    );

    let NativeSettingsOutcome::SavedApplied { mut snapshot } = result.outcome else {
        panic!("saved applied");
    };
    snapshot.revision = WorkbenchRevision::new(7);
    snapshot.theme_name = ConfigurationThemeName::OneLight;
    result.outcome = NativeSettingsOutcome::NoChange {
        snapshot: snapshot.clone(),
    };
    assert!(result.answers(&request));
    snapshot.revision = WorkbenchRevision::new(8);
    result.outcome = NativeSettingsOutcome::NoChange {
        snapshot: snapshot.clone(),
    };
    assert!(
        !result.answers(&request),
        "no change must retain the captured revision"
    );
    snapshot.revision = WorkbenchRevision::new(7);
    snapshot.theme_name = ConfigurationThemeName::OneDark;
    result.outcome = NativeSettingsOutcome::NoChange {
        snapshot: snapshot.clone(),
    };
    assert!(
        !result.answers(&request),
        "no change must agree with the requested named theme"
    );
    result.outcome = NativeSettingsOutcome::SavedNotApplied {
        snapshot: snapshot.clone(),
    };
    assert!(
        result.answers(&request),
        "a retained active theme may differ from the disk save"
    );
    snapshot.revision = WorkbenchRevision::new(8);
    result.outcome = NativeSettingsOutcome::SavedNotApplied {
        snapshot: snapshot.clone(),
    };
    assert!(
        !result.answers(&request),
        "not applied must retain the captured revision"
    );

    let mut exhausted = request.clone();
    let NativeSettingsOperation::Save {
        expected_revision, ..
    } = &mut exhausted.operation
    else {
        panic!("save request");
    };
    *expected_revision = WorkbenchRevision::new(u64::MAX);
    snapshot.revision = WorkbenchRevision::new(0);
    snapshot.theme_name = ConfigurationThemeName::OneLight;
    result.outcome = NativeSettingsOutcome::SavedApplied { snapshot };
    assert!(
        !result.answers(&exhausted),
        "revision exhaustion cannot wrap to a known apply"
    );
    let mut open = request;
    open.operation = NativeSettingsOperation::Open {};
    assert!(!result.answers(&open), "save results cannot answer an open");
}

#[test]
fn settings_checks_request_size_and_direction_before_body_admission() {
    let request = NativeSettingsRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("00000000000000000000000000000001").unwrap(),
        site_id: SiteId::parse("00000000000000000000000000000002").unwrap(),
        operation: NativeSettingsOperation::Open {},
    };
    let mut encoded =
        serialize_workbench_request(&WorkbenchRequest::Settings(request.clone())).unwrap();
    encoded.resize(MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES, b' ');
    assert!(parse_workbench_request(&encoded).is_ok());
    encoded.push(b' ');
    assert!(parse_workbench_request(&encoded).is_err());
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &WorkbenchRequest::Settings(request)).unwrap();
    frame[8..12]
        .copy_from_slice(&((MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES + 1) as u32).to_be_bytes());
    assert!(matches!(
        read_workbench_request(&mut &frame[..12]),
        Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge { actual, maximum }))
            if actual == MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES + 1 && maximum == MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES
    ));
    frame[6..8].copy_from_slice(&60u16.to_be_bytes());
    assert!(matches!(
        read_workbench_request(&mut frame.as_slice()),
        Err(WorkbenchWireError::InvalidHeader)
    ));
}
