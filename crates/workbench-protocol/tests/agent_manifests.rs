use orchbee_workbench_protocol::*;
use serde_json::Value;
#[test]
fn agent_manifests_shared_corpus_and_carrier_are_closed() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-agent-manifests-conformance-v1.json"
    ))
    .unwrap();
    for accepted in [true, false] {
        for case in corpus[if accepted { "valid" } else { "invalid" }]
            .as_array()
            .unwrap()
        {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            if case["direction"] == "request" {
                let parsed = parse_workbench_request(&bytes);
                assert_eq!(parsed.is_ok(), accepted, "{}", case["name"]);
                if let Ok(request) = parsed {
                    let mut frame = Vec::new();
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(&frame[6..8], &37u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                    assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                }
            } else {
                let parsed = parse_workbench_response(&bytes);
                assert_eq!(parsed.is_ok(), accepted, "{}", case["name"]);
                if let Ok(response) = parsed {
                    let mut frame = Vec::new();
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(&frame[6..8], &38u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                    assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                }
            }
        }
    }
}
