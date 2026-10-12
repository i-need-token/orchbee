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
fn settings_save_receipts_confirm_dirty_values_reset_claims_and_uncertainty_without_live_acknowledgment()
 {
    let before = UserPreferencesValues::default();
    let mut after = before.clone();
    after.application_theme = ApplicationAppearance::Dark;
    let changes = UserPreferencesPatch::between(&before, &after);
    let request = NativeSettingsRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
        operation: NativeSettingsOperation::Save {
            source: NativeSettingsSource::parse("missing").unwrap(),
            changes,
        },
    };
    let snapshot = UserPreferencesSnapshot {
        source: "a".repeat(64),
        values: after,
        claimed: [PreferenceKey::ApplicationTheme].into(),
        desktop_imported: false,
        desktop_main_imported: false,
    };
    let mut result = NativeSettingsResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: NativeSettingsOutcome::Saved {
            snapshot: snapshot.clone(),
            changed: true,
        },
    };
    assert!(result.answers(&request));
    let mut wrong = snapshot.clone();
    wrong.values.application_theme = ApplicationAppearance::Light;
    result.outcome = NativeSettingsOutcome::Saved {
        snapshot: wrong,
        changed: true,
    };
    assert!(
        !result.answers(&request),
        "another value cannot acknowledge this save"
    );
    let mut unclaimed = snapshot;
    unclaimed.claimed.clear();
    result.outcome = NativeSettingsOutcome::Saved {
        snapshot: unclaimed,
        changed: false,
    };
    assert!(
        !result.answers(&request),
        "a migration may not overwrite acknowledged edits"
    );
    result.outcome = NativeSettingsOutcome::OutcomeUnknown {};
    assert!(result.answers(&request));
    let mut read = request;
    read.operation = NativeSettingsOperation::Open {};
    assert!(
        !result.answers(&read),
        "a write receipt cannot answer a read"
    );
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
