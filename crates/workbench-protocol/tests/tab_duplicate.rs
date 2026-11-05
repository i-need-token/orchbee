use orchbee_workbench_protocol::{
    MAXIMUM_ENCODED_INTENT_BYTES, MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES, TabDuplicateAction,
    WorkbenchRequest, WorkbenchResponse, WorkbenchWireError, parse_workbench_request,
    parse_workbench_response, read_workbench_request, read_workbench_response,
    serialize_workbench_request, serialize_workbench_response, write_workbench_request,
    write_workbench_response,
};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-tab-duplicate-conformance-v1.json"
    ))
    .unwrap()
}

fn case(name: &str) -> Value {
    let corpus = corpus();
    corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["document"]
        .clone()
}

#[test]
fn shared_tab_duplicate_corpus_has_matching_typed_json_and_frame_results() {
    let corpus = corpus();
    for (group, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let encoded = serde_json::to_vec(&case["document"]).unwrap();
            let mut frame = vec![];
            if case["direction"] == "request" {
                let result = parse_workbench_request(&encoded);
                assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
                if let Ok(request) = result {
                    assert_eq!(
                        parse_workbench_request(&serialize_workbench_request(&request).unwrap())
                            .unwrap(),
                        request
                    );
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x45");
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                    assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                }
            } else {
                let result = parse_workbench_response(&encoded);
                assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
                if let Ok(response) = result {
                    assert_eq!(
                        parse_workbench_response(&serialize_workbench_response(&response).unwrap())
                            .unwrap(),
                        response
                    );
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x46");
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                    assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                }
            }
        }
    }
    for case in corpus["validJson"].as_array().unwrap() {
        let encoded = case["json"].as_str().unwrap().as_bytes();
        let canonical = if case["direction"] == "request" {
            serde_json::to_value(parse_workbench_request(encoded).unwrap()).unwrap()
        } else {
            serde_json::to_value(parse_workbench_response(encoded).unwrap()).unwrap()
        };
        assert_eq!(canonical, case["canonical"], "{}", case["name"]);
    }
}

#[test]
fn tab_duplicate_frame_limits_apply_before_reading_or_allocating_the_body() {
    for (kind, maximum) in [
        (69_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (70, MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut reader = std::io::Cursor::new(header);
        if kind == 69 {
            assert!(matches!(
                read_workbench_request(&mut reader),
                Err(WorkbenchWireError::Contract(_))
            ));
        } else {
            assert!(matches!(
                read_workbench_response(&mut reader),
                Err(WorkbenchWireError::Contract(_))
            ));
        }
        assert_eq!(reader.position(), 12);
    }
}

#[test]
fn maximum_operation_fits_both_frames() {
    let request = serde_json::to_vec(&case("maximum-tab-ids")).unwrap();
    assert!(request.len() < MAXIMUM_ENCODED_INTENT_BYTES);
    parse_workbench_request(&request).unwrap();
    let mut document = case("duplicated");
    let members = (0..256)
        .map(|index| {
            serde_json::json!({
                "tabId": format!("{:032x}", 1000 + index),
                "copy": {
                    "tabId": format!("{:032x}", 2000 + index),
                    "revision": "18446744073709551615",
                    "terminal": { "kind": "started", "geometry": {
                        "columns": 4096, "rows": 64, "cellPixelWidth": 15, "cellPixelHeight": 1023
                    }}
                }
            })
        })
        .collect::<Vec<_>>();
    document["outcome"]["members"] = Value::Array(members);
    let encoded = serde_json::to_vec(&document).unwrap();
    assert!(encoded.len() < MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES);
    let parsed = parse_workbench_response(&encoded).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), document);
}

/// A Start receipt names the requested context and members, and a Read never answers for another
/// operation's Start.
#[test]
fn start_admits_only_receipts_for_its_frozen_scope() {
    let WorkbenchRequest::TabDuplicate(request) =
        parse_workbench_request(&serde_json::to_vec(&case("start")).unwrap()).unwrap()
    else {
        unreachable!()
    };
    let outcome = |name: &str| {
        let WorkbenchResponse::TabDuplicate(result) =
            parse_workbench_response(&serde_json::to_vec(&case(name)).unwrap()).unwrap()
        else {
            unreachable!()
        };
        result.outcome
    };
    let pending = outcome("pending-uncopied-members");
    assert!(request.action.admits(&pending));
    assert!(
        request
            .action
            .admits(&outcome("rejected-revision-mismatch"))
    );
    assert!(!request.action.admits(&outcome("not-found")));
    let TabDuplicateAction::Start {
        operation_id,
        terminal_context_id,
        tab_ids,
        expected_revision,
        geometry,
    } = request.action
    else {
        unreachable!()
    };
    let fewer = TabDuplicateAction::Start {
        operation_id: operation_id.clone(),
        terminal_context_id,
        tab_ids: tab_ids[..1].to_vec(),
        expected_revision,
        geometry,
    };
    assert!(!fewer.admits(&pending));
    let read = TabDuplicateAction::Read { operation_id };
    assert!(read.admits(&pending));
    assert!(read.admits(&outcome("not-found")));
    assert!(!read.admits(&outcome("conflict")));
    assert!(!read.admits(&outcome("rejected-revision-mismatch")));
}
