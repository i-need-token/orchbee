use orchbee_workbench_protocol::*;
#[test]
fn semantic_events_share_closed_shapes_full_width_correlation_and_preallocation_bounds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-events-conformance-v1.json"
    ))
    .unwrap();
    for (group, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let valid = if case["direction"] == "request" {
                parse_workbench_request(&bytes)
                    .map(|request| {
                        let mut frame = Vec::new();
                        write_workbench_request(&mut frame, &request).unwrap();
                        assert_eq!(&frame[6..8], &55u16.to_be_bytes());
                        assert_eq!(
                            read_workbench_request(&mut frame.as_slice()).unwrap(),
                            request
                        );
                        frame[6..8].copy_from_slice(&53u16.to_be_bytes());
                        assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                    })
                    .is_ok()
            } else {
                parse_workbench_response(&bytes)
                    .map(|response| {
                        let mut frame = Vec::new();
                        write_workbench_response(&mut frame, &response).unwrap();
                        assert_eq!(&frame[6..8], &56u16.to_be_bytes());
                        assert_eq!(
                            read_workbench_response(&mut frame.as_slice()).unwrap(),
                            response
                        );
                        frame[6..8].copy_from_slice(&54u16.to_be_bytes());
                        assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                    })
                    .is_ok()
            };
            assert_eq!(valid, accepted, "{}", case["name"]);
        }
    }
    let mut header = b"OBWP".to_vec();
    header.extend(1u16.to_be_bytes());
    header.extend(55u16.to_be_bytes());
    header.extend((MAXIMUM_EVENTS_BYTES as u32 + 1).to_be_bytes());
    assert!(
        matches!(read_workbench_request(&mut header.as_slice()),Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge {actual,maximum})) if actual==MAXIMUM_EVENTS_BYTES+1 && maximum==MAXIMUM_EVENTS_BYTES)
    );
}
