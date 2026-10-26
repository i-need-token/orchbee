use orchbee_workbench_protocol::*;

#[test]
fn configured_commands_match_shared_shapes_semantics_and_closed_carrier() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-configured-command-conformance-v1.json"
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
                        assert_eq!(&frame[6..8], &53u16.to_be_bytes());
                        assert_eq!(
                            read_workbench_request(&mut frame.as_slice()).unwrap(),
                            request
                        );
                        frame[6..8].copy_from_slice(&51u16.to_be_bytes());
                        assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                    })
                    .is_ok()
            } else {
                parse_workbench_response(&bytes)
                    .map(|response| {
                        let mut frame = Vec::new();
                        write_workbench_response(&mut frame, &response).unwrap();
                        assert_eq!(&frame[6..8], &54u16.to_be_bytes());
                        assert_eq!(
                            read_workbench_response(&mut frame.as_slice()).unwrap(),
                            response
                        );
                        frame[6..8].copy_from_slice(&52u16.to_be_bytes());
                        assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                    })
                    .is_ok()
            };
            assert_eq!(valid, accepted, "{}", case["name"]);
        }
    }
    let mut header = b"OBWP".to_vec();
    header.extend(1u16.to_be_bytes());
    header.extend(53u16.to_be_bytes());
    header.extend((MAXIMUM_CONFIGURED_COMMAND_BYTES as u32 + 1).to_be_bytes());
    assert!(
        matches!(read_workbench_request(&mut header.as_slice()),Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge {actual,maximum})) if actual==MAXIMUM_CONFIGURED_COMMAND_BYTES+1 && maximum==MAXIMUM_CONFIGURED_COMMAND_BYTES)
    );
}

#[test]
fn configured_popup_read_correlates_a_retained_closing_owner() {
    let request = ConfiguredCommandRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        action: ConfiguredCommandAction::PopupRead {
            popup_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
        },
    };
    let mut result = ConfiguredCommandResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome: ConfiguredCommandOutcome::PopupClosing {
            popup_id: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
        },
    };
    assert!(result.answers(&request));
    result.outcome = ConfiguredCommandOutcome::PopupClosing {
        popup_id: WorkbenchRequestId::parse(&format!("{:032x}", 4)).unwrap(),
    };
    assert!(!result.answers(&request));
}
