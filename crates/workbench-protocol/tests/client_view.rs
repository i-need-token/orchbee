use orchbee_workbench_protocol::{
    MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES, MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES,
    MAXIMUM_ENCODED_INTENT_BYTES, WorkbenchWireError, parse_workbench_request,
    parse_workbench_response, read_workbench_request, read_workbench_response,
    serialize_workbench_request, serialize_workbench_response, write_workbench_request,
    write_workbench_response,
};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap()
}

#[test]
fn shared_client_view_corpus_has_matching_typed_json_and_frame_results() {
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
                    let kind = u16::try_from(case["frameKind"].as_u64().unwrap_or(25)).unwrap();
                    assert_eq!(&frame[6..8], &kind.to_be_bytes());
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
                    let kind = u16::try_from(case["frameKind"].as_u64().unwrap_or(26)).unwrap();
                    assert_eq!(&frame[6..8], &kind.to_be_bytes());
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
fn client_view_frame_limits_apply_before_reading_or_allocating_the_body() {
    for (kind, maximum) in [
        (25_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (26, MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES),
        (29, MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut reader = std::io::Cursor::new(header);
        if kind == 25 || kind == 29 {
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
fn attention_maximum_batch_fits_and_overflow_is_rejected() {
    let corpus = corpus();
    let mut doc = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "client-agent-attention-read")
        .unwrap()["document"]
        .clone();
    let ids: Vec<_> = (0..100)
        .map(|index| format!("{index:03}{}", "a".repeat(125)))
        .collect();
    doc["outcome"]["operation"]["command"]["intent"]["sessionIds"] = serde_json::json!(ids);
    doc["outcome"]["operation"]["status"]["attention"]["revision"] =
        serde_json::json!(9007199254740991_u64);
    let entries: Vec<_> = ids.iter().map(|id| serde_json::json!({"sessionId":id,"state":{"generation":9007199254740991_u64,"sequence":9007199254740991_u64,"stateChangeRevision":9007199254740991_u64,"seen":false}})).collect();
    doc["outcome"]["operation"]["status"]["attention"]["entries"] = serde_json::json!(entries);
    let encoded = serde_json::to_vec(&doc).unwrap();
    let response = parse_workbench_response(&encoded).unwrap();
    let mut frame = vec![];
    write_workbench_response(&mut frame, &response).unwrap();
    assert!(frame.len() <= MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES + 12);
    let extra = doc["outcome"]["operation"]["status"]["attention"]["entries"][0].clone();
    doc["outcome"]["operation"]["status"]["attention"]["entries"]
        .as_array_mut()
        .unwrap()
        .push(extra);
    assert!(parse_workbench_response(&serde_json::to_vec(&doc).unwrap()).is_err());
}

#[test]
fn navigation_maximum_batch_fits_and_overflow_is_rejected() {
    let corpus = corpus();
    let mut doc = corpus["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "client-navigation-read")
        .unwrap()["document"]
        .clone();
    let contexts: Vec<_> = (1..=128).map(|n| format!("{n:032x}")).collect();
    let op = &mut doc["outcome"]["operation"];
    op["command"]["intent"]["terminalContextIds"] = serde_json::json!(contexts);
    op["status"]["view"]["selection"]["terminalContextId"] = serde_json::json!(contexts[0]);
    op["status"]["view"]["selection"]["tabId"] = serde_json::json!(format!("{:032x}", 1001));
    op["status"]["navigation"]["contexts"] = serde_json::json!(contexts.iter().enumerate().map(|(i,id)| serde_json::json!({"terminalContextId":id,"activeTabId":format!("{:032x}",1001+i)})).collect::<Vec<_>>());
    let encoded = serde_json::to_vec(&doc).unwrap();
    let response = parse_workbench_response(&encoded).unwrap();
    let mut frame = vec![];
    write_workbench_response(&mut frame, &response).unwrap();
    assert!(frame.len() <= MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES + 12);
    let contexts = doc["outcome"]["operation"]["status"]["navigation"]["contexts"]
        .as_array_mut()
        .unwrap();
    contexts
        .push(serde_json::json!({"terminalContextId":format!("{:032x}",129),"activeTabId":null}));
    assert!(parse_workbench_response(&serde_json::to_vec(&doc).unwrap()).is_err());
}
