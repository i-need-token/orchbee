use super::*;
use std::io::Cursor;

fn request() -> WorkbenchRequest {
    parse_workbench_request(br#"{"kind":"workbench.snapshot.read","protocolVersion":1,"requestId":"11111111111111111111111111111111"}"#).unwrap()
}

#[test]
fn a_complete_record_leaves_the_next_record_unread_and_rejects_eof_truncation() {
    let request = request();
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &request).unwrap();
    for length in 0..frame.len() {
        assert!(read_workbench_request(&mut Cursor::new(&frame[..length])).is_err());
    }
    let length = frame.len();
    frame.extend(frame.clone());
    let mut reader = Cursor::new(frame);
    assert_eq!(read_workbench_request(&mut reader).unwrap(), request);
    assert_eq!(reader.position(), u64::try_from(length).unwrap());
    assert_eq!(read_workbench_request(&mut reader).unwrap(), request);
}

#[test]
fn direction_kind_version_and_size_fail_before_body_allocation_or_read() {
    let mut frame = Vec::new();
    write_workbench_request(&mut frame, &request()).unwrap();
    let mut reader = Cursor::new(frame.clone());
    assert!(matches!(
        read_workbench_response(&mut reader),
        Err(WorkbenchWireError::InvalidHeader)
    ));
    assert_eq!(reader.position(), 12);
    // Use an unassigned kind: 15 is now workspace.close.read.
    for (offset, byte) in [(0, b'X'), (5, 2), (7, u8::MAX)] {
        let mut invalid = frame.clone();
        invalid[offset] = byte;
        let mut reader = Cursor::new(invalid);
        assert!(matches!(
            read_workbench_request(&mut reader),
            Err(WorkbenchWireError::InvalidHeader)
        ));
        assert_eq!(reader.position(), 12);
    }
    let mut invalid = frame.clone();
    invalid[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    let mut reader = Cursor::new(invalid);
    assert!(matches!(
        read_workbench_request(&mut reader),
        Err(WorkbenchWireError::Contract(
            ContractError::EncodedValueTooLarge { .. }
        ))
    ));
    assert_eq!(reader.position(), 12);
    frame[7] = 2;
    assert!(matches!(
        read_workbench_request(&mut Cursor::new(frame)),
        Err(WorkbenchWireError::InvalidHeader)
    ));
}

#[test]
fn tab_create_and_started_result_roundtrip_through_their_obwp_kinds() {
    let request = parse_workbench_request(
        br#"{"kind":"workbench.command","protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","expectedRevision":"7","action":{"kind":"tab.create","terminalContextId":"33333333333333333333333333333333","label":"2","geometry":{"columns":120.0,"rows":3.6e1,"cellPixelWidth":9,"cellPixelHeight":18}}}"#,
    )
    .unwrap();
    let mut request_frame = Vec::new();
    write_workbench_request(&mut request_frame, &request).unwrap();
    assert_eq!(&request_frame[6..8], &2_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut Cursor::new(request_frame)).unwrap(),
        request
    );

    let response = parse_workbench_response(
        br#"{"kind":"workbench.command.result","protocolVersion":1,"requestId":"11111111111111111111111111111111","outcome":{"kind":"tab-created","revision":"8","terminalContextId":"33333333333333333333333333333333","tabId":"44444444444444444444444444444444","panelId":"55555555555555555555555555555555","presentationId":"66666666666666666666666666666666","terminalOutcome":{"kind":"started","geometry":{"columns":120.0,"rows":3.6e1,"cellPixelWidth":9,"cellPixelHeight":18}}}}"#,
    )
    .unwrap();
    let mut response_frame = Vec::new();
    write_workbench_response(&mut response_frame, &response).unwrap();
    assert_eq!(&response_frame[6..8], &4_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut Cursor::new(response_frame)).unwrap(),
        response
    );
}
