use super::*;

mod bounds;
use serde::Deserialize;
use serde_json::Value;
use std::io::Cursor;

#[derive(Deserialize)]
struct Case {
    name: String,
    direction: Direction,
    document: Value,
}
#[derive(Deserialize)]
struct RawCase {
    name: String,
    direction: Direction,
    json: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalRawCase {
    name: String,
    direction: Direction,
    json: String,
    canonical_json: String,
    canonical: Value,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Request,
    Response,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Corpus {
    valid: Vec<Case>,
    invalid: Vec<Case>,
    valid_json: Vec<CanonicalRawCase>,
    invalid_json: Vec<RawCase>,
}

#[test]
fn command_contract_matches_the_shared_corpus() {
    let corpus: Corpus = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/workbench/v1/workbench-command-conformance-v1.json"
    )))
    .unwrap();
    for case in corpus.valid {
        let encoded = serde_json::to_vec(&case.document).unwrap();
        let roundtrip = match case.direction {
            Direction::Request => {
                serialize_workbench_request(&parse_workbench_request(&encoded).unwrap()).unwrap()
            }
            Direction::Response => {
                serialize_workbench_response(&parse_workbench_response(&encoded).unwrap()).unwrap()
            }
        };
        assert_eq!(
            serde_json::from_slice::<Value>(&roundtrip).unwrap(),
            case.document,
            "{}",
            case.name
        );
    }
    for case in corpus.valid_json {
        let encoded = match case.direction {
            Direction::Request => {
                let parsed = parse_workbench_request(case.json.as_bytes())
                    .unwrap_or_else(|error| panic!("{}: {error}", case.name));
                let encoded = serialize_workbench_request(&parsed).unwrap();
                assert_eq!(parse_workbench_request(&encoded).unwrap(), parsed);
                encoded
            }
            Direction::Response => {
                let parsed = parse_workbench_response(case.json.as_bytes())
                    .unwrap_or_else(|error| panic!("{}: {error}", case.name));
                let encoded = serialize_workbench_response(&parsed).unwrap();
                assert_eq!(parse_workbench_response(&encoded).unwrap(), parsed);
                encoded
            }
        };
        assert_eq!(encoded, case.canonical_json.as_bytes(), "{}", case.name);
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap(),
            case.canonical,
            "{}",
            case.name
        );
    }
    for case in corpus.invalid {
        let encoded = serde_json::to_vec(&case.document).unwrap();
        let rejected = match case.direction {
            Direction::Request => parse_workbench_request(&encoded).is_err(),
            Direction::Response => parse_workbench_response(&encoded).is_err(),
        };
        assert!(rejected, "{}", case.name);
    }
    for case in corpus.invalid_json {
        let rejected = match case.direction {
            Direction::Request => parse_workbench_request(case.json.as_bytes()).is_err(),
            Direction::Response => parse_workbench_response(case.json.as_bytes()).is_err(),
        };
        assert!(rejected, "{}", case.name);
    }
}

#[test]
fn terminal_geometry_matches_the_obcs_numeric_boundaries_without_a_protocol_dependency() {
    for (columns, rows, cell_pixel_width, cell_pixel_height) in [
        (1, 1, 1, 1),
        (1, 1, 4096, 4096),
        (4096, 64, 15, 1023),
        (255, 255, 257, 257),
        (80, 24, 0, 0),
        (4096, 64, 0, 0),
    ] {
        let geometry =
            WorkbenchTerminalGeometry::new(columns, rows, cell_pixel_width, cell_pixel_height)
                .unwrap();
        assert_eq!(geometry.columns(), columns);
        assert_eq!(geometry.rows(), rows);
        assert_eq!(geometry.cell_pixel_width(), cell_pixel_width);
        assert_eq!(geometry.cell_pixel_height(), cell_pixel_height);
    }

    for dimensions in [
        (0, 1, 1, 1),
        (1, 0, 1, 1),
        (1, 1, 0, 1),
        (1, 1, 1, 0),
        (80, 24, 0, 18),
        (80, 24, 9, 0),
        (4097, 1, 1, 1),
        (1, 4097, 1, 1),
        (1, 1, 4097, 1),
        (1, 1, 1, 4097),
        (4096, 65, 1, 1),
        (4096, 1, 16, 1),
        (1, 4096, 1, 16),
    ] {
        assert!(
            WorkbenchTerminalGeometry::new(dimensions.0, dimensions.1, dimensions.2, dimensions.3)
                .is_err()
        );
    }
}

#[test]
fn request_bound_is_checked_before_parsing_and_semantic_equality_ignores_encoding() {
    let first = br#"{"kind":"workbench.command","protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","expectedRevision":"0","action":{"kind":"project.create","label":"A"}}"#;
    let second = br#"{ "action":{"label":"\u0041","kind":"project.create"},"expectedRevision":"0","siteId":"22222222222222222222222222222222","requestId":"11111111111111111111111111111111","protocolVersion":1,"kind":"workbench.command" }"#;
    assert_eq!(
        parse_workbench_request(first).unwrap(),
        parse_workbench_request(second).unwrap()
    );
    let mut padded = first.to_vec();
    padded.resize(MAXIMUM_ENCODED_INTENT_BYTES, b' ');
    assert!(parse_workbench_request(&padded).is_ok());
    padded.push(b' ');
    assert!(matches!(
        parse_workbench_request(&padded),
        Err(ContractError::EncodedValueTooLarge { .. })
    ));
}

#[test]
fn typed_tab_moves_preserve_exact_fields_through_json_and_obwp() {
    for (direction, wire_direction) in [
        (WorkbenchTabMoveDirection::Previous, "previous"),
        (WorkbenchTabMoveDirection::Next, "next"),
        (WorkbenchTabMoveDirection::First, "first"),
        (WorkbenchTabMoveDirection::Last, "last"),
    ] {
        let request_id = WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap();
        let context =
            WorkbenchTerminalContextId::parse("33333333333333333333333333333333").unwrap();
        let tab = WorkbenchTabId::parse("44444444444444444444444444444444").unwrap();
        let request = WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            site_id: SiteId::parse("22222222222222222222222222222222").unwrap(),
            expected_revision: WorkbenchRevision::new(7),
            action: WorkbenchAction::TabMove {
                terminal_context_id: context.clone(),
                tab_id: tab.clone(),
                direction,
            },
        });
        let encoded = serialize_workbench_request(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap(),
            serde_json::json!({
                "kind": "workbench.command", "protocolVersion": 1,
                "requestId": request_id, "siteId": "22222222222222222222222222222222",
                "expectedRevision": "7",
                "action": { "kind": "tab.move", "terminalContextId": context,
                    "tabId": tab, "direction": wire_direction }
            })
        );
        assert_eq!(parse_workbench_request(&encoded).unwrap(), request);
        let mut frame = Vec::new();
        crate::write_workbench_request(&mut frame, &request).unwrap();
        assert_eq!(&frame[6..8], &2_u16.to_be_bytes());
        assert_eq!(
            crate::read_workbench_request(&mut Cursor::new(frame)).unwrap(),
            request
        );

        let response = WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            outcome: WorkbenchCommandOutcome::TabMoved {
                revision: WorkbenchRevision::new(8),
                terminal_context_id: context.clone(),
                tab_id: tab.clone(),
            },
        });
        let encoded = serialize_workbench_response(&response).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap(),
            serde_json::json!({
                "kind": "workbench.command.result", "protocolVersion": 1,
                "requestId": request_id,
                "outcome": { "kind": "tab-moved", "revision": "8",
                    "terminalContextId": context, "tabId": tab }
            })
        );
        assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
        let mut frame = Vec::new();
        crate::write_workbench_response(&mut frame, &response).unwrap();
        assert_eq!(&frame[6..8], &4_u16.to_be_bytes());
        assert_eq!(
            crate::read_workbench_response(&mut Cursor::new(frame)).unwrap(),
            response
        );
    }
}

#[test]
fn typed_workspace_primary_directory_acceptance_preserves_exact_fields() {
    let request_id = WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap();
    let request = WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        site_id: SiteId::parse("22222222222222222222222222222222").unwrap(),
        expected_revision: WorkbenchRevision::new(7),
        action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            project_id: WorkbenchProjectId::parse("33333333333333333333333333333333").unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse("44444444444444444444444444444444").unwrap(),
            path: WorkbenchDirectoryPath::parse("/work/../work/orchbee").unwrap(),
        },
    });
    let encoded = serialize_workbench_request(&request).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encoded).unwrap(),
        serde_json::json!({
            "kind": "workbench.command",
            "protocolVersion": 1,
            "requestId": request_id,
            "siteId": "22222222222222222222222222222222",
            "expectedRevision": "7",
            "action": {
                "kind": "workspace.primary-directory.accept",
                "projectId": "33333333333333333333333333333333",
                "workspaceId": "44444444444444444444444444444444",
                "path": "/work/../work/orchbee"
            }
        })
    );
    assert_eq!(parse_workbench_request(&encoded).unwrap(), request);

    let response = WorkbenchResponse::Command(WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        outcome: WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
            revision: WorkbenchRevision::new(8),
            directory_id: WorkbenchDirectoryId::parse("55555555555555555555555555555555").unwrap(),
            worker_id: WorkerId::parse("66666666666666666666666666666666").unwrap(),
        },
    });
    let encoded = serialize_workbench_response(&response).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encoded).unwrap(),
        serde_json::json!({
            "kind": "workbench.command.result",
            "protocolVersion": 1,
            "requestId": request_id,
            "outcome": {
                "kind": "workspace-primary-directory-accepted",
                "revision": "8",
                "directoryId": "55555555555555555555555555555555",
                "workerId": "66666666666666666666666666666666"
            }
        })
    );
    assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
}

mod tab_close;
