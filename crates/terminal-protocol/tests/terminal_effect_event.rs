use std::{error::Error, io, num::NonZeroU64};

use orchbee_terminal_protocol::{
    AttachmentId, CreationId, EncodedTerminalEffectEvent, Envelope, HostEpoch,
    MAXIMUM_CLIPBOARD_TEXT_BYTES, MAXIMUM_CONTROL_JSON_BYTES,
    MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES, MessageRole, PayloadEncoding, ProtocolError,
    ProtocolErrorCode, TerminalApplicationSignal, TerminalApplicationSignalChannel, TerminalBell,
    TerminalClipboardWriteRequest, TerminalEffectEvent, TerminalId, TerminalIdentity,
    TerminalMetadataChanged, decode_terminal_effect_event, decode_transport_envelope,
    encode_terminal_effect_event, encode_transport_envelope,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/control-conformance.json");
const METADATA_EXAMPLE: &str = concat!(
    "{\"currentDirectory\":\"file://orchbee-host/workspace\",\"identity\":{",
    "\"creationId\":\"11111111111111111111111111111111\",",
    "\"hostEpoch\":\"22222222222222222222222222222222\",",
    "\"terminalId\":\"33333333333333333333333333333333\"},",
    "\"revision\":\"7\",\"title\":\"background agent\"}",
);

#[test]
fn canonical_terminal_effect_payloads_round_trip_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, pointer) in [
        (55, "/outputs/terminal_bell"),
        (56, "/outputs/terminal_clipboard_write_request"),
        (58, "/outputs/terminal_application_signal"),
    ] {
        assert_canonical_round_trip(kind, fixture_string(&fixture, pointer)?)?;
    }
    assert_canonical_round_trip(57, METADATA_EXAMPLE)?;
    Ok(())
}

#[test]
fn canonical_payloads_retain_complete_scoped_effect_facts() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;

    let bell = decode_terminal_effect_event(
        55,
        MessageRole::Event,
        fixture_string(&fixture, "/outputs/terminal_bell")?.as_bytes(),
    )?;
    let TerminalEffectEvent::Bell(bell) = bell else {
        return Err(fixture_error("bell decoded to the wrong variant").into());
    };
    assert_eq!(bell.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(bell.sequence.get(), 6);

    let clipboard = decode_terminal_effect_event(
        56,
        MessageRole::Event,
        fixture_string(&fixture, "/outputs/terminal_clipboard_write_request")?.as_bytes(),
    )?;
    let TerminalEffectEvent::ClipboardWriteRequest(clipboard) = clipboard else {
        return Err(fixture_error("clipboard event decoded to the wrong variant").into());
    };
    assert_eq!(clipboard.attachment_id.to_text(), "22".repeat(16));
    assert_eq!(clipboard.sequence.get(), 7);
    assert_eq!(clipboard.text, "copied from terminal");

    let metadata =
        decode_terminal_effect_event(57, MessageRole::Event, METADATA_EXAMPLE.as_bytes())?;
    let TerminalEffectEvent::MetadataChanged(metadata) = metadata else {
        return Err(fixture_error("metadata event decoded to the wrong variant").into());
    };
    assert_eq!(
        metadata.current_directory.as_deref(),
        Some("file://orchbee-host/workspace")
    );
    assert_eq!(metadata.identity.creation_id.to_text(), "11".repeat(16));
    assert_eq!(metadata.identity.host_epoch.to_text(), "22".repeat(16));
    assert_eq!(metadata.identity.terminal_id.to_text(), "33".repeat(16));
    assert_eq!(metadata.revision.get(), 7);
    assert_eq!(metadata.title.as_deref(), Some("background agent"));

    let signal = decode_terminal_effect_event(
        58,
        MessageRole::Event,
        fixture_string(&fixture, "/outputs/terminal_application_signal")?.as_bytes(),
    )?;
    let TerminalEffectEvent::ApplicationSignal(signal) = signal else {
        return Err(fixture_error("application signal decoded to the wrong variant").into());
    };
    assert_eq!(
        signal.channel,
        TerminalApplicationSignalChannel::OrchbeeAgentV1
    );
    assert_eq!(signal.identity.creation_id.to_text(), "33".repeat(16));
    assert_eq!(signal.identity.host_epoch.to_text(), "11".repeat(16));
    assert_eq!(signal.identity.terminal_id.to_text(), "44".repeat(16));
    assert_eq!(signal.payload, "{\"activity\":\"command\",\"version\":1}");
    assert_eq!(signal.sequence.get(), 8);
    Ok(())
}

#[test]
fn terminal_effect_json_is_utf8_unique_complete_and_closed() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_terminal_effect_event(55, MessageRole::Event, &[0xff]),
        ProtocolErrorCode::InvalidUtf8,
    );
    assert_payload_code(55, "{", ProtocolErrorCode::InvalidJson);
    assert_code(
        decode_terminal_effect_event(55, MessageRole::Event, b"\xef\xbb\xbf{}"),
        ProtocolErrorCode::InvalidJson,
    );
    assert_payload_code(55, "{}", ProtocolErrorCode::InvalidField);

    let fixture = read_fixture()?;
    let bell = fixture_string(&fixture, "/outputs/terminal_bell")?;
    let unknown = bell.replacen('}', ",\"extra\":true}", 1);
    assert_payload_code(55, &unknown, ProtocolErrorCode::InvalidField);
    let duplicate = bell.replacen("\"sequence\":", "\"sequence\":\"5\",\"sequence\":", 1);
    assert_payload_code(55, &duplicate, ProtocolErrorCode::InvalidJson);
    let escaped_duplicate =
        bell.replacen("\"sequence\":", "\"sequence\":\"5\",\"sequ\\u0065nce\":", 1);
    assert_payload_code(55, &escaped_duplicate, ProtocolErrorCode::InvalidJson);

    let nested_duplicate = METADATA_EXAMPLE.replacen(
        "\"terminalId\":",
        "\"terminalId\":\"44444444444444444444444444444444\",\"terminal\\u0049d\":",
        1,
    );
    assert_payload_code(57, &nested_duplicate, ProtocolErrorCode::InvalidJson);

    let mut missing: Value = serde_json::from_str(METADATA_EXAMPLE)?;
    object_mut(&mut missing)?.remove("title");
    assert_value_code(57, missing, ProtocolErrorCode::InvalidField);
    Ok(())
}

#[test]
fn terminal_effect_identity_and_positive_decimal_refinements_are_exact()
-> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let bell = fixture_string(&fixture, "/outputs/terminal_bell")?;
    for identity in ["00".repeat(16), "AA".repeat(16), "1".repeat(31)] {
        let mut value: Value = serde_json::from_str(bell)?;
        set_pointer(&mut value, "/attachmentId", Value::String(identity))?;
        assert_value_code(55, value, ProtocolErrorCode::InvalidIdentifier);
    }

    for invalid in ["0", "00", "-1", "+1", "18446744073709551616", " 1"] {
        let mut value: Value = serde_json::from_str(bell)?;
        set_pointer(&mut value, "/sequence", Value::String(invalid.to_owned()))?;
        assert_value_code(55, value, ProtocolErrorCode::InvalidField);
    }
    let mut maximum: Value = serde_json::from_str(bell)?;
    set_pointer(
        &mut maximum,
        "/sequence",
        Value::String(u64::MAX.to_string()),
    )?;
    assert!(decode_value(55, maximum).is_ok());

    let mut numeric: Value = serde_json::from_str(bell)?;
    set_pointer(&mut numeric, "/sequence", Value::from(6))?;
    assert_value_code(55, numeric, ProtocolErrorCode::InvalidField);

    for pointer in [
        "/identity/creationId",
        "/identity/hostEpoch",
        "/identity/terminalId",
    ] {
        let mut value: Value = serde_json::from_str(METADATA_EXAMPLE)?;
        set_pointer(&mut value, pointer, Value::String("00".repeat(16)))?;
        assert_value_code(57, value, ProtocolErrorCode::InvalidIdentifier);
    }
    for invalid in ["0", "01", "-1", "18446744073709551616"] {
        let mut value: Value = serde_json::from_str(METADATA_EXAMPLE)?;
        set_pointer(&mut value, "/revision", Value::String(invalid.to_owned()))?;
        assert_value_code(57, value, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

#[test]
fn application_signal_channel_is_the_exact_closed_literal() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let signal = fixture_string(&fixture, "/outputs/terminal_application_signal")?;
    for invalid in ["", "orchbee.agent.v3", "ORCHBEE.AGENT.V1", "future"] {
        let mut value: Value = serde_json::from_str(signal)?;
        set_pointer(&mut value, "/channel", Value::String(invalid.to_owned()))?;
        assert_value_code(58, value, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

#[test]
fn terminal_effect_text_bounds_use_utf8_bytes_and_reject_nul() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    let clipboard = fixture_string(&fixture, "/outputs/terminal_clipboard_write_request")?;
    for text in [
        String::new(),
        "x".repeat(MAXIMUM_CLIPBOARD_TEXT_BYTES),
        "é".repeat(MAXIMUM_CLIPBOARD_TEXT_BYTES / 2),
    ] {
        let mut value: Value = serde_json::from_str(clipboard)?;
        set_pointer(&mut value, "/text", Value::String(text))?;
        assert!(decode_value(56, value).is_ok());
    }
    for text in [
        "x".repeat(MAXIMUM_CLIPBOARD_TEXT_BYTES + 1),
        "é".repeat(MAXIMUM_CLIPBOARD_TEXT_BYTES / 2 + 1),
        "bad\0text".to_owned(),
    ] {
        let mut value: Value = serde_json::from_str(clipboard)?;
        set_pointer(&mut value, "/text", Value::String(text))?;
        assert_value_code(56, value, ProtocolErrorCode::InvalidField);
    }

    let signal = fixture_string(&fixture, "/outputs/terminal_application_signal")?;
    for payload in [
        String::new(),
        "x".repeat(MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES),
        "é".repeat(MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES / 2),
    ] {
        let mut value: Value = serde_json::from_str(signal)?;
        set_pointer(&mut value, "/payload", Value::String(payload))?;
        assert!(decode_value(58, value).is_ok());
    }
    for payload in [
        "x".repeat(MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES + 1),
        "é".repeat(MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES / 2 + 1),
        "bad\0payload".to_owned(),
    ] {
        let mut value: Value = serde_json::from_str(signal)?;
        set_pointer(&mut value, "/payload", Value::String(payload))?;
        assert_value_code(58, value, ProtocolErrorCode::InvalidField);
    }

    for pointer in ["/currentDirectory", "/title"] {
        let mut empty: Value = serde_json::from_str(METADATA_EXAMPLE)?;
        set_pointer(&mut empty, pointer, Value::String(String::new()))?;
        assert!(decode_value(57, empty).is_ok());

        let mut absent: Value = serde_json::from_str(METADATA_EXAMPLE)?;
        set_pointer(&mut absent, pointer, Value::Null)?;
        assert!(decode_value(57, absent).is_ok());

        let mut nul: Value = serde_json::from_str(METADATA_EXAMPLE)?;
        set_pointer(&mut nul, pointer, Value::String("bad\0metadata".to_owned()))?;
        assert_value_code(57, nul, ProtocolErrorCode::InvalidField);
    }
    Ok(())
}

#[test]
fn public_terminal_effect_values_are_revalidated_and_all_variants_round_trip()
-> Result<(), Box<dyn Error>> {
    let attachment_id = AttachmentId::parse(&"22".repeat(16))?;
    let identity = example_identity()?;
    let sequence = NonZeroU64::new(1).ok_or_else(|| fixture_error("one must be nonzero"))?;

    for value in [
        TerminalEffectEvent::Bell(TerminalBell {
            attachment_id,
            sequence,
        }),
        TerminalEffectEvent::ClipboardWriteRequest(TerminalClipboardWriteRequest {
            attachment_id,
            sequence,
            text: String::new(),
        }),
        TerminalEffectEvent::MetadataChanged(TerminalMetadataChanged {
            current_directory: None,
            identity,
            revision: sequence,
            title: Some(String::new()),
        }),
        TerminalEffectEvent::ApplicationSignal(TerminalApplicationSignal {
            channel: TerminalApplicationSignalChannel::OrchbeeAgentV1,
            identity,
            payload: String::new(),
            sequence,
        }),
    ] {
        assert_public_round_trip(value)?;
    }

    assert_code(
        encode_terminal_effect_event(&TerminalEffectEvent::ClipboardWriteRequest(
            TerminalClipboardWriteRequest {
                attachment_id,
                sequence,
                text: "x".repeat(MAXIMUM_CLIPBOARD_TEXT_BYTES + 1),
            },
        )),
        ProtocolErrorCode::InvalidField,
    );
    assert_code(
        encode_terminal_effect_event(&TerminalEffectEvent::ApplicationSignal(
            TerminalApplicationSignal {
                channel: TerminalApplicationSignalChannel::OrchbeeAgentV1,
                identity,
                payload: "bad\0payload".to_owned(),
                sequence,
            },
        )),
        ProtocolErrorCode::InvalidField,
    );
    assert_code(
        encode_terminal_effect_event(&TerminalEffectEvent::MetadataChanged(
            TerminalMetadataChanged {
                current_directory: Some("bad\0directory".to_owned()),
                identity,
                revision: sequence,
                title: None,
            },
        )),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn metadata_null_and_escaped_text_encoding_is_canonical() -> Result<(), Box<dyn Error>> {
    let identity = example_identity()?;
    let revision = NonZeroU64::new(9).ok_or_else(|| fixture_error("nine must be nonzero"))?;
    let value = TerminalEffectEvent::MetadataChanged(TerminalMetadataChanged {
        current_directory: None,
        identity,
        revision,
        title: Some("agent \"one\"\nready".to_owned()),
    });
    let encoded = encode_terminal_effect_event(&value)?;
    assert_eq!(
        String::from_utf8(encoded.payload.clone())?,
        concat!(
            "{\"currentDirectory\":null,\"identity\":{",
            "\"creationId\":\"11111111111111111111111111111111\",",
            "\"hostEpoch\":\"22222222222222222222222222222222\",",
            "\"terminalId\":\"33333333333333333333333333333333\"},",
            "\"revision\":\"9\",\"title\":\"agent \\\"one\\\"\\nready\"}",
        )
    );
    assert_eq!(
        decode_terminal_effect_event(encoded.kind, encoded.role, &encoded.payload)?,
        value
    );
    Ok(())
}

#[test]
fn terminal_effect_kind_role_and_control_json_boundaries_are_exact() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, pointer) in [
        (55, "/outputs/terminal_bell"),
        (56, "/outputs/terminal_clipboard_write_request"),
        (58, "/outputs/terminal_application_signal"),
    ] {
        let payload = fixture_string(&fixture, pointer)?;
        for role in [MessageRole::Request, MessageRole::Response] {
            assert_code(
                decode_terminal_effect_event(kind, role, payload.as_bytes()),
                ProtocolErrorCode::InvalidField,
            );
        }
    }
    for role in [MessageRole::Request, MessageRole::Response] {
        assert_code(
            decode_terminal_effect_event(57, role, METADATA_EXAMPLE.as_bytes()),
            ProtocolErrorCode::InvalidField,
        );
    }
    assert_code(
        decode_terminal_effect_event(54, MessageRole::Event, b"{}"),
        ProtocolErrorCode::InvalidField,
    );

    let bell = fixture_string(&fixture, "/outputs/terminal_bell")?;
    let exact = format!(
        "{bell}{}",
        " ".repeat(MAXIMUM_CONTROL_JSON_BYTES - bell.len())
    );
    assert_eq!(exact.len(), MAXIMUM_CONTROL_JSON_BYTES);
    assert!(decode_terminal_effect_event(55, MessageRole::Event, exact.as_bytes()).is_ok());
    let over = format!("{exact} ");
    assert_code(
        decode_terminal_effect_event(55, MessageRole::Event, over.as_bytes()),
        ProtocolErrorCode::LimitExceeded,
    );

    let oversized_metadata = TerminalEffectEvent::MetadataChanged(TerminalMetadataChanged {
        current_directory: Some("x".repeat(MAXIMUM_CONTROL_JSON_BYTES)),
        identity: example_identity()?,
        revision: NonZeroU64::new(1).ok_or_else(|| fixture_error("one must be nonzero"))?,
        title: None,
    });
    assert_code(
        encode_terminal_effect_event(&oversized_metadata),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

#[test]
fn terminal_effect_events_compose_with_the_transport_envelope() -> Result<(), Box<dyn Error>> {
    let fixture = read_fixture()?;
    for (kind, payload) in [
        (55, fixture_string(&fixture, "/outputs/terminal_bell")?),
        (
            56,
            fixture_string(&fixture, "/outputs/terminal_clipboard_write_request")?,
        ),
        (57, METADATA_EXAMPLE),
        (
            58,
            fixture_string(&fixture, "/outputs/terminal_application_signal")?,
        ),
    ] {
        let value = decode_terminal_effect_event(kind, MessageRole::Event, payload.as_bytes())?;
        let encoded = encode_terminal_effect_event(&value)?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: Some(HostEpoch::parse(&"11".repeat(16))?),
            kind: encoded.kind,
            optional_message: false,
            payload: &encoded.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
            role: encoded.role,
        })?;
        let envelope = decode_transport_envelope(&record)?;
        assert_eq!(envelope.kind, kind);
        assert_eq!(envelope.role, MessageRole::Event);
        assert_eq!(envelope.payload, payload.as_bytes());
        assert_eq!(
            decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload)?,
            value
        );
    }
    Ok(())
}

fn example_identity() -> Result<TerminalIdentity, orchbee_terminal_protocol::IdentityError> {
    Ok(TerminalIdentity {
        creation_id: CreationId::parse(&"11".repeat(16))?,
        host_epoch: HostEpoch::parse(&"22".repeat(16))?,
        terminal_id: TerminalId::parse(&"33".repeat(16))?,
    })
}

fn assert_public_round_trip(value: TerminalEffectEvent) -> Result<(), ProtocolError> {
    let encoded = encode_terminal_effect_event(&value)?;
    assert_eq!(encoded.role, MessageRole::Event);
    assert_eq!(
        decode_terminal_effect_event(encoded.kind, encoded.role, &encoded.payload)?,
        value
    );
    Ok(())
}

fn assert_canonical_round_trip(kind: u16, payload: &str) -> Result<(), Box<dyn Error>> {
    let decoded = decode_terminal_effect_event(kind, MessageRole::Event, payload.as_bytes())?;
    let encoded: EncodedTerminalEffectEvent = encode_terminal_effect_event(&decoded)?;
    assert_eq!(encoded.kind, kind);
    assert_eq!(encoded.role, MessageRole::Event);
    assert_eq!(encoded.payload, payload.as_bytes());
    Ok(())
}

fn read_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
}

fn fixture_string<'fixture>(
    fixture: &'fixture Value,
    pointer: &str,
) -> Result<&'fixture str, io::Error> {
    fixture
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| fixture_error("fixture pointer is not a string"))
}

fn decode_value(kind: u16, value: Value) -> Result<TerminalEffectEvent, ProtocolError> {
    let payload = serde_json::to_vec(&value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidJson,
            "test value could not be serialized",
        )
    })?;
    decode_terminal_effect_event(kind, MessageRole::Event, &payload)
}

fn assert_value_code(kind: u16, value: Value, expected: ProtocolErrorCode) {
    assert_code(decode_value(kind, value), expected);
}

fn assert_payload_code(kind: u16, payload: &str, expected: ProtocolErrorCode) {
    assert_code(
        decode_terminal_effect_event(kind, MessageRole::Event, payload.as_bytes()),
        expected,
    );
}

fn assert_code<T>(result: Result<T, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Err(error) => assert_eq!(error.code(), expected),
        Ok(_) => panic!("payload unexpectedly succeeded"),
    }
}

fn set_pointer(value: &mut Value, pointer: &str, replacement: Value) -> Result<(), io::Error> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| fixture_error("test JSON pointer is absent"))?;
    *target = replacement;
    Ok(())
}

fn object_mut(value: &mut Value) -> Result<&mut serde_json::Map<String, Value>, io::Error> {
    value
        .as_object_mut()
        .ok_or_else(|| fixture_error("test JSON value is not an object"))
}

fn fixture_error(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail)
}
