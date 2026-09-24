use orchbee_workbench_protocol::{
    MAXIMUM_ENCODED_INTENT_BYTES, MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES,
    WorkbenchWireError, parse_workbench_request, parse_workbench_response, read_workbench_request,
    read_workbench_response, serialize_workbench_request, serialize_workbench_response,
    write_workbench_request, write_workbench_response,
};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-workspace-group-close-conformance-v1.json"
    ))
    .unwrap()
}

#[test]
fn shared_group_close_corpus_has_matching_typed_json_and_frame_results() {
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
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x17");
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
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x18");
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
fn group_close_frame_limits_apply_before_reading_or_allocating_the_body() {
    for (kind, maximum) in [
        (23_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (24, MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut reader = std::io::Cursor::new(header);
        if kind == 23 {
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
fn maximum_group_with_worst_case_escaped_labels_fits_and_extra_member_is_rejected() {
    let corpus = corpus();
    let mut document = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "pending-frozen-members")
        .unwrap()["document"]
        .clone();
    let source = document["outcome"]["workspaceId"].clone();
    let project = document["outcome"]["projectId"].clone();
    let members = (0..128).map(|index| serde_json::json!({
        "projectId":project,
        "workspaceId":if index == 127 { source.clone() } else { Value::String(format!("{:032x}", 1000 + index)) },
        "label":"\u{1}".repeat(512),
        "close": { "requestId":format!("{:032x}", 2000 + index), "observation": {
            "attemptGeneration":"18446744073709551615",
            "status":{"kind":"closed","revision":"18446744073709551615"}
        }}
    })).collect::<Vec<_>>();
    document["outcome"]["members"] = Value::Array(members);
    document["outcome"]["status"] = serde_json::json!({"kind":"closed"});
    let encoded = serde_json::to_vec(&document).unwrap();
    assert!(encoded.len() < MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES);
    let parsed = parse_workbench_response(&encoded).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), document);
    let mut extra = document["outcome"]["members"][0].clone();
    extra["workspaceId"] = Value::String(format!("{:032x}", 4000));
    extra["close"]["requestId"] = Value::String(format!("{:032x}", 4001));
    document["outcome"]["members"]
        .as_array_mut()
        .unwrap()
        .insert(0, extra);
    assert!(parse_workbench_response(&serde_json::to_vec(&document).unwrap()).is_err());
}
