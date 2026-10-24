use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    AttachmentControl, EncodedAttachmentControl, Envelope, MAXIMUM_CONTROL_JSON_BYTES, MessageRole,
    PayloadEncoding, ProtocolError, ProtocolErrorCode, decode_attachment_control,
    decode_transport_envelope, encode_attachment_control, encode_transport_envelope,
};
use serde_json::{Map, Value};

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance.json");

#[test]
fn every_canonical_attachment_payload_decodes_and_reencodes_byte_for_byte()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, pointer) in [
        (20, MessageRole::Request, "/commands/attach_terminal"),
        (21, MessageRole::Request, "/commands/detach_terminal"),
        (
            21,
            MessageRole::Response,
            "/outputs/attachment_acknowledged",
        ),
        (
            22,
            MessageRole::Response,
            "/outputs/attachment_capabilities",
        ),
        (23, MessageRole::Event, "/outputs/resize_lease_changed"),
        (
            24,
            MessageRole::Request,
            "/commands/update_attachment_input_capability",
        ),
        (25, MessageRole::Request, "/commands/claim_resize_lease"),
    ] {
        assert_canonical_round_trip(kind, role, fixture_string(&fixture, pointer)?)?;
    }
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_attachment_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let decoded = decode_fixture(
        &fixture,
        20,
        MessageRole::Request,
        "/commands/attach_terminal",
    )?;
    let AttachmentControl::AttachTerminal(attach) = decoded else {
        return Err(fixture_error("attach payload decoded to the wrong variant").into());
    };
    assert_eq!(attach.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(attach.terminal_id.to_text(), "44".repeat(16));
    assert!(attach.capabilities.input);
    assert!(attach.capabilities.read);
    assert!(!attach.capabilities.resize);
    assert!(!attach.request_resize_lease);

    let decoded = decode_fixture(
        &fixture,
        22,
        MessageRole::Response,
        "/outputs/attachment_capabilities",
    )?;
    let AttachmentControl::AttachmentCapabilities(capabilities) = decoded else {
        return Err(fixture_error("capability payload decoded to the wrong variant").into());
    };
    assert_eq!(capabilities.attachment_id, attach.attachment_id);
    assert_eq!(capabilities.terminal_id, attach.terminal_id);
    assert_eq!(
        capabilities.capability_revision,
        NonZeroU64::new(2).unwrap()
    );
    assert!(!capabilities.capabilities.input);
    assert!(!capabilities.capabilities.read);
    assert!(!capabilities.capabilities.resize);
    assert!(!capabilities.resize_lease);

    let decoded = decode_fixture(
        &fixture,
        23,
        MessageRole::Event,
        "/outputs/resize_lease_changed",
    )?;
    let AttachmentControl::ResizeLeaseChanged(changed) = decoded else {
        return Err(fixture_error("resize-lease payload decoded to the wrong variant").into());
    };
    assert_eq!(changed.attachment_id, attach.attachment_id);
    assert_eq!(changed.terminal_id, attach.terminal_id);
    assert!(changed.resize_lease);
    Ok(())
}

#[test]
fn equal_target_shapes_preserve_the_selected_attachment_operation() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/detach_terminal")?;
    let detach = decode_attachment_control(21, MessageRole::Request, target.as_bytes())?;
    let acknowledged = decode_attachment_control(21, MessageRole::Response, target.as_bytes())?;
    let claim = decode_attachment_control(25, MessageRole::Request, target.as_bytes())?;

    assert!(matches!(detach, AttachmentControl::DetachTerminal(_)));
    assert!(matches!(
        acknowledged,
        AttachmentControl::DetachTerminalAccepted(_)
    ));
    assert!(matches!(claim, AttachmentControl::ClaimResizeLease(_)));
    assert_ne!(detach, acknowledged);
    assert_ne!(acknowledged, claim);
    Ok(())
}

#[test]
fn attachment_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_attachment_control(20, MessageRole::Request, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_code(
        decode_attachment_control(20, MessageRole::Request, b"{"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_attachment_control(20, MessageRole::Request, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );

    let fixture = read_fixture()?;
    let attach = fixture_string(&fixture, "/commands/attach_terminal")?;
    let unknown = attach.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        20,
        MessageRole::Request,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    assert_payload_code(
        20,
        MessageRole::Request,
        "{}",
        ProtocolErrorCode::InvalidField,
    );

    let duplicate = attach.replacen(
        "\"attachmentId\":",
        "\"attachmentId\":\"22222222222222222222222222222222\",\"attachmentId\":",
        1,
    );
    assert_payload_code(
        20,
        MessageRole::Request,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = attach.replacen(
        "\"attachmentId\":",
        "\"attachmentId\":\"22222222222222222222222222222222\",\"attachment\u{5c}u0049d\":",
        1,
    );
    assert_payload_code(
        20,
        MessageRole::Request,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );

    let mut nested_missing: Value = serde_json::from_str(attach)?;
    object_mut_at(&mut nested_missing, "/capabilities")?.remove("read");
    assert_value_code(
        20,
        MessageRole::Request,
        nested_missing,
        ProtocolErrorCode::InvalidField,
    );
    let mut nested_unknown: Value = serde_json::from_str(attach)?;
    object_mut_at(&mut nested_unknown, "/capabilities")?
        .insert("extra".to_owned(), Value::Bool(true));
    assert_value_code(
        20,
        MessageRole::Request,
        nested_unknown,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn attachment_identities_are_canonical_and_nonzero() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let attach = fixture_string(&fixture, "/commands/attach_terminal")?;
    for (pointer, original) in [
        ("/attachmentId", "22".repeat(16)),
        ("/terminalId", "44".repeat(16)),
    ] {
        for invalid in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
            let mut value: Value = serde_json::from_str(attach)?;
            assert_eq!(
                value.pointer(pointer).and_then(Value::as_str),
                Some(original.as_str())
            );
            set_pointer(&mut value, pointer, Value::String(invalid))?;
            assert_value_code(
                20,
                MessageRole::Request,
                value,
                ProtocolErrorCode::InvalidIdentifier,
            );
        }
    }
    Ok(())
}

#[test]
fn admission_and_current_capabilities_have_distinct_invariants() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let attach = fixture_string(&fixture, "/commands/attach_terminal")?;
    for admitted in [
        [true, false, false],
        [false, true, false],
        [false, false, true],
        [true, true, true],
    ] {
        let mut value: Value = serde_json::from_str(attach)?;
        set_capabilities(&mut value, admitted)?;
        assert!(decode_value(20, MessageRole::Request, value).is_ok());
    }

    let mut empty: Value = serde_json::from_str(attach)?;
    set_capabilities(&mut empty, [false, false, false])?;
    assert_value_code(
        20,
        MessageRole::Request,
        empty,
        ProtocolErrorCode::InvalidField,
    );

    let mut lease_without_resize: Value = serde_json::from_str(attach)?;
    set_pointer(
        &mut lease_without_resize,
        "/requestResizeLease",
        Value::Bool(true),
    )?;
    assert_value_code(
        20,
        MessageRole::Request,
        lease_without_resize,
        ProtocolErrorCode::InvalidField,
    );

    let response = fixture_string(&fixture, "/outputs/attachment_capabilities")?;
    assert!(decode_attachment_control(22, MessageRole::Response, response.as_bytes()).is_ok());
    let mut invalid_response: Value = serde_json::from_str(response)?;
    set_pointer(&mut invalid_response, "/resizeLease", Value::Bool(true))?;
    assert_value_code(
        22,
        MessageRole::Response,
        invalid_response,
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn capability_revisions_are_positive_canonical_uint64_text() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let update = fixture_string(&fixture, "/commands/update_attachment_input_capability")?;
    for revision in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(update)?;
        set_pointer(
            &mut value,
            "/capabilityRevision",
            Value::String(revision.to_owned()),
        )?;
        assert_value_code(
            24,
            MessageRole::Request,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut numeric: Value = serde_json::from_str(update)?;
    set_pointer(&mut numeric, "/capabilityRevision", Value::from(2))?;
    assert_value_code(
        24,
        MessageRole::Request,
        numeric,
        ProtocolErrorCode::InvalidField,
    );
    for revision in ["1".to_owned(), u64::MAX.to_string()] {
        let mut value: Value = serde_json::from_str(update)?;
        set_pointer(&mut value, "/capabilityRevision", Value::String(revision))?;
        assert!(decode_value(24, MessageRole::Request, value).is_ok());
    }
    Ok(())
}

#[test]
fn public_attachment_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let decoded = decode_fixture(
        &fixture,
        20,
        MessageRole::Request,
        "/commands/attach_terminal",
    )?;
    let AttachmentControl::AttachTerminal(mut attach) = decoded else {
        return Err(fixture_error("attach payload decoded to the wrong variant").into());
    };
    attach.capabilities.input = false;
    attach.capabilities.read = false;
    assert_code(
        encode_attachment_control(&AttachmentControl::AttachTerminal(attach)),
        ProtocolErrorCode::InvalidField,
    );

    let decoded = decode_fixture(
        &fixture,
        22,
        MessageRole::Response,
        "/outputs/attachment_capabilities",
    )?;
    let AttachmentControl::AttachmentCapabilities(mut capabilities) = decoded else {
        return Err(fixture_error("capability payload decoded to the wrong variant").into());
    };
    capabilities.resize_lease = true;
    assert_code(
        encode_attachment_control(&AttachmentControl::AttachmentCapabilities(capabilities)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn attachment_kind_role_and_payload_bounds_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let target = fixture_string(&fixture, "/commands/detach_terminal")?;
    for (kind, role) in [
        (19, MessageRole::Event),
        (20, MessageRole::Event),
        (20, MessageRole::Response),
        (21, MessageRole::Event),
        (22, MessageRole::Request),
        (23, MessageRole::Response),
        (24, MessageRole::Response),
        (25, MessageRole::Response),
        (30, MessageRole::Request),
    ] {
        assert_payload_code(kind, role, target, ProtocolErrorCode::InvalidField);
    }

    let oversized = vec![b' '; MAXIMUM_CONTROL_JSON_BYTES + 1];
    assert_code(
        decode_attachment_control(21, MessageRole::Request, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn attachment_control_composes_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, role, pointer) in [
        (20, MessageRole::Request, "/commands/attach_terminal"),
        (
            22,
            MessageRole::Response,
            "/outputs/attachment_capabilities",
        ),
        (23, MessageRole::Event, "/outputs/resize_lease_changed"),
    ] {
        let value = decode_fixture(&fixture, kind, role, pointer)?;
        let EncodedAttachmentControl {
            kind,
            payload,
            role,
        } = encode_attachment_control(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: None,
            kind,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(
            decode_attachment_control(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn assert_canonical_round_trip(
    kind: u16,
    role: MessageRole,
    payload: &str,
) -> Result<(), Box<dyn Error>> {
    let decoded = decode_attachment_control(kind, role, payload.as_bytes())?;
    let encoded = encode_attachment_control(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, role);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn decode_fixture(
    fixture: &Value,
    kind: u16,
    role: MessageRole,
    pointer: &str,
) -> Result<AttachmentControl, Box<dyn Error>> {
    Ok(decode_attachment_control(
        kind,
        role,
        fixture_string(fixture, pointer)?.as_bytes(),
    )?)
}

fn decode_value(
    kind: u16,
    role: MessageRole,
    value: Value,
) -> Result<AttachmentControl, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_attachment_control(kind, role, &payload)
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_attachment_control(kind, role, payload.as_bytes()),
        expected,
    );
}

fn set_capabilities(value: &mut Value, capabilities: [bool; 3]) -> Result<(), io::Error> {
    for (name, enabled) in ["input", "read", "resize"].into_iter().zip(capabilities) {
        set_pointer(
            value,
            &format!("/capabilities/{name}"),
            Value::Bool(enabled),
        )?;
    }
    Ok(())
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?;
    *target = replacement;
    Ok(())
}

fn object_mut_at<'value>(
    value: &'value mut Value,
    pointer: &str,
) -> Result<&'value mut Map<String, Value>, io::Error> {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| fixture_error(&format!("fixture object {pointer} is missing")))
}

fn read_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
}

fn fixture_string<'value>(value: &'value Value, pointer: &str) -> Result<&'value str, io::Error> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| fixture_error(&format!("fixture string {pointer} is missing")))
}

fn fixture_error(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

fn assert_code<ValueType>(result: Result<ValueType, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}
