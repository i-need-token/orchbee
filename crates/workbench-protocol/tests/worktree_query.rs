use orchbee_workbench_protocol::{
    MAXIMUM_ENCODED_INTENT_BYTES, MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES, WorkbenchWireError,
    parse_workbench_request, parse_workbench_response, read_workbench_request,
    read_workbench_response, serialize_workbench_request, serialize_workbench_response,
    write_workbench_request, write_workbench_response,
};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json"
    ))
    .unwrap()
}

#[test]
fn shared_source_corpus_has_matching_typed_json_and_frame_results() {
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
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x15");
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
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x16");
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
        let response = parse_workbench_response(case["json"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            case["canonical"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn source_query_frame_limits_apply_before_reading_or_allocating_the_body() {
    for (kind, maximum) in [
        (21_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (22, MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut reader = std::io::Cursor::new(header);
        if kind == 21 {
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
