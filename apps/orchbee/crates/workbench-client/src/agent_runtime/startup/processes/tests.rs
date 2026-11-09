//! Host process-envelope acceptance and declared failure codes, independent of CLI rendering.
use super::*;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, TerminalId, TerminalProcess,
    TerminalProcessConsole, TypedFailure, encode_connection_control, encode_process_info_control,
    encode_transport_envelope,
};

fn info() -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 41,
        processes: vec![TerminalProcess {
            pid: 41,
            parent_pid: None,
            creation_time: u64::MAX,
            console: TerminalProcessConsole::Attached,
            image: Some("C:\\Windows\\cmd.exe".into()),
            command_line: Some("cmd /C echo 中 😀\u{1b}]2;title\u{7}".into()),
            current_directory: None,
        }],
        launch_kind: None,
    }
}

fn record(kind: u16, payload: &[u8], change: impl FnOnce(&mut Envelope<'_>)) -> Vec<u8> {
    let mut envelope = Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind,
        optional_message: false,
        payload_encoding: PayloadEncoding::ControlJson,
        correlation_id: Some(CorrelationId::from_bytes([3; 16]).unwrap()),
        host_epoch: Some(HostEpoch::from_bytes([4; 16]).unwrap()),
        payload,
    };
    change(&mut envelope);
    encode_transport_envelope(&envelope).unwrap()
}

fn read(record: &[u8]) -> Result<(TerminalProcessInfo, String), Error> {
    decode(
        record,
        info().attachment_id,
        HostEpoch::from_bytes([4; 16]).unwrap(),
    )
}

#[test]
fn process_info_rejects_wrong_envelope_attachment_and_malformed_payload() {
    let encoded = encode_process_info_control(&ProcessInfoControl::ProcessInfo(info())).unwrap();
    let valid = record(encoded.kind, &encoded.payload, |_| {});
    assert_eq!(read(&valid).unwrap().0, info());
    let epoch = record(encoded.kind, &encoded.payload, |e| {
        e.host_epoch = Some(HostEpoch::from_bytes([5; 16]).unwrap())
    });
    let role = record(encoded.kind, &encoded.payload, |e| {
        e.role = MessageRole::Request
    });
    let malformed = record(encoded.kind, b"{}", |_| {});
    let mut wrong = info();
    wrong.attachment_id = AttachmentId::from_bytes([6; 16]).unwrap();
    let wrong = encode_process_info_control(&ProcessInfoControl::ProcessInfo(wrong)).unwrap();
    let attachment = record(wrong.kind, &wrong.payload, |_| {});
    for bad in [&epoch, &role, &malformed, &attachment, &vec![0; 65536 + 65]] {
        assert!(matches!(read(bad).unwrap_err(), Error::Protocol));
    }
}

#[test]
fn process_info_preserves_only_declared_host_failure_codes() {
    for (code, expected) in [
        ("limit-exceeded", Some(HostFailure::LimitExceeded)),
        ("missing-resource", Some(HostFailure::MissingResource)),
        ("process-sample-unsupported", Some(HostFailure::Unsupported)),
        ("process-sample-changed", None),
        ("process-sample-unavailable", Some(HostFailure::Unavailable)),
        ("invalid-field", None),
    ] {
        let encoded = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
            code: code.into(),
            message: "untrusted Host diagnostic".into(),
            retryable: false,
            reason: None,
        }))
        .unwrap();
        let failure = read(&record(encoded.kind, &encoded.payload, |_| {})).unwrap_err();
        match expected {
            Some(expected) => assert!(matches!(failure, Error::Host(actual) if actual == expected)),
            None => assert!(matches!(failure, Error::Protocol)),
        }
    }
}
