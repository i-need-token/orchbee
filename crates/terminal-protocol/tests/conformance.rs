use std::error::Error;

use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch, MAXIMUM_CONTROL_JSON_BYTES,
    MAXIMUM_RAW_PAYLOAD_BYTES, MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_TERMINAL_FRAME_BYTES,
    MAXIMUM_TRANSPORT_RECORD_BYTES, MESSAGE_CONTRACTS, MessageRole, PayloadEncoding, ProtocolError,
    ProtocolErrorCode, TRANSPORT_ENVELOPE_HEADER_BYTES, decode_transport_envelope,
    encode_transport_envelope, message_contract,
};
use serde::Deserialize;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance-v0.0.24.json");
const ENVELOPE_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/transport-envelope-v0.0.24.bin");
const FRAME_FIXTURE: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin");

#[derive(Deserialize)]
struct ControlFixture {
    message_contracts: Vec<FixtureMessageContract>,
    protocol_limits: FixtureProtocolLimits,
    #[serde(flatten)]
    _remaining: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureMessageContract {
    event_encoding: Option<String>,
    kind: u16,
    name: String,
    request: Option<FixtureRequestContract>,
    response_encoding: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRequestContract {
    payload_encoding: String,
    success_kind: u16,
}

#[derive(Deserialize)]
struct FixtureProtocolLimits {
    control_json_bytes: usize,
    frame_bytes: usize,
    raw_payload_bytes: usize,
    row_window_bytes: usize,
    transport_record_bytes: usize,
}

#[test]
fn complete_message_registry_matches_the_canonical_fixture() -> Result<(), Box<dyn Error>> {
    let fixture = read_control_fixture()?;
    assert_eq!(MESSAGE_CONTRACTS.len(), fixture.message_contracts.len());

    for (actual, expected) in MESSAGE_CONTRACTS.iter().zip(&fixture.message_contracts) {
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.name, expected.name);
        assert_eq!(
            actual.event_encoding,
            expected
                .event_encoding
                .as_deref()
                .map(fixture_payload_encoding)
                .transpose()?
        );
        assert_eq!(
            actual.response_encoding,
            expected
                .response_encoding
                .as_deref()
                .map(fixture_payload_encoding)
                .transpose()?
        );
        match (actual.request, &expected.request) {
            (Some(actual_request), Some(expected_request)) => {
                assert_eq!(
                    actual_request.payload_encoding,
                    fixture_payload_encoding(&expected_request.payload_encoding)?
                );
                assert_eq!(actual_request.success_kind, expected_request.success_kind);
            }
            (None, None) => {}
            _ => panic!("request contract differs for kind {}", actual.kind),
        }
        assert_eq!(message_contract(actual.kind), Some(actual));
    }

    for kind in 0..=u16::MAX {
        let expected = fixture
            .message_contracts
            .binary_search_by_key(&kind, |contract| contract.kind)
            .is_ok();
        assert_eq!(message_contract(kind).is_some(), expected, "kind {kind}");
    }
    Ok(())
}

#[test]
fn envelope_limits_match_the_canonical_fixture() -> Result<(), Box<dyn Error>> {
    let limits = read_control_fixture()?.protocol_limits;
    assert_eq!(MAXIMUM_CONTROL_JSON_BYTES, limits.control_json_bytes);
    assert_eq!(MAXIMUM_RAW_PAYLOAD_BYTES, limits.raw_payload_bytes);
    assert_eq!(MAXIMUM_TERMINAL_FRAME_BYTES, limits.frame_bytes);
    assert_eq!(MAXIMUM_ROW_WINDOW_BYTES, limits.row_window_bytes);
    assert_eq!(
        MAXIMUM_TRANSPORT_RECORD_BYTES,
        limits.transport_record_bytes
    );
    assert_eq!(
        MAXIMUM_TRANSPORT_RECORD_BYTES,
        TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_ROW_WINDOW_BYTES
    );
    Ok(())
}

#[test]
fn canonical_envelope_decodes_and_reencodes_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let decoded = decode_transport_envelope(ENVELOPE_FIXTURE)?;
    assert_eq!(decoded.correlation_id, None);
    assert_eq!(decoded.host_epoch, Some(host_epoch(0x11)?));
    assert_eq!(decoded.ignored_optional_flags, 0);
    assert_eq!(decoded.kind, 42);
    assert!(decoded.known_message);
    assert!(!decoded.optional_message);
    assert_eq!(decoded.payload, FRAME_FIXTURE);
    assert_eq!(decoded.payload_encoding, PayloadEncoding::TerminalFrame);
    assert_eq!(decoded.protocol, CURRENT_PROTOCOL_VERSION);
    assert_eq!(decoded.role, MessageRole::Event);

    let encoded = encode_transport_envelope(&Envelope {
        correlation_id: decoded.correlation_id,
        host_epoch: decoded.host_epoch,
        kind: decoded.kind,
        optional_message: decoded.optional_message,
        payload: decoded.payload,
        payload_encoding: decoded.payload_encoding,
        protocol: decoded.protocol,
        role: decoded.role,
    })?;
    assert_eq!(encoded, ENVELOPE_FIXTURE);
    Ok(())
}

#[test]
fn malformed_headers_roles_and_encodings_are_rejected() -> Result<(), Box<dyn Error>> {
    assert_code(
        decode_transport_envelope(&ENVELOPE_FIXTURE[..63]),
        ProtocolErrorCode::Truncated,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_byte(&mut mutated, 0, b'X')?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidMagic,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 4, 63)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::UnsupportedVersion,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u32(&mut mutated, 8, 500)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::LengthMismatch,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u32(&mut mutated, 12, 436)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::LengthMismatch,
    );

    for offset in [23, 58] {
        let mut mutated = ENVELOPE_FIXTURE.to_vec();
        set_byte(&mut mutated, offset, 1)?;
        assert_code(
            decode_transport_envelope(&mutated),
            ProtocolErrorCode::UnknownRequiredFlag,
        );
    }

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0x0104)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::UnknownRequiredFlag,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0x0006)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 20, 60_000)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::UnknownRequiredMessage,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_byte(&mut mutated, 22, 0)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );

    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_byte(&mut mutated, 22, 1)?;
    assert_code(
        decode_transport_envelope(&mutated),
        ProtocolErrorCode::InvalidField,
    );
    Ok(())
}

#[test]
fn unknown_optional_messages_and_flags_remain_skippable() -> Result<(), Box<dyn Error>> {
    let mut mutated = ENVELOPE_FIXTURE.to_vec();
    set_u16(&mut mutated, 6, 0x000d)?;
    set_u16(&mut mutated, 20, 60_000)?;
    let decoded = decode_transport_envelope(&mutated)?;
    assert!(!decoded.known_message);
    assert!(decoded.optional_message);
    assert_eq!(decoded.ignored_optional_flags, 0x08);
    assert_eq!(decoded.role, MessageRole::Event);
    assert_eq!(decoded.payload_encoding, PayloadEncoding::TerminalFrame);
    assert_eq!(decoded.payload, FRAME_FIXTURE);
    Ok(())
}

#[test]
fn encoder_rejects_unknown_required_kinds_and_illegal_contract_pairs() -> Result<(), Box<dyn Error>>
{
    let base = Envelope {
        correlation_id: Some(correlation_id(0x22)?),
        host_epoch: Some(host_epoch(0x11)?),
        kind: 1,
        optional_message: false,
        payload: b"{}",
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    };

    assert_code(
        encode_transport_envelope(&Envelope {
            kind: 60_000,
            ..base
        }),
        ProtocolErrorCode::UnknownRequiredMessage,
    );
    assert_code(
        encode_transport_envelope(&Envelope {
            role: MessageRole::Event,
            ..base
        }),
        ProtocolErrorCode::InvalidField,
    );
    assert_code(
        encode_transport_envelope(&Envelope {
            payload_encoding: PayloadEncoding::RawBytes,
            ..base
        }),
        ProtocolErrorCode::InvalidField,
    );

    let optional = encode_transport_envelope(&Envelope {
        kind: 60_000,
        optional_message: true,
        payload_encoding: PayloadEncoding::RawBytes,
        role: MessageRole::Event,
        ..base
    })?;
    let decoded = decode_transport_envelope(&optional)?;
    assert!(!decoded.known_message);
    assert!(decoded.optional_message);
    Ok(())
}

#[test]
fn representative_legal_roles_round_trip_without_payload_reinterpretation()
-> Result<(), Box<dyn Error>> {
    let cases = [
        (1, MessageRole::Request, PayloadEncoding::ControlJson),
        (2, MessageRole::Response, PayloadEncoding::ControlJson),
        (30, MessageRole::Request, PayloadEncoding::RawBytes),
        (42, MessageRole::Event, PayloadEncoding::TerminalFrame),
        (46, MessageRole::Response, PayloadEncoding::RowWindow),
    ];
    let payloads = [0_usize, 1, 63, 512];

    for (kind, role, payload_encoding) in cases {
        for payload_length in payloads {
            let payload = (0..payload_length)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>();
            let value = Envelope {
                correlation_id: Some(correlation_id(0x22)?),
                host_epoch: Some(host_epoch(0x11)?),
                kind,
                optional_message: false,
                payload: &payload,
                payload_encoding,
                protocol: CURRENT_PROTOCOL_VERSION,
                role,
            };
            let encoded = encode_transport_envelope(&value)?;
            let decoded = decode_transport_envelope(&encoded)?;
            assert_eq!(decoded.correlation_id, value.correlation_id);
            assert_eq!(decoded.host_epoch, value.host_epoch);
            assert_eq!(decoded.kind, value.kind);
            assert_eq!(decoded.optional_message, value.optional_message);
            assert_eq!(decoded.payload, value.payload);
            assert_eq!(decoded.payload_encoding, value.payload_encoding);
            assert_eq!(decoded.protocol, value.protocol);
            assert_eq!(decoded.role, value.role);
        }
    }
    Ok(())
}

#[test]
fn aggregate_record_boundary_is_exact() -> Result<(), Box<dyn Error>> {
    let payload = vec![0; MAXIMUM_ROW_WINDOW_BYTES + 1];
    let base = Envelope {
        correlation_id: Some(correlation_id(0x22)?),
        host_epoch: Some(host_epoch(0x11)?),
        kind: 46,
        optional_message: false,
        payload: &payload[..MAXIMUM_ROW_WINDOW_BYTES],
        payload_encoding: PayloadEncoding::RowWindow,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    };
    let encoded = encode_transport_envelope(&base)?;
    assert_eq!(encoded.len(), MAXIMUM_TRANSPORT_RECORD_BYTES);
    assert_eq!(
        decode_transport_envelope(&encoded)?.payload.len(),
        MAXIMUM_ROW_WINDOW_BYTES
    );
    drop(encoded);

    assert_code(
        encode_transport_envelope(&Envelope {
            payload: &payload,
            ..base
        }),
        ProtocolErrorCode::LimitExceeded,
    );

    let mut overlong_record = vec![0; MAXIMUM_TRANSPORT_RECORD_BYTES + 1];
    overlong_record[..4].copy_from_slice(b"OBTP");
    assert_code(
        decode_transport_envelope(&overlong_record),
        ProtocolErrorCode::LimitExceeded,
    );
    Ok(())
}

fn read_control_fixture() -> Result<ControlFixture, serde_json::Error> {
    serde_json::from_str(CONTROL_FIXTURE)
}

fn fixture_payload_encoding(value: &str) -> Result<PayloadEncoding, Box<dyn Error>> {
    match value {
        "control-json" => Ok(PayloadEncoding::ControlJson),
        "raw-bytes" => Ok(PayloadEncoding::RawBytes),
        "terminal-frame" => Ok(PayloadEncoding::TerminalFrame),
        "row-window" => Ok(PayloadEncoding::RowWindow),
        _ => Err(format!("unknown fixture payload encoding {value}").into()),
    }
}

fn correlation_id(byte: u8) -> Result<CorrelationId, Box<dyn Error>> {
    Ok(CorrelationId::from_bytes([byte; 16])?)
}

fn host_epoch(byte: u8) -> Result<HostEpoch, Box<dyn Error>> {
    Ok(HostEpoch::from_bytes([byte; 16])?)
}

fn assert_code<T>(result: Result<T, ProtocolError>, expected: ProtocolErrorCode) {
    match result {
        Ok(_value) => panic!("expected protocol error {expected}"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}

fn set_byte(bytes: &mut [u8], offset: usize, value: u8) -> Result<(), Box<dyn Error>> {
    let target = bytes
        .get_mut(offset)
        .ok_or_else(|| format!("missing fixture byte {offset}"))?;
    *target = value;
    Ok(())
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), Box<dyn Error>> {
    set_bytes(bytes, offset, &value.to_le_bytes())
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Box<dyn Error>> {
    set_bytes(bytes, offset, &value.to_le_bytes())
}

fn set_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Box<dyn Error>> {
    let target = bytes
        .get_mut(offset..offset.saturating_add(value.len()))
        .ok_or_else(|| format!("missing fixture range at {offset}"))?;
    target.copy_from_slice(value);
    Ok(())
}
