use orchbee_workbench_protocol::{
    MAXIMUM_ENCODED_INTENT_BYTES, WorkbenchRequest, WorkbenchResponse, WorkbenchWireError,
    parse_workbench_request, parse_workbench_response, read_workbench_request,
    read_workbench_response, serialize_workbench_request, serialize_workbench_response,
    write_workbench_request, write_workbench_response,
};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-layout-history-conformance-v1.json"
    ))
    .unwrap()
}

#[test]
fn shared_layout_history_corpus_has_matching_typed_json_and_frame_results() {
    let corpus = corpus();
    for (group, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let encoded = serde_json::to_vec(&case["document"]).unwrap();
            let mut frame = vec![];
            if case["direction"] == "request" {
                let result = parse_workbench_request(&encoded);
                assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
                if let Ok(request) = result {
                    assert!(matches!(request, WorkbenchRequest::LayoutHistory(_)));
                    assert_eq!(
                        parse_workbench_request(&serialize_workbench_request(&request).unwrap())
                            .unwrap(),
                        request
                    );
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x47");
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                }
            } else {
                let result = parse_workbench_response(&encoded);
                assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
                if let Ok(response) = result {
                    assert!(matches!(response, WorkbenchResponse::LayoutHistory(_)));
                    assert_eq!(
                        parse_workbench_response(&serialize_workbench_response(&response).unwrap())
                            .unwrap(),
                        response
                    );
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x48");
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                }
            }
        }
    }
    for case in corpus["validJson"].as_array().unwrap() {
        let encoded = case["json"].as_str().unwrap().as_bytes();
        let canonical = serde_json::to_value(parse_workbench_request(encoded).unwrap()).unwrap();
        assert_eq!(canonical, case["canonical"], "{}", case["name"]);
    }
}

#[test]
fn layout_history_frame_limits_apply_before_reading_the_body() {
    for kind in [71_u16, 72] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(
            u32::try_from(MAXIMUM_ENCODED_INTENT_BYTES + 1)
                .unwrap()
                .to_be_bytes(),
        );
        let mut reader = std::io::Cursor::new(header);
        let refused = if kind == 71 {
            matches!(
                read_workbench_request(&mut reader),
                Err(WorkbenchWireError::Contract(_))
            )
        } else {
            matches!(
                read_workbench_response(&mut reader),
                Err(WorkbenchWireError::Contract(_))
            )
        };
        assert!(refused);
        assert_eq!(reader.position(), 12);
    }
}
