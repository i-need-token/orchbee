use orchbee_terminal_protocol::{
    AttachmentId, AttachmentTarget, CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch,
    MessageRole, PayloadEncoding, ProcessInfoControl, TerminalId, TerminalProcess,
    TerminalProcessInfo, decode_process_info_control, decode_transport_envelope,
    encode_process_info_control, encode_transport_envelope,
};

fn sample() -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 41,
        processes: vec![TerminalProcess {
            pid: 41,
            parent_pid: Some(17),
            creation_time: 133_999_999_999_999_999,
            image: Some(r"C:\Windows\System32\cmd.exe".into()),
            command_line: Some("cmd /C echo 中文 😀".into()),
            current_directory: None,
        }],
    }
}

#[test]
fn typescript_process_fixtures_refine_to_the_same_typed_rust_contract() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.25.json"
    ))
    .unwrap();
    for (pointer, role) in [
        ("/commands/read_process_info", MessageRole::Request),
        ("/outputs/process_info", MessageRole::Response),
    ] {
        let payload = fixture.pointer(pointer).unwrap().as_str().unwrap();
        let decoded = decode_process_info_control(79, role, payload.as_bytes()).unwrap();
        let encoded = encode_process_info_control(&decoded).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&encoded.payload).unwrap(),
            serde_json::from_str::<serde_json::Value>(payload).unwrap()
        );
    }
}

#[test]
fn correlated_process_request_and_owned_sample_round_trip_without_losing_filetime() {
    for value in [
        ProcessInfoControl::ReadProcessInfo(AttachmentTarget {
            attachment_id: sample().attachment_id,
        }),
        ProcessInfoControl::ProcessInfo(sample()),
    ] {
        let encoded = encode_process_info_control(&value).unwrap();
        let mut envelope = Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            role: encoded.role,
            kind: encoded.kind,
            optional_message: false,
            payload_encoding: PayloadEncoding::ControlJson,
            correlation_id: Some(CorrelationId::from_bytes([3; 16]).unwrap()),
            host_epoch: Some(HostEpoch::from_bytes([4; 16]).unwrap()),
            payload: &encoded.payload,
        };
        let record = encode_transport_envelope(&envelope).unwrap();
        let decoded = decode_transport_envelope(&record).unwrap();
        assert_eq!(decoded.kind, envelope.kind);
        assert_eq!(decoded.role, envelope.role);
        assert_eq!(decoded.correlation_id, envelope.correlation_id);
        assert_eq!(decoded.host_epoch, envelope.host_epoch);
        assert_eq!(
            decode_process_info_control(decoded.kind, decoded.role, decoded.payload).unwrap(),
            value
        );
        assert!(decode_process_info_control(79, MessageRole::Event, &encoded.payload).is_err());
        envelope.role = MessageRole::Event;
        assert!(encode_transport_envelope(&envelope).is_err());
    }
    let mut ended = sample();
    ended.processes.clear();
    assert!(encode_process_info_control(&ProcessInfoControl::ProcessInfo(ended)).is_ok());
}

#[test]
fn process_reply_rejects_partial_fields_unknown_fields_invalid_identity_and_noncanonical_rows() {
    let encoded = encode_process_info_control(&ProcessInfoControl::ProcessInfo(sample())).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&encoded.payload).unwrap();
    for field in ["parentPid", "image", "commandLine", "currentDirectory"] {
        let mut bad = original.clone();
        bad["processes"][0].as_object_mut().unwrap().remove(field);
        assert!(
            decode_process_info_control(
                79,
                MessageRole::Response,
                &serde_json::to_vec(&bad).unwrap()
            )
            .is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("pid", serde_json::json!(0)),
        ("pid", serde_json::json!(4_294_967_296_u64)),
        ("parentPid", serde_json::json!(0)),
        ("creationTime", serde_json::json!("01")),
        ("creationTime", serde_json::json!("0")),
        ("creationTime", serde_json::json!("18446744073709551616")),
        ("commandLine", serde_json::json!("a\0b")),
        ("extra", serde_json::json!(true)),
    ] {
        let mut bad = original.clone();
        bad["processes"][0][field] = value;
        assert!(
            decode_process_info_control(
                79,
                MessageRole::Response,
                &serde_json::to_vec(&bad).unwrap()
            )
            .is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("attachmentId", serde_json::json!("0".repeat(32))),
        ("terminalId", serde_json::json!("F".repeat(32))),
        ("kind", serde_json::json!("foreground")),
        ("rootPid", serde_json::json!(0)),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        assert!(
            decode_process_info_control(
                79,
                MessageRole::Response,
                &serde_json::to_vec(&bad).unwrap()
            )
            .is_err()
        );
    }
    let duplicate = String::from_utf8(encoded.payload).unwrap().replacen(
        "\"rootPid\":41",
        "\"rootPid\":41,\"rootPid\":42",
        1,
    );
    assert!(decode_process_info_control(79, MessageRole::Response, duplicate.as_bytes()).is_err());
}

#[test]
fn row_membership_and_text_bounds_are_enforced_on_both_sides() {
    for count in [0, 1, 64, 65] {
        let mut value = sample();
        value.processes = (0..count)
            .map(|index| {
                let mut row = sample().processes.remove(0);
                row.pid = index + 1;
                row
            })
            .collect();
        assert_eq!(
            encode_process_info_control(&ProcessInfoControl::ProcessInfo(value)).is_ok(),
            count <= 64
        );
    }
    let mut duplicate = sample();
    duplicate.processes.push(duplicate.processes[0].clone());
    assert!(encode_process_info_control(&ProcessInfoControl::ProcessInfo(duplicate)).is_err());
    for (text, accepted) in [("😀".repeat(2048), true), ("😀".repeat(2048) + "x", false)] {
        let mut value = sample();
        value.processes[0].command_line = Some(text);
        assert_eq!(
            encode_process_info_control(&ProcessInfoControl::ProcessInfo(value)).is_ok(),
            accepted
        );
    }
    let mut too_large = sample();
    too_large.processes = (1..=64)
        .map(|pid| TerminalProcess {
            pid,
            command_line: Some("\n".repeat(4096)),
            ..sample().processes.remove(0)
        })
        .collect();
    assert!(encode_process_info_control(&ProcessInfoControl::ProcessInfo(too_large)).is_err());
}
