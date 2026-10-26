use super::*;

fn close_read() -> WorkbenchTabCloseRead {
    WorkbenchTabCloseRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
        session_id: SessionId::parse("22222222222222222222222222222222").unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse("33333333333333333333333333333333")
            .unwrap(),
        tab_id: WorkbenchTabId::parse("44444444444444444444444444444444").unwrap(),
    }
}

#[test]
fn close_generation_is_nonzero_canonical_and_never_wraps() {
    assert!(WorkbenchTabCloseAttemptGeneration::new(0).is_err());
    let first = WorkbenchTabCloseAttemptGeneration::new(1).unwrap();
    assert_eq!(first.checked_next().unwrap().value(), 2);
    let last = WorkbenchTabCloseAttemptGeneration::new(u64::MAX).unwrap();
    assert_eq!(last.checked_next(), None);
    assert_eq!(last.to_string(), "18446744073709551615");
    for value in [
        "",
        "0",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e0",
        " 1",
        "18446744073709551616",
    ] {
        assert!(
            WorkbenchTabCloseAttemptGeneration::parse(value).is_err(),
            "{value}"
        );
    }
    for value in [first, last] {
        let encoded = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            serde_json::from_slice::<WorkbenchTabCloseAttemptGeneration>(&encoded).unwrap(),
            value
        );
    }
}

#[test]
fn typed_close_controls_keep_exact_obwp_kinds_targets_and_small_bounds() {
    let read = close_read();
    let close_request_id = WorkbenchRequestId::parse("55555555555555555555555555555555").unwrap();
    let generation = WorkbenchTabCloseAttemptGeneration::new(1).unwrap();
    let resume = WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: read.request_id.clone(),
        session_id: read.session_id.clone(),
        terminal_context_id: read.terminal_context_id.clone(),
        tab_id: read.tab_id.clone(),
        close_request_id: close_request_id.clone(),
        expected_attempt_generation: generation,
    };
    for (kind, request) in [
        (5_u16, WorkbenchRequest::TabCloseRead(read.clone())),
        (7_u16, WorkbenchRequest::TabCloseResume(resume)),
    ] {
        let mut frame = Vec::new();
        crate::write_workbench_request(&mut frame, &request).unwrap();
        assert_eq!(&frame[6..8], &kind.to_be_bytes());
        assert_eq!(
            crate::read_workbench_request(&mut Cursor::new(&frame)).unwrap(),
            request
        );
        let mut wrong_direction = Cursor::new(&frame);
        assert!(matches!(
            crate::read_workbench_response(&mut wrong_direction),
            Err(crate::WorkbenchWireError::InvalidHeader)
        ));
        assert_eq!(wrong_direction.position(), 12);
        frame[7] = if kind == 5 { 7 } else { 5 };
        assert!(matches!(
            crate::read_workbench_request(&mut Cursor::new(&frame)),
            Err(crate::WorkbenchWireError::InvalidHeader)
        ));
        frame[7] = u8::try_from(kind).unwrap();
        frame[8..12].copy_from_slice(&16_385_u32.to_be_bytes());
        let mut oversized = Cursor::new(frame);
        assert!(matches!(
            crate::read_workbench_request(&mut oversized),
            Err(crate::WorkbenchWireError::Contract(
                ContractError::EncodedValueTooLarge { .. }
            ))
        ));
        assert_eq!(oversized.position(), 12);
        let mut encoded = serialize_workbench_request(&request).unwrap();
        encoded.resize(MAXIMUM_ENCODED_INTENT_BYTES, b' ');
        assert_eq!(parse_workbench_request(&encoded).unwrap(), request);
        encoded.push(b' ');
        assert!(matches!(
            parse_workbench_request(&encoded),
            Err(ContractError::EncodedValueTooLarge { .. })
        ));
    }
    for (kind, response) in [
        (
            6_u16,
            WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id.clone(),
                outcome: WorkbenchTabCloseReadOutcome::Available {
                    close_request_id: close_request_id.clone(),
                    terminal_context_id: read.terminal_context_id.clone(),
                    tab_id: read.tab_id.clone(),
                    attempt_generation: generation,
                    status: WorkbenchTabCloseStatus::Closed {
                        revision: WorkbenchRevision::new(9),
                    },
                },
            }),
        ),
        (
            8_u16,
            WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                session_id: read.session_id,
                terminal_context_id: read.terminal_context_id,
                tab_id: read.tab_id,
                close_request_id,
                outcome: WorkbenchTabCloseResumeOutcome::Denied {},
            }),
        ),
    ] {
        let mut frame = Vec::new();
        crate::write_workbench_response(&mut frame, &response).unwrap();
        assert_eq!(&frame[6..8], &kind.to_be_bytes());
        assert_eq!(
            crate::read_workbench_response(&mut Cursor::new(&frame)).unwrap(),
            response
        );
        for length in 0..frame.len() {
            assert!(crate::read_workbench_response(&mut Cursor::new(&frame[..length])).is_err());
        }
        let length = frame.len();
        frame.extend(frame.clone());
        let mut records = Cursor::new(&frame);
        assert_eq!(
            crate::read_workbench_response(&mut records).unwrap(),
            response
        );
        assert_eq!(records.position(), u64::try_from(length).unwrap());
        assert_eq!(
            crate::read_workbench_response(&mut records).unwrap(),
            response
        );
        let mut wrong_direction = Cursor::new(&frame);
        assert!(matches!(
            crate::read_workbench_request(&mut wrong_direction),
            Err(crate::WorkbenchWireError::InvalidHeader)
        ));
        assert_eq!(wrong_direction.position(), 12);
        frame[7] = if kind == 6 { 8 } else { 6 };
        assert!(matches!(
            crate::read_workbench_response(&mut Cursor::new(&frame)),
            Err(crate::WorkbenchWireError::InvalidHeader)
        ));
        frame[7] = u8::try_from(kind).unwrap();
        frame[8..12].copy_from_slice(&16_385_u32.to_be_bytes());
        let mut oversized = Cursor::new(frame);
        assert!(matches!(
            crate::read_workbench_response(&mut oversized),
            Err(crate::WorkbenchWireError::Contract(
                ContractError::EncodedValueTooLarge { .. }
            ))
        ));
        assert_eq!(oversized.position(), 12);
        let mut encoded = serialize_workbench_response(&response).unwrap();
        encoded.resize(MAXIMUM_ENCODED_INTENT_BYTES, b' ');
        assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
        encoded.push(b' ');
        assert!(matches!(
            parse_workbench_response(&encoded),
            Err(ContractError::EncodedValueTooLarge { .. })
        ));
    }
}
