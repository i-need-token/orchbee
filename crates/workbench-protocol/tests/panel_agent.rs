use orchbee_workbench_protocol::*;
#[test]
fn panel_agent_shared_corpus_and_carrier_bounds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json"
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
                    assert_eq!(&frame[6..8], &35_u16.to_be_bytes());
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
                    assert_eq!(&frame[6..8], &36_u16.to_be_bytes());
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
        (35_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (36, MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut cursor = std::io::Cursor::new(header);
        let result = if kind == 35 {
            read_workbench_request(&mut cursor).map(|_| ())
        } else {
            read_workbench_response(&mut cursor).map(|_| ())
        };
        assert!(matches!(result, Err(WorkbenchWireError::Contract(_))));
        assert_eq!(cursor.position(), 12);
    }
}
