use orchbee_workbench_protocol::*;
use serde_json::json;
fn request() -> serde_json::Value {
    json!({"kind":"workbench.workspace-metadata","protocolVersion":1,"requestId":"00000000000000000000000000000001","sessionId":"00000000000000000000000000000002",
        "action":{"kind":"report","workspaceId":"00000000000000000000000000000003","source":"hook:codex","tokens":{"summary":"中文"},"sequence":"18446744073709551615","ttlMilliseconds":86400000}})
}
#[test]
fn canonical_reports_roundtrip_and_reject_invalid_or_duplicate_tokens() {
    let value = request();
    let parsed = parse_workbench_request(&serde_json::to_vec(&value).unwrap()).unwrap();
    let mut frame = vec![];
    write_workbench_request(&mut frame, &parsed).unwrap();
    assert_eq!(&frame[6..8], &31_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut frame.as_slice()).unwrap(),
        parsed
    );
    for (pointer, value) in [
        ("/action/source", json!(" source ")),
        ("/action/tokens/summary", json!("a\nb")),
        ("/action/sequence", json!("18446744073709551616")),
        ("/action/ttlMilliseconds", json!(0)),
        ("/action/ttlMilliseconds", json!(86400001)),
        ("/action/tokens", json!({})),
        ("/action/tokens", json!({"bad.key":"x"})),
    ] {
        let mut bad = request();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            parse_workbench_request(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let duplicate = serde_json::to_string(&request()).unwrap().replace(
        "\"summary\":\"中文\"",
        "\"summary\":null,\"summary\":\"中文\"",
    );
    assert!(parse_workbench_request(duplicate.as_bytes()).is_err());
    assert_eq!(
        MetadataTokenValue::normalize(" \n review\nready \t ")
            .unwrap()
            .as_str(),
        "reviewready"
    );
    assert_eq!(
        MetadataTokenValue::normalize(&"字".repeat(90))
            .unwrap()
            .as_str()
            .chars()
            .count(),
        80
    );
    assert!(MetadataTokenValue::normalize(" \n \t").is_none());
}

#[test]
fn metadata_shared_corpus_and_carrier_bounds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-workspace-metadata-conformance-v1.json"
    ))
    .unwrap();
    for (group, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let mut frame = vec![];
            if case["direction"] == "request" {
                let parsed = parse_workbench_request(&bytes);
                assert_eq!(parsed.is_ok(), expected, "{}", case["name"]);
                if let Ok(request) = parsed {
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(&frame[6..8], &31_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                    assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                }
            } else {
                let parsed = parse_workbench_response(&bytes);
                assert_eq!(parsed.is_ok(), expected, "{}", case["name"]);
                if let Ok(response) = parsed {
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(&frame[6..8], &32_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                    assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                }
            }
        }
    }
    for (kind, maximum) in [
        (31_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (32, MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut cursor = std::io::Cursor::new(header);
        let result = if kind == 31 {
            read_workbench_request(&mut cursor).map(|_| ())
        } else {
            read_workbench_response(&mut cursor).map(|_| ())
        };
        assert!(matches!(result, Err(WorkbenchWireError::Contract(_))));
        assert_eq!(cursor.position(), 12);
    }
}

#[test]
fn metadata_result_correlation_preserves_order_and_all_request_identity() {
    let request = WorkspaceMetadataRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        action: WorkspaceMetadataAction::Read {
            workspace_ids: (3..11)
                .map(|n| WorkbenchWorkspaceId::parse(&format!("{n:032x}")).unwrap())
                .collect(),
        },
    };
    let WorkspaceMetadataAction::Read { workspace_ids } = &request.action else {
        unreachable!()
    };
    let tokens = MetadataTokens::new(
        (0..32)
            .map(|n| {
                (
                    MetadataTokenKey::parse(&format!("k{n}")).unwrap(),
                    MetadataTokenValue::parse(&"😀".repeat(80)).unwrap(),
                )
            })
            .collect(),
    )
    .unwrap();
    let mut result = WorkspaceMetadataResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome: WorkspaceMetadataOutcome::Read {
            workbench_revision: WorkbenchRevision::new(0),
            metadata_revision: WorkbenchRevision::new(u64::MAX),
            workspaces: workspace_ids
                .iter()
                .map(|id| WorkspaceMetadataRow {
                    workspace_id: id.clone(),
                    tokens: tokens.clone(),
                })
                .collect(),
        },
    };
    assert!(result.answers(&request));
    let response = WorkbenchResponse::WorkspaceMetadata(result.clone());
    let bytes = serialize_workbench_response(&response).unwrap();
    assert!(bytes.len() > MAXIMUM_ENCODED_INTENT_BYTES);
    assert_eq!(parse_workbench_response(&bytes).unwrap(), response);
    let WorkspaceMetadataOutcome::Read { workspaces, .. } = &mut result.outcome else {
        unreachable!()
    };
    workspaces.swap(0, 1);
    assert!(!result.answers(&request));
    result.outcome = WorkspaceMetadataOutcome::Failed {
        reason: WorkspaceMetadataFailure::NotFound,
    };
    assert!(result.answers(&request));
    result.session_id = SessionId::parse(&format!("{:032x}", 90)).unwrap();
    assert!(!result.answers(&request));
    result.session_id = request.session_id.clone();
    result.request_id = WorkbenchRequestId::parse(&format!("{:032x}", 91)).unwrap();
    assert!(!result.answers(&request));
}
