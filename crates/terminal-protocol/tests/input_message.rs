use std::{error::Error, io};

use orchbee_terminal_protocol::{
    EncodedInputMessage, Envelope, InputMessage, MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_INPUT_BYTES,
    MAXIMUM_WRITE_INPUT_PAYLOAD_BYTES, MessageRole, PayloadEncoding, ProtocolError,
    ProtocolErrorCode, WRITE_INPUT_HEADER_BYTES, decode_input_message, decode_transport_envelope,
    encode_input_message, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.24.json");
const WRITE_INPUT_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/write-input-v0.0.24.bin");

#[test]
fn canonical_input_messages_decode_and_reencode_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let request = decode_input_message(30, MessageRole::Request, WRITE_INPUT_FIXTURE)?;
    let encoded = encode_input_message(&request)?;
    assert_eq!(encoded.kind, 30);
    assert_eq!(encoded.role, MessageRole::Request);
    assert_eq!(encoded.payload, WRITE_INPUT_FIXTURE);

    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    let accepted = decode_input_message(31, MessageRole::Response, response.as_bytes())?;
    let encoded = encode_input_message(&accepted)?;
    assert_eq!(encoded.kind, 31);
    assert_eq!(encoded.role, MessageRole::Response);
    assert_eq!(encoded.payload, response.as_bytes());
    Ok(())
}

#[test]
fn canonical_messages_retain_complete_input_facts() -> Result<(), Box<dyn Error>> {
    let request = decode_input_message(30, MessageRole::Request, WRITE_INPUT_FIXTURE)?;
    let InputMessage::WriteInput(write) = request else {
        return Err(fixture_error("raw fixture decoded to the wrong variant").into());
    };
    assert_eq!(write.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(write.capability_revision.get(), 7);
    assert_eq!(write.bytes, [0, 0x1b, 0xf0, 0x9f, 0x90, 0x8d]);
    assert_eq!(
        WRITE_INPUT_FIXTURE.get(16..24),
        Some(7_u64.to_le_bytes().as_slice())
    );

    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    let accepted = decode_input_message(31, MessageRole::Response, response.as_bytes())?;
    let InputMessage::InputAccepted(accepted) = accepted else {
        return Err(fixture_error("response fixture decoded to the wrong variant").into());
    };
    assert_eq!(accepted.accepted_bytes, 12);
    assert_eq!(accepted.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(accepted.capability_revision.get(), 1);
    Ok(())
}

#[test]
fn raw_input_body_bounds_are_exact_and_separate_from_the_prefix() -> Result<(), Box<dyn Error>> {
    assert_eq!(WRITE_INPUT_HEADER_BYTES, 24);
    assert_eq!(
        MAXIMUM_WRITE_INPUT_PAYLOAD_BYTES,
        WRITE_INPUT_HEADER_BYTES + MAXIMUM_INPUT_BYTES
    );

    let header_only = raw_payload(0x22, 1, 0);
    assert_code(
        decode_input_message(30, MessageRole::Request, &header_only),
        ProtocolErrorCode::Truncated,
    );
    let singleton = raw_payload(0x22, 1, 1);
    assert!(decode_input_message(30, MessageRole::Request, &singleton).is_ok());
    let maximum = raw_payload(0x22, 1, MAXIMUM_INPUT_BYTES);
    assert_eq!(maximum.len(), MAXIMUM_WRITE_INPUT_PAYLOAD_BYTES);
    assert!(decode_input_message(30, MessageRole::Request, &maximum).is_ok());
    let oversized = raw_payload(0x22, 1, MAXIMUM_INPUT_BYTES + 1);
    assert_code(
        decode_input_message(30, MessageRole::Request, &oversized),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn raw_input_identity_and_revision_are_nonzero_and_revision_is_little_endian()
-> Result<(), Box<dyn Error>> {
    let zero_identity = raw_payload(0, 1, 1);
    assert_code(
        decode_input_message(30, MessageRole::Request, &zero_identity),
        ProtocolErrorCode::InvalidIdentifier,
    );
    let zero_revision = raw_payload(0x22, 0, 1);
    assert_code(
        decode_input_message(30, MessageRole::Request, &zero_revision),
        ProtocolErrorCode::InvalidField,
    );

    let maximum_revision = raw_payload(0x22, u64::MAX, 1);
    let decoded = decode_input_message(30, MessageRole::Request, &maximum_revision)?;
    let InputMessage::WriteInput(write) = &decoded else {
        return Err(fixture_error("maximum revision decoded to the wrong variant").into());
    };
    assert_eq!(write.capability_revision.get(), u64::MAX);
    assert_eq!(maximum_revision.get(16..24), Some([0xff; 8].as_slice()));
    assert_eq!(encode_input_message(&decoded)?.payload, maximum_revision);
    Ok(())
}

#[test]
fn input_accepted_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_input_message(31, MessageRole::Response, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_code(
        decode_input_message(31, MessageRole::Response, b"{"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_code(
        decode_input_message(31, MessageRole::Response, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );

    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    assert_payload_code(
        31,
        MessageRole::Response,
        "{}",
        ProtocolErrorCode::InvalidField,
    );
    let unknown = response.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(
        31,
        MessageRole::Response,
        &unknown,
        ProtocolErrorCode::InvalidField,
    );
    let duplicate = response.replacen(
        "\"attachmentId\":",
        "\"attachmentId\":\"22222222222222222222222222222222\",\"attachmentId\":",
        1,
    );
    assert_payload_code(
        31,
        MessageRole::Response,
        &duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    let escaped_duplicate = response.replacen(
        "\"attachmentId\":",
        "\"attachmentId\":\"22222222222222222222222222222222\",\"attachment\u{5c}u0049d\":",
        1,
    );
    assert_payload_code(
        31,
        MessageRole::Response,
        &escaped_duplicate,
        ProtocolErrorCode::InvalidJson,
    );
    Ok(())
}

#[test]
fn accepted_byte_counts_obey_binary64_integer_and_input_bounds() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    for accepted in [0_u64, u64::try_from(MAXIMUM_INPUT_BYTES)?] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(&mut value, "/acceptedBytes", Value::from(accepted))?;
        assert!(decode_value(31, MessageRole::Response, value).is_ok());
    }
    for accepted in [
        Value::from(-1),
        Value::from(u64::try_from(MAXIMUM_INPUT_BYTES)? + 1),
        serde_json::from_str::<Value>("1.5")?,
        serde_json::from_str::<Value>("9007199254740992")?,
    ] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(&mut value, "/acceptedBytes", accepted)?;
        assert_value_code(
            31,
            MessageRole::Response,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    Ok(())
}

#[test]
fn accepted_identity_and_revision_obey_the_shared_refinements() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(
            31,
            MessageRole::Response,
            value,
            ProtocolErrorCode::InvalidIdentifier,
        );
    }
    for revision in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(response)?;
        set_pointer(
            &mut value,
            "/capabilityRevision",
            Value::String(revision.to_owned()),
        )?;
        assert_value_code(
            31,
            MessageRole::Response,
            value,
            ProtocolErrorCode::InvalidField,
        );
    }
    let mut maximum: Value = serde_json::from_str(response)?;
    set_pointer(
        &mut maximum,
        "/capabilityRevision",
        Value::String(u64::MAX.to_string()),
    )?;
    assert!(decode_value(31, MessageRole::Response, maximum).is_ok());
    Ok(())
}

#[test]
fn input_message_kind_role_and_encoding_boundaries_are_exact() {
    for (kind, role) in [
        (29, MessageRole::Request),
        (30, MessageRole::Response),
        (30, MessageRole::Event),
        (31, MessageRole::Request),
        (31, MessageRole::Event),
        (32, MessageRole::Request),
    ] {
        assert_code(
            decode_input_message(kind, role, WRITE_INPUT_FIXTURE),
            ProtocolErrorCode::InvalidField,
        );
    }
    let oversized_json = vec![b' '; MAXIMUM_CONTROL_JSON_BYTES + 1];
    assert_code(
        decode_input_message(31, MessageRole::Response, &oversized_json),
        ProtocolErrorCode::LimitExceeded,
    );
}

#[test]
fn public_input_values_are_revalidated_before_encoding() -> Result<(), Box<dyn Error>> {
    let decoded = decode_input_message(30, MessageRole::Request, WRITE_INPUT_FIXTURE)?;
    let InputMessage::WriteInput(mut write) = decoded else {
        return Err(fixture_error("raw fixture decoded to the wrong variant").into());
    };
    write.bytes.clear();
    assert_code(
        encode_input_message(&InputMessage::WriteInput(write.clone())),
        ProtocolErrorCode::InvalidField,
    );
    write.bytes = vec![0; MAXIMUM_INPUT_BYTES + 1];
    assert_code(
        encode_input_message(&InputMessage::WriteInput(write)),
        ProtocolErrorCode::InvalidField,
    );

    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    let decoded = decode_input_message(31, MessageRole::Response, response.as_bytes())?;
    let InputMessage::InputAccepted(mut accepted) = decoded else {
        return Err(fixture_error("response fixture decoded to the wrong variant").into());
    };
    accepted.accepted_bytes = u32::try_from(MAXIMUM_INPUT_BYTES)? + 1;
    assert_code(
        encode_input_message(&InputMessage::InputAccepted(accepted)),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn input_messages_compose_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let response = fixture_string(&fixture, "/outputs/input_accepted")?;
    for (value, encoding) in [
        (
            decode_input_message(30, MessageRole::Request, WRITE_INPUT_FIXTURE)?,
            PayloadEncoding::RawBytes,
        ),
        (
            decode_input_message(31, MessageRole::Response, response.as_bytes())?,
            PayloadEncoding::ControlJson,
        ),
    ] {
        let EncodedInputMessage {
            kind,
            payload,
            role,
        } = encode_input_message(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: None,
            kind,
            optional_message: false,
            payload: &payload,
            payload_encoding: encoding,
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(envelope.payload_encoding, encoding);
        assert_eq!(
            decode_input_message(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn raw_payload(identity_byte: u8, revision: u64, body_bytes: usize) -> Vec<u8> {
    let mut payload = Vec::with_capacity(WRITE_INPUT_HEADER_BYTES + body_bytes);
    payload.extend_from_slice(&[identity_byte; 16]);
    payload.extend_from_slice(&revision.to_le_bytes());
    payload.resize(WRITE_INPUT_HEADER_BYTES + body_bytes, 0x61);
    payload
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

fn decode_value(kind: u16, role: MessageRole, value: Value) -> Result<InputMessage, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_input_message(kind, role, &payload)
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error(&format!("fixture pointer {pointer} is missing")))?;
    *target = replacement;
    Ok(())
}

fn assert_value_code(kind: u16, role: MessageRole, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, role, value), expected);
}

fn assert_payload_code(kind: u16, role: MessageRole, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_input_message(kind, role, payload.as_bytes()),
        expected,
    );
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
