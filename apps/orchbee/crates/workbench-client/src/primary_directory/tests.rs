use super::*;
use orchbee_terminal_protocol::{
    AttachmentId, CorrelationId, Envelope, HostEpoch, TerminalId, TerminalProcess,
    TerminalProcessConsole, TerminalProcessInfo, encode_process_info_control,
    encode_transport_envelope,
};

/// The primary process and a child deliberately report different directories. Both carrier owners
/// use this wire fixture, so no test can accidentally accept the foreground child's cwd.
pub(crate) fn process_record(
    cwd: Option<&str>,
    wrong_identity: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let value = TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([if wrong_identity { 9 } else { 1 }; 16])?,
        terminal_id: TerminalId::from_bytes([2; 16])?,
        root_pid: 41,
        processes: vec![
            TerminalProcess {
                pid: 41,
                parent_pid: None,
                creation_time: 900,
                console: TerminalProcessConsole::Attached,
                image: Some("powershell.exe".into()),
                command_line: None,
                current_directory: cwd.map(str::to_owned),
            },
            TerminalProcess {
                pid: 42,
                parent_pid: Some(41),
                creation_time: 901,
                console: TerminalProcessConsole::Attached,
                image: Some("agent.exe".into()),
                command_line: None,
                current_directory: Some("C:/wrong-child".into()),
            },
        ],
    };
    let encoded = encode_process_info_control(&ProcessInfoControl::ProcessInfo(value))?;
    Ok(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind: encoded.kind,
        optional_message: false,
        payload_encoding: PayloadEncoding::ControlJson,
        correlation_id: Some(CorrelationId::from_bytes([3; 16])?),
        host_epoch: Some(HostEpoch::from_bytes([4; 16])?),
        payload: &encoded.payload,
    })?)
}

#[test]
fn unavailable_and_mismatched_root_samples_never_become_an_absent_cwd() {
    let id = orchbee_client_server_protocol::ViewerId::parse(&"01".repeat(16)).unwrap();
    let epoch = orchbee_client_server_protocol::HostEpoch::parse(&"04".repeat(16)).unwrap();
    let terminal = orchbee_client_server_protocol::HostTerminalId::parse(&"02".repeat(16)).unwrap();
    let absent = TerminalViewerOutcome::Record(process_record(None, false).unwrap());
    assert_eq!(
        decode_root(absent, &id, &epoch, &terminal),
        Ok(RootDirectory::Available(None))
    );
    let relative = TerminalViewerOutcome::Record(process_record(Some("relative"), false).unwrap());
    assert_eq!(
        decode_root(relative, &id, &epoch, &terminal),
        Ok(RootDirectory::Unavailable)
    );
    let mismatched = TerminalViewerOutcome::Record(process_record(Some("C:/repo"), true).unwrap());
    assert_eq!(
        decode_root(mismatched, &id, &epoch, &terminal),
        Err(InvalidRootSample)
    );
    let wrong_epoch = orchbee_client_server_protocol::HostEpoch::parse(&"05".repeat(16)).unwrap();
    let record = TerminalViewerOutcome::Record(process_record(None, false).unwrap());
    assert_eq!(
        decode_root(record, &id, &wrong_epoch, &terminal),
        Err(InvalidRootSample)
    );
}
